// One renderer per native window. The out-of-band stop also works when its
// data channel cannot accept a packet. It can only revoke this renderer.
function createSurfaceLifetime(bridge, target, revoke=()=>{}) {
    let stopping=false, waiter=null;
    const stop=()=>{
        if(stopping)return;
        stopping=true;revoke();waiter?.resolve({stop:true});waiter=null;
    };
    target.addEventListener("pttstella:stop",stop);
    return {
        receive() {
            if(stopping)return Promise.resolve({stop:true});
            if(waiter)return Promise.reject(new Error("surface receive already pending"));
            return new Promise((resolve,reject)=>{
                const current={resolve,reject};waiter=current;
                const finish=(reply,value)=>{
                    if(waiter!==current)return;
                    waiter=null;reply(value);
                };
                try {
                    Promise.resolve(bridge.recv()).then(packet=>{
                        if(packet.stop)stop();else finish(resolve,packet);
                    },error=>finish(reject,error));
                } catch(error){finish(reject,error);}
            });
        },
        release(){target.removeEventListener("pttstella:stop",stop);}
    };
}
