// One trusted pointer transaction. Rust owns modes, admission and byte encoding.
function createStationMouse({current,point,viewport,send,blocked,prepare,changed}) {
    let held=null,sequence=0,disposed=false;
    const route=()=>current()?.mouse?.route??"local";
    const local=()=>route()==="local";
    const modifiers=e=>({shift:!!e.shiftKey,alt:!!e.altKey,ctrl:!!e.ctrlKey});
    const sameLease=(a,b)=>!!a&&!!b&&a.transaction===b.transaction&&a.mode_epoch===b.mode_epoch
        &&a.policy_epoch===b.policy_epoch&&a.stamp.generation===b.stamp.generation
        &&a.stamp.revision===b.stamp.revision&&a.stamp.input_epoch===b.stamp.input_epoch;
    function finish(cancel) {
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
        if(local()||e.shiftKey)return false;
        e.preventDefault();
        if(disposed||!e.isTrusted||!e.isPrimary||held||e.metaKey||blocked()||route()!=="station")return true;
        const packet=current(),cell=point(e,false),button=["left","middle","right"][e.button];
        if(!packet?.connected||packet.output_pending||!cell||!button||sequence>=Number.MAX_SAFE_INTEGER)return true;
        prepare();
        const lease={stamp:{generation:packet.generation,revision:packet.frame.revision,input_epoch:packet.input_epoch},
            mode_epoch:packet.mouse.modes.epoch,policy_epoch:packet.mouse.policy_epoch,transaction:++sequence};
        held={lease,pointer:e.pointerId,button,observed:false};
        try{viewport.setPointerCapture(e.pointerId);}catch(_){finish(true);return true;}
        send({kind:"station_mouse",event:{lease,report:{...cell,button,action:"press",modifiers:modifiers(e)}},trusted:true});
        changed();return true;
    }
    function up(e) {
        if(!held)return !local()&&!e.shiftKey;
        if(e.pointerId!==held.pointer)return true;
        e.preventDefault();
        const cell=point(e,false);
        if(!e.isTrusted||["left","middle","right"][e.button]!==held.button||e.metaKey||blocked()||route()!=="station"||!cell){cancel();return true;}
        const event={lease:held.lease,report:{...cell,button:held.button,action:"release",modifiers:modifiers(e)}};
        finish(false);send({kind:"station_mouse",event,trusted:true});return true;
    }
    function update(packet) {
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
    return {down,up,update,cancel,revoke,reject,lost,destroy,local,active:()=>!!held,clean:()=>disposed&&!held};
}
