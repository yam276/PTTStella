// Production pointer owner and handlers with explicit state doubles; no native-input claim.
import test from "node:test";
import assert from "node:assert/strict";
import {readFileSync} from "node:fs";
import vm from "node:vm";
const create=vm.runInNewContext(readFileSync(new URL("../assets/surface-mouse.js",import.meta.url),"utf8")+"\ncreateStationMouse");
const plain=v=>JSON.parse(JSON.stringify(v));
function environment(){
    let packet={generation:7,frame:{revision:4},input_epoch:2,connected:true,output_pending:false,
        mouse:{route:"station",enabled:true,policy_epoch:3,modes:{epoch:5},active:null}},block=false;
    const sent=[],captures=new Set(),viewport={hasPointerCapture:id=>captures.has(id),setPointerCapture:id=>captures.add(id),releasePointerCapture:id=>captures.delete(id)};
    const owner=create({current:()=>packet,point:e=>e.cell??null,viewport,send:v=>sent.push(plain(v)),blocked:()=>block,prepare(){},changed(){}});
    return {owner,sent,captures,viewport,get packet(){return packet;},set packet(value){packet=value;},set blocked(value){block=value;},
        event(extra={}){return {button:0,pointerId:9,isTrusted:true,isPrimary:true,cell:{col:4,row:3},preventDefault(){},...extra};},
        update(extra){packet={...packet,...extra};owner.update(packet);}};
}
const reports=env=>env.sent.filter(v=>v.kind==="station_mouse");
const cancels=env=>env.sent.filter(v=>v.kind==="station_mouse_cancel");

test("one original lease releases after a normal redraw, with fresh coordinates and modifiers",()=>{
    const env=environment();assert.equal(env.owner.down(env.event()),true);
    const press=reports(env)[0];
    assert.deepEqual(press,{kind:"station_mouse",trusted:true,event:{lease:{stamp:{generation:7,revision:4,input_epoch:2},mode_epoch:5,policy_epoch:3,transaction:1},report:{col:4,row:3,button:"left",action:"press",modifiers:{shift:false,alt:false,ctrl:false}}}});
    env.update({frame:{revision:8},input_epoch:3,output_pending:true,mouse:{...env.packet.mouse,active:press.event.lease}});
    assert.equal(env.owner.active(),true); // A complete station redraw may precede its paint ACK.
    env.owner.up(env.event({cell:{col:79,row:23},shiftKey:true,altKey:true,ctrlKey:true}));
    assert.deepEqual(reports(env)[1],{kind:"station_mouse",trusted:true,event:{lease:press.event.lease,report:{col:79,row:23,button:"left",action:"release",modifiers:{shift:true,alt:true,ctrl:true}}}});
    assert.equal(env.owner.active(),false);assert.equal(env.captures.size,0);assert.equal(cancels(env).length,0);
    env.owner.up(env.event());assert.equal(reports(env).length,2);
});

test("Shift overrides on press, unsupported mode blocks, and untrusted events never report",()=>{
    for(const extra of [{shiftKey:true},{isTrusted:false},{isPrimary:false},{button:3},{metaKey:true},{cell:null}]){
        const env=environment();assert.equal(env.owner.down(env.event(extra)),!!extra.shiftKey===false);
        assert.equal(reports(env).length,0);assert.equal(env.captures.size,0);
    }
    const env=environment();env.blocked=true;env.owner.down(env.event());assert.equal(reports(env).length,0);
    env.blocked=false;env.update({mouse:{...env.packet.mouse,route:"blocked"}});assert.equal(env.owner.down(env.event()),true);assert.equal(reports(env).length,0);
    env.update({mouse:{...env.packet.mouse,route:"local",enabled:false}});assert.equal(env.owner.down(env.event()),false);
});

test("all three buttons map to named reports; duplicate and foreign pointer edges are ignored",()=>{
    for(const [button,name] of ["left","middle","right"].entries()){
        const env=environment();env.owner.down(env.event({button}));env.owner.down(env.event({button}));
        env.owner.up(env.event({button,pointerId:10}));assert.equal(env.owner.active(),true);
        env.owner.up(env.event({button}));assert.equal(reports(env).length,2);
        assert.deepEqual(reports(env).map(v=>[v.event.report.button,v.event.report.action]),[[name,"press"],[name,"release"]]);
    }
});

