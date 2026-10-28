// No browser automation: exercise the production stop owner with fake channels.
import {spawnSync} from "node:child_process";
import {fileURLToPath} from "node:url";
import test from "node:test";
import assert from "node:assert/strict";
import {readFileSync} from "node:fs";
import vm from "node:vm";
const packetsSource=readFileSync(new URL("../assets/surface-packets.js",import.meta.url),"utf8");
const create=vm.runInNewContext(packetsSource+"\n"+readFileSync(new URL("../assets/surface-lifetime.js",import.meta.url),"utf8")+"\ncreateSurfaceLifetime");

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
    assert.equal((await waiting).kind,"stop");
    assert.equal((await lifetime.receive()).kind,"stop");
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

test("malformed envelopes reach the router without stranding the receive waiter",{timeout:1000},async()=>{
    const router=vm.runInNewContext(readFileSync(new URL("../assets/surface-packets.js",import.meta.url),"utf8")+"\ncreateSurfaceRouter");
    const calls=[],route=router(new Proxy({}, {get:(_,kind)=>payload=>calls.push({kind,payload})}));
    const packets=[null,undefined,{kind:"notice",payload:"still receiving"},{kind:"stop"}];
    const target=new Target(),lifetime=create({recv:()=>Promise.resolve(packets.shift())},target);
    try {
        for(let index=0;index<2;index++){
            const packet=await lifetime.receive();
            assert.throws(()=>route(packet),/Unknown surface packet/);
            assert.equal(calls.length,0);
        }
        route(await lifetime.receive());
        assert.deepEqual(calls,[{kind:"notice",payload:"still receiving"}]);
        assert.equal((await lifetime.receive()).kind,"stop");
        assert.equal(packets.length,0);
    } finally {
        lifetime.release();assert.equal(target.count,0);
    }
});

test("stop before receive does not start channel work",async()=>{
    const target=new Target();let revocations=0;
    const lifetime=create({recv(){assert.fail("revoked renderer received again");}},target,()=>{revocations++;});
    target.dispatchEvent(new Event("pttstella:stop"));
    assert.equal(revocations,1); // Synchronous admission revocation, before await.
    target.dispatchEvent(new Event("pttstella:stop"));
    assert.equal((await lifetime.receive()).kind,"stop");
    assert.equal(revocations,1);
    lifetime.release();assert.equal(target.count,0);
});

test("serialized transport delivers once, rejects bad JSON without retaining a waiter, and stops",async()=>{
    const target=new Target(),packets=['{"kind":"notice","payload":"a\\n中文"}','{bad','{"kind":"stop","payload":null}'];
    let revocations=0;
    const lifetime=create({recv:()=>Promise.resolve(packets.shift())},target,()=>revocations++);
    assert.equal((await lifetime.receive()).payload,"a\n中文");
    await assert.rejects(lifetime.receive(),/JSON|property|Expected/);
    assert.equal((await lifetime.receive()).kind,"stop");assert.equal(revocations,1);
    assert.equal(packets.length,0);lifetime.release();assert.equal(target.count,0);
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
    const factory=vm.runInNewContext(packetsSource+"\n"+readFileSync(new URL("../assets/surface-lifetime.js",import.meta.url),"utf8")+"\ncreateSurfaceLifetime",{Promise:TrackedPromise});
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
    assert.equal((await lifetime.receive()).kind,"stop");
    lifetime.release();assert.equal(target.count,0);
});

test("the final Eval channel survives GC through join and is released afterwards",()=>{
    const result=spawnSync(process.execPath,["--expose-gc",fileURLToPath(new URL("./probe_surface_result_gc.mjs",import.meta.url))],{encoding:"utf8",timeout:10000});
    assert.equal(result.status,0,result.stderr||String(result.error));
    assert.deepEqual(JSON.parse(result.stdout),{unretained_collected:true,retained_alive_until_join:true,released_collected:true,listeners_after_release:0});
});
