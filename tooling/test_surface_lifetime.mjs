// No browser automation: exercise the production stop owner with fake channels.
import test from "node:test";
import assert from "node:assert/strict";
import {readFileSync} from "node:fs";
import vm from "node:vm";
const create=vm.runInNewContext(readFileSync(new URL("../assets/surface-lifetime.js",import.meta.url),"utf8")+"\ncreateSurfaceLifetime");

class Target extends EventTarget {
    count=0;
    addEventListener(...args){this.count++;super.addEventListener(...args);}
    removeEventListener(...args){this.count--;super.removeEventListener(...args);}
}

test("out-of-band stop wakes an idle broken data channel and releases its listener",async()=>{
    const target=new Target();let outstanding=0,lateReply;
    const lifetime=create({recv(){outstanding++;return new Promise(resolve=>{lateReply=resolve;});}},target);
    const waiting=lifetime.receive();
    assert.equal(outstanding,1);
    target.dispatchEvent(new Event("pttstella:stop"));
    assert.equal((await waiting).stop,true);
    assert.equal((await lifetime.receive()).stop,true);
    assert.equal(outstanding,1); // No retry/extra channel waiter after stop.
    lateReply({frame:"discarded"});
    lifetime.release();assert.equal(target.count,0);
});

test("normal packets still arrive once, and receive failure retains explicit cleanup",async()=>{
    const target=new Target();let calls=0;
    const lifetime=create({recv(){calls++;return calls===1?Promise.resolve({revision:42}):Promise.reject(new Error("channel closed"));}},target);
    assert.equal((await lifetime.receive()).revision,42);
    await assert.rejects(lifetime.receive(),/channel closed/);
    assert.equal(target.count,1);
    lifetime.release();assert.equal(target.count,0);
});

test("stop before receive does not start channel work",async()=>{
    const target=new Target();let revocations=0;
    const lifetime=create({recv(){assert.fail("revoked renderer received again");}},target,()=>{revocations++;});
    target.dispatchEvent(new Event("pttstella:stop"));
    assert.equal(revocations,1); // Synchronous admission revocation, before await.
    target.dispatchEvent(new Event("pttstella:stop"));
    assert.equal((await lifetime.receive()).stop,true);
    assert.equal(revocations,1);
    lifetime.release();assert.equal(target.count,0);
});

test("twenty thousand packets do not accumulate reactions on an unresolved stop promise",async()=>{
    const states=[];
    class TrackedPromise extends Promise {
        constructor(executor){
            const state={settled:false,reactions:0};
            super((resolve,reject)=>executor(value=>{state.settled=true;resolve(value);},error=>{state.settled=true;reject(error);}));
            this.state=state;states.push(state);
        }
        static get [Symbol.species](){return Promise;}
        then(...args){if(!this.state.settled)this.state.reactions++;return super.then(...args);}
    }
    const factory=vm.runInNewContext(readFileSync(new URL("../assets/surface-lifetime.js",import.meta.url),"utf8")+"\ncreateSurfaceLifetime",{Promise:TrackedPromise});
    const target=new Target();let received=0;
    const lifetime=factory({recv(){return Promise.resolve({sequence:++received});}},target);
    for(let index=1;index<=20000;index++){
        assert.equal((await lifetime.receive()).sequence,index);
        if(index%1000===0){
            const retained=states.filter(state=>!state.settled).reduce((sum,state)=>sum+state.reactions,0);
            assert.ok(retained<=2,`unresolved stop retained ${retained} reactions after ${index} packets`);
        }
    }
    target.dispatchEvent(new Event("pttstella:stop"));
    assert.equal((await lifetime.receive()).stop,true);
    lifetime.release();assert.equal(target.count,0);
});