test("lost capture, outside or wrong releases cancel once without a synthetic up",()=>{
    for(const finish of [env=>{env.captures.delete(9);env.owner.lost(env.event());},env=>env.owner.up(env.event({cell:null})),env=>env.owner.up(env.event({button:1})),env=>env.owner.up(env.event({isTrusted:false}))]){
        const env=environment();env.owner.down(env.event());finish(env);env.owner.cancel();
        assert.equal(reports(env).length,1);assert.equal(cancels(env).length,1);assert.equal(env.captures.size,0);
        env.owner.up(env.event());assert.equal(reports(env).length,1);
    }
    const env=environment();env.viewport.setPointerCapture=()=>{throw new Error("capture failed");};
    env.owner.down(env.event());assert.equal(reports(env).length,0);assert.equal(cancels(env).length,1);assert.equal(env.owner.active(),false);
});

test("mode, policy, session, unsupported route and acknowledged expiry revoke the original transaction",()=>{
    for(const update of [env=>({generation:8}),env=>({connected:false}),env=>({mouse:{...env.packet.mouse,modes:{epoch:7}}}),env=>({mouse:{...env.packet.mouse,policy_epoch:4}}),env=>({mouse:{...env.packet.mouse,route:"blocked"}}),env=>({input_epoch:4})]){
        const env=environment();env.owner.down(env.event());env.update(update(env));
        assert.equal(cancels(env).length,1);assert.equal(env.owner.active(),false);assert.equal(reports(env).length,1);
    }
    const env=environment();env.owner.down(env.event());const lease=reports(env)[0].event.lease;
    env.update({mouse:{...env.packet.mouse,active:lease}});env.update({mouse:{...env.packet.mouse,active:null}});
    assert.equal(cancels(env).length,1);
});

test("late rejection cannot cancel a newer transaction; blur and destroy revoke without replay",()=>{
    const env=environment();env.owner.down(env.event());const old=reports(env)[0].event.lease;env.owner.up(env.event());
    env.owner.down(env.event());assert.equal(reports(env)[2].event.lease.transaction,2);
    env.owner.reject(old);assert.equal(env.owner.active(),true);
    const current=reports(env)[2].event.lease;
    env.owner.reject({...current,stamp:{...current.stamp,revision:999}});assert.equal(env.owner.active(),true);
    env.owner.lost(env.event());assert.equal(env.owner.active(),true); // Late capture-loss event after a new capture.
    env.owner.revoke();assert.equal(cancels(env).length,1);assert.equal(env.sent.at(-1).kind,"station_mouse_revoke");
    env.owner.down(env.event());assert.equal(reports(env).at(-1).event.lease.transaction,3);
    env.owner.destroy();assert.equal(env.owner.clean(),true);assert.equal(env.captures.size,0);
    const count=env.sent.length;env.owner.destroy();env.owner.down(env.event());env.owner.up(env.event());env.owner.revoke();assert.equal(env.sent.length,count);
});

const surface=readFileSync(new URL("../assets/surface.js",import.meta.url),"utf8");
function handler(name,next,context){vm.createContext(context);vm.runInContext(surface.slice(surface.indexOf(`function ${name}(`),surface.indexOf(`function ${next}(`)),context);return context[name];}
test("station wheel produces no local navigation while Ctrl+wheel still zooms",()=>{
    for(const local of [false,true]){
        const actions=[],context={stationMouse:{local:()=>local},cancelHover(){},blocked:()=>false,dragging:null,hasSelection:()=>false,
            viewport:{scrollWidth:100,clientWidth:100,scrollHeight:100,clientHeight:100},latest:{connected:true,navigation:{kind:"article_list"}},
            navigate:intent=>actions.push(intent.action),zoom:1,zoomTo:zoom=>actions.push(zoom)};
        vm.createContext(context);vm.runInContext(surface.slice(surface.indexOf("function wheel("),surface.indexOf("const zoomIn=")),context);
        const event={deltaY:120,preventDefault(){}};context.wheel(event);context.wheel({...event,ctrlKey:true});
        assert.deepEqual(actions,local?["next_item",0.75]:[0.75]);
    }
});

test("a local Shift drag completes even if Shift is released before pointerup",()=>{
    const calls=[],context={dragging:{pointer:9,moved:true},stationMouse:{up(){assert.fail("station must not consume local drag");}},pointermove(){},hitAt:()=>null,cellAt:()=>({col:4,row:4}),
        viewport:{hasPointerCapture:()=>true,releasePointerCapture:id=>calls.push(id)},surface:{dataset:{}},refreshKeepAlive(){}};
    handler("pointerup","click",context)({pointerId:9,shiftKey:false});assert.equal(context.dragging,null);assert.deepEqual(calls,[9]);
});
