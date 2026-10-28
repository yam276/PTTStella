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

// One renderer per native window. The out-of-band stop also works when its
// data channel cannot accept a packet. It can only revoke this renderer.
function createSurfaceLifetime(bridge, target, revoke=()=>{}) {
    let stopping=false, waiter=null;
    const stop=()=>{
        if(stopping)return;
        stopping=true;revoke();waiter?.resolve({kind:"stop"});waiter=null;
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
        release(){target.removeEventListener("pttstella:stop",stop);}
    };
}
