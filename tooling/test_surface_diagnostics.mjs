import test from "node:test";
import assert from "node:assert/strict";
import {readFileSync} from "node:fs";
import vm from "node:vm";
const create=vm.runInNewContext(readFileSync(new URL("../assets/surface-diagnostics.js",import.meta.url),"utf8")+"\ncreateSurfaceDiagnostics");
class Target extends EventTarget {
    count=0;
    addEventListener(...args){this.count++;super.addEventListener(...args);}
    removeEventListener(...args){this.count--;super.removeEventListener(...args);}
}
function setup(){
    const document=new Target(),window=new Target(),sent=[];
    let now=0,tick=null,cancelled=0;
    const owner=create({document,window,send:value=>sent.push(value),clock:()=>now,
        snapshot:()=>({flags:0x1234,generation:3,revision:7,presentation:9,scroll:24,extent:1000,
            text:"SENSITIVE ARTICLE",url:"https://private.invalid/",password:"SECRET PASSWORD"}),
        every:fn=>{tick=fn;return 42;},cancel:id=>{assert.equal(id,42);cancelled++;}});
    return {owner,document,window,sent,advance:ms=>{now+=ms;tick?.();},cancelled:()=>cancelled};
}
test("ordinary builds have no diagnostic listeners, timer or messages",()=>{
    const s=setup();s.owner.enable(false);s.owner.note("key");s.advance(10000);
    assert.equal(s.document.count+s.window.count+s.sent.length,0);
    s.owner.destroy();assert.equal(s.cancelled(),0);
});
test("the production packet router admits the explicit boolean diagnostic switch",()=>{
    const router=vm.runInNewContext(readFileSync(new URL("../assets/surface-packets.js",import.meta.url),"utf8")+"\ncreateSurfaceRouter");
    const s=setup();const route=router(new Proxy({}, {get:(_,kind)=>value=>{if(kind==="diagnostics")s.owner.enable(value);}}));
    route({kind:"diagnostics",payload:false});assert.equal(s.sent.length,0);
    route({kind:"diagnostics",payload:"yes"});assert.equal(s.sent.length,0);
    route({kind:"diagnostics",payload:true});assert.equal(s.sent[0].trace.event,"pulse");
    s.owner.destroy();assert.equal(s.window.count+s.document.count,0);
});
test("only closed numeric state is reported and original input is never read or changed",()=>{
    const s=setup();s.owner.enable(true);
    const input=new Event("input",{cancelable:true});
    Object.defineProperties(input,{data:{get(){assert.fail("read sensitive text");}},key:{get(){assert.fail("read key");}}});
    s.document.dispatchEvent(input);
    assert.equal(input.defaultPrevented,false);
    assert.equal(s.sent.at(-1).trace.event,"text");
    assert.equal(s.sent.at(-1).trace.flags,0x1234);
    assert.deepEqual(Object.keys(s.sent.at(-1).trace).sort(),["event","phase","target","flags","generation","revision","presentation","elapsed_ms","scroll","extent","suppressed"].sort());
    const text=JSON.stringify(s.sent);
    for(const secret of ["SENSITIVE","PASSWORD","private.invalid"]){assert.equal(text.includes(secret),false);}
    s.owner.destroy();assert.equal(s.document.count+s.window.count,0);assert.equal(s.cancelled(),1);
    const before=s.sent.length;s.document.dispatchEvent(input);s.advance(10000);s.owner.note("key");assert.equal(s.sent.length,before);
});
test("floods are sampled while the independent pulse retains delay and suppression evidence",()=>{
    const s=setup();s.owner.enable(true);
    for(let i=0;i<1000;i++)s.owner.note("key");
    assert.equal(s.sent.length,33); // First pulse plus 32 sampled events.
    s.advance(5400);
    assert.equal(s.sent.at(-1).trace.event,"pulse");assert.equal(s.sent.at(-1).trace.elapsed_ms,5400);
    assert.equal(s.sent.at(-1).trace.suppressed,968);
    s.owner.destroy();
});
test("background packets cannot exhaust the budget needed for mouse and keyboard evidence",()=>{
    const s=setup();s.owner.enable(true);
    for(let i=0;i<1000;i++)s.owner.note("packet",0,0,1);
    assert.equal(s.sent.length,1);
    for(let i=0;i<100;i++)s.owner.note("packet",0,0,20);
    for(let i=0;i<16;i++){s.owner.note("key");s.owner.note("wheel");}
    assert.equal(s.sent.filter(v=>v.trace.event==="packet").length,8);
    assert.equal(s.sent.filter(v=>v.trace.event==="key").length,16);
    assert.equal(s.sent.filter(v=>v.trace.event==="wheel").length,16);
    s.owner.destroy();
});
test("a trusted nonrepeating marker is local and listeners cannot multiply",()=>{
    const s=setup();s.owner.enable(true);const count=s.window.count+s.document.count;s.owner.enable(true);
    assert.equal(s.window.count+s.document.count,count);
    for(const [trusted,repeat,expected] of [[false,false,false],[true,true,false],[true,false,true]]){
        const e=new Event("keydown",{cancelable:true});
        for(const [name,value] of Object.entries({isTrusted:trusted,repeat,metaKey:true,shiftKey:true,altKey:false,code:"KeyL"}))
            Object.defineProperty(e,name,{value});
        s.window.dispatchEvent(e);assert.equal(e.defaultPrevented,expected);
    }
    assert.equal(s.sent.filter(value=>value.trace.event==="marker").length,1);
    s.owner.destroy();
});
test("a telemetry channel failure cannot escape into input or leave observers behind",()=>{
    const document=new Target(),window=new Target();let cancelled=0;
    const owner=create({document,window,clock:()=>0,every:()=>42,cancel:()=>cancelled++,snapshot:()=>({}),send:()=>{throw Error("synthetic unavailable channel");}});
    assert.doesNotThrow(()=>owner.enable(true));assert.equal(document.count+window.count,0);assert.equal(cancelled,1);
    assert.doesNotThrow(()=>owner.note("key"));owner.destroy();assert.equal(cancelled,1);
});
