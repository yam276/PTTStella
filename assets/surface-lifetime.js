// Dioxus Desktop releases a returned Eval through a JS FinalizationRegistry.
// Keep its channel reachable until Rust has consumed the final result. A single
// window-owned listener is released by the native callback after join, including
// error paths; it never carries BBS input or starts a timer/worker.
function retainSurfaceResult(bridge,target) {
    const slot=Symbol("PTTStella pending surface result");
    Object.defineProperty(target,slot,{value:bridge,configurable:true});
    const release=()=>{
        target.removeEventListener("pttstella:release-result",release);
        delete target[slot];
    };
    target.addEventListener("pttstella:release-result",release);
}

function flushNativeInterpreter(interpreter) {
    if(!interpreter||!Array.isArray(interpreter.queuedBytes)||interpreter.queuedBytes.length===0)return false;
    interpreter.flushQueuedBytes();
    interpreter.markEditsFinished();
    return true;
}

// Dioxus Desktop's NativeInterpreter only sets `headless` at initial window
// creation and otherwise waits for requestAnimationFrame before acknowledging
// DOM edits via markEditsFinished(). While a commit is unacknowledged, Rust's
// poll_vdom() returns before polling VirtualDom tasks. On GTK/Wayland, hiding
// the window on CloseRequested (WindowHides) unmaps the surface and suspends
// requestAnimationFrame, stranding the shutdown task before window.close().
function guardNativeInterpreter(target,{
    document=target?.document,
    requestFrame=typeof target?.requestAnimationFrame==="function"
        ?target.requestAnimationFrame.bind(target)
        :typeof requestAnimationFrame==="function"
            ?requestAnimationFrame
            :null,
    setTimer=typeof target?.setTimeout==="function"
        ?target.setTimeout.bind(target)
        :typeof setTimeout==="function"
            ?setTimeout
            :null,
    clearTimer=typeof target?.clearTimeout==="function"
        ?target.clearTimeout.bind(target)
        :typeof clearTimeout==="function"
            ?clearTimeout
            :null,
    fallbackMs=32,
}={}) {
    const interpreter=target?.interpreter;
    if(!interpreter||typeof interpreter.rafEdits!=="function"||typeof interpreter.enqueueBytes!=="function"
        ||typeof interpreter.flushQueuedBytes!=="function"||typeof interpreter.markEditsFinished!=="function") {
        return {flush(){return false;},stop(){},release(){}};
    }
    let timer=0;
    const clearFallback=()=>{if(timer){clearTimer?.(timer);timer=0;}};
    const hidden=()=>Boolean(interpreter.headless||document?.hidden||document?.visibilityState==="hidden");
    const flush=()=>{
        clearFallback();
        return flushNativeInterpreter(interpreter);
    };
    const originalRafEdits=interpreter.rafEdits;
    interpreter.rafEdits=bytes=>{
        interpreter.enqueueBytes(bytes);
        if(hidden()||!requestFrame){
            flush();
            return;
        }
        requestFrame(flush);
        clearFallback();
        if(setTimer)timer=setTimer(flush,fallbackMs);
    };
    const onVisibilityChange=()=>{if(hidden())flush();};
    document?.addEventListener?.("visibilitychange",onVisibilityChange);
    if(hidden())flush();
    return {
        flush,
        stop(){
            interpreter.headless=true;
            flush();
        },
        release(){
            clearFallback();
            document?.removeEventListener?.("visibilitychange",onVisibilityChange);
            interpreter.rafEdits=originalRafEdits;
        }
    };
}

// One renderer per native window. The out-of-band stop also works when its
// data channel cannot accept a packet. It can only revoke this renderer.
function createSurfaceLifetime(bridge, target, revoke=()=>{}, options={}) {
    let stopping=false, waiter=null;
    const interpreter=guardNativeInterpreter(target,options);
    const stop=()=>{
        if(stopping)return;
        stopping=true;interpreter.stop();revoke();waiter?.resolve({kind:"stop"});waiter=null;
    };
    target.addEventListener("pttstella:stop",stop);
    return {
        receive() {
            if(stopping)return Promise.resolve({kind:"stop"});
            if(waiter)return Promise.reject(new Error("surface receive already pending"));
            return new Promise((resolve,reject)=>{
                const current={resolve,reject};waiter=current;
                const finish=(reply,value)=>{
                    if(waiter!==current)return;
                    waiter=null;reply(value);
                };
                try {
                    Promise.resolve(bridge.recv()).then(packet=>{
                        try {
                            packet=decodeSurfaceEnvelope(packet);
                            if(packet?.kind==="stop")stop();else finish(resolve,packet);
                        } catch(error){finish(reject,error);}
                    },error=>finish(reject,error));
                } catch(error){finish(reject,error);}
            });
        },
        release(){
            interpreter.release();
            target.removeEventListener("pttstella:stop",stop);
        }
    };
}
