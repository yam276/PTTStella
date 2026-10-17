// One trusted pointer transaction. Rust owns modes, admission and byte encoding.
function createStationMouse({current,point,viewport,send,blocked,prepare,changed,
    requestFrame=callback=>requestAnimationFrame(callback),cancelFrame=id=>cancelAnimationFrame(id)}) {
    let held=null,sequence=0,sampleSequence=0,disposed=false,frame=0,pendingMotion=null,pendingWheel=null,wheelRemainder=0,wheelContext=null,lastFrameTime=null;
    const route=()=>current()?.mouse?.route??"local";
    const local=()=>route()==="local";
    const buttonNames=["left","middle","right"],buttonMask={left:1,middle:4,right:2};
    const modifiers=e=>({shift:!!e.shiftKey,alt:!!e.altKey,ctrl:!!e.ctrlKey});
    const sameLease=(a,b)=>!!a&&!!b&&a.transaction===b.transaction&&a.mode_epoch===b.mode_epoch
        &&a.policy_epoch===b.policy_epoch&&a.stamp.generation===b.stamp.generation
        &&a.stamp.revision===b.stamp.revision&&a.stamp.input_epoch===b.stamp.input_epoch;
    function lease(packet) {
        if(sequence>=Number.MAX_SAFE_INTEGER)return null;
        return {stamp:{generation:packet.generation,revision:packet.frame.revision,input_epoch:packet.input_epoch},
            mode_epoch:packet.mouse.modes.epoch,policy_epoch:packet.mouse.policy_epoch,transaction:++sequence};
    }
    function clearSamples() {
        if(frame)cancelFrame(frame);
        frame=0;pendingMotion=null;pendingWheel=null;wheelRemainder=0;wheelContext=null;
    }
    function currentSample(sample,packet=current()) {
        const source=sample?.lease;
        return !!packet&&!!source&&packet.connected&&packet.mouse?.route==="station"
            &&packet.generation===source.stamp.generation&&packet.mouse.modes.epoch===source.mode_epoch
            &&packet.mouse.policy_epoch===source.policy_epoch
            &&(sameLease(held?.lease,source)||packet.frame.revision===source.stamp.revision&&packet.input_epoch===source.stamp.input_epoch);
    }
    function flushSamples(time) {
        if(frame)cancelFrame(frame);
        frame=0;
        if(Number.isFinite(time)&&lastFrameTime!==null&&time-lastFrameTime<34&&(pendingWheel||pendingMotion)){
            schedule();return;
        }
        // One latest sample per animation frame; wheel takes precedence over
        // passive motion. No queued tail is replayed after a source change.
        const sample=pendingWheel??pendingMotion;pendingWheel=null;pendingMotion=null;
        if(disposed||blocked()||!currentSample(sample)||sampleSequence>=Number.MAX_SAFE_INTEGER)return;
        if(Number.isFinite(time))lastFrameTime=time;
        send({kind:"station_mouse_sample",sample:{...sample,sequence:++sampleSequence},trusted:true});
    }
    function schedule() {if(!frame)frame=requestFrame(flushSamples);}
    function finish(cancel) {
        clearSamples();
        const previous=held;held=null;
        if(!previous)return;
        if(viewport.hasPointerCapture(previous.pointer))viewport.releasePointerCapture(previous.pointer);
        changed();
        if(cancel&&!disposed)send({kind:"station_mouse_cancel",lease:previous.lease});
    }
    function cancel(){finish(true);}
    function revoke(){
        if(disposed)return;
        const generation=current()?.generation;
        cancel();
        if(generation!==undefined)send({kind:"station_mouse_revoke",generation});
    }
    function down(e) {
        if(held){e.preventDefault();return true;}
        clearSamples();
        if(local()||e.shiftKey)return false;
        e.preventDefault();
        if(disposed||!e.isTrusted||!e.isPrimary||held||e.metaKey||blocked()||route()!=="station")return true;
        const packet=current(),cell=point(e,false),button=buttonNames[e.button];
        if(!packet?.connected||packet.output_pending||!cell||!button||sequence>=Number.MAX_SAFE_INTEGER)return true;
        prepare();
        const source=lease(packet);
        held={lease:source,pointer:e.pointerId,button,observed:false};
        try{viewport.setPointerCapture(e.pointerId);}catch(_){finish(true);return true;}
        send({kind:"station_mouse",event:{lease:source,report:{...cell,button,action:"press",modifiers:modifiers(e)}},trusted:true});
        changed();return true;
    }
    function up(e) {
        if(!held)return !local()&&!e.shiftKey;
        if(e.pointerId!==held.pointer)return true;
        e.preventDefault();
        const cell=point(e,false);
        if(!e.isTrusted||buttonNames[e.button]!==held.button||e.metaKey||blocked()||route()!=="station"||!cell){cancel();return true;}
        flushSamples();
        const event={lease:held.lease,report:{...cell,button:held.button,action:"release",modifiers:modifiers(e)}};
        finish(false);send({kind:"station_mouse",event,trusted:true});return true;
    }
    function move(e) {
        if(local())return false;
        if(disposed||!e.isTrusted||!e.isPrimary)return true;
        if(held&&held.pointer!==e.pointerId)return true;
        const physical=Number.isInteger(e.buttons)&&e.buttons>=0?e.buttons:null;
        if(held&&(physical===null||!(physical&buttonMask[held.button]))){cancel();return true;}
        if(!held&&physical!==0){clearSamples();return true;}
        if(e.metaKey||!held&&e.shiftKey||blocked()||route()!=="station")return true;
        const packet=current(),tracking=packet?.mouse?.modes?.tracking,cell=point(e,false);
        if(!cell){clearSamples();return true;}
        if(tracking!=="motion"&&!(tracking==="drag"&&held))return true;
        const source=held?.lease??lease(packet);
        if(!source)return true;
        pendingMotion={lease:source,report:{kind:"motion",...cell,button:held?.button??null,modifiers:modifiers(e)}};
        schedule();return true;
    }
    function wheel(e) {
        if(local())return false;
        e.preventDefault();
        if(disposed||!e.isTrusted||e.ctrlKey||e.shiftKey||e.metaKey||blocked()||route()!=="station"||!Number.isFinite(e.deltaY)||!e.deltaY)return true;
        const packet=current(),cell=point(e,false);
        if(!cell){clearSamples();return true;}
        // Normalize pixel/line/page deltas, retain at most one sub-cell fraction
        // and one latest tick. A fast or inertial gesture cannot leave a backlog.
        const context=[packet.generation,packet.frame.revision,packet.input_epoch,packet.mouse.modes.epoch,packet.mouse.policy_epoch].join(":");
        if(wheelContext!==context){wheelRemainder=0;wheelContext=context;}
        const delta=e.deltaY*(e.deltaMode===1?40:e.deltaMode===2?40*Math.max(1,packet.frame.rows??24):1);
        wheelRemainder=Math.max(-160,Math.min(160,wheelRemainder+delta));
        if(Math.abs(wheelRemainder)<40)return true;
        const direction=wheelRemainder<0?"up":"down";wheelRemainder%=40;
        const source=held?.lease??lease(packet);
        if(!source)return true;
        pendingWheel={lease:source,report:{kind:"wheel",...cell,direction,modifiers:modifiers(e)}};
        schedule();return true;
    }
    function update(packet) {
        if(pendingMotion&&!currentSample(pendingMotion,packet)||pendingWheel&&!currentSample(pendingWheel,packet))clearSamples();
        if(!held)return;
        const mouse=packet.mouse,lease=held.lease;
        if(!packet.connected||!mouse||mouse.route!=="station"
            ||packet.generation!==lease.stamp.generation||mouse.modes.epoch!==lease.mode_epoch||mouse.policy_epoch!==lease.policy_epoch){cancel();return;}
        if(sameLease(mouse.active,lease))held.observed=true;
        else if(held.observed||packet.input_epoch>lease.stamp.input_epoch)cancel();
    }
    function reject(lease){if(sameLease(held?.lease,lease))cancel();}
    function lost(e){if(held?.pointer===e.pointerId&&!viewport.hasPointerCapture(e.pointerId))cancel();}
    function destroy(){if(disposed)return;disposed=true;finish(false);}
    return {down,up,move,wheel,update,cancel,revoke,reject,lost,destroy,local,active:()=>!!held,
        clean:()=>disposed&&!held&&!frame&&!pendingMotion&&!pendingWheel};
}
