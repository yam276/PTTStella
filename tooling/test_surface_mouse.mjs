// Production pointer owner and handlers with explicit state doubles; no native-input claim.
import test from "node:test";
import assert from "node:assert/strict";
import {readFileSync} from "node:fs";
import vm from "node:vm";
const create=vm.runInNewContext(readFileSync(new URL("../assets/surface-mouse.js",import.meta.url),"utf8")+"\ncreateStationMouse");
const plain=v=>JSON.parse(JSON.stringify(v));
function environment(){
    let packet={generation:7,frame:{revision:4},input_epoch:2,connected:true,output_pending:false,
        mouse:{route:"station",enabled:true,policy_epoch:3,modes:{epoch:5,tracking:"click"},active:null}},block=false;
    const sent=[],captures=new Set(),viewport={hasPointerCapture:id=>captures.has(id),setPointerCapture:id=>captures.add(id),releasePointerCapture:id=>captures.delete(id)};
    const frames=new Map();let nextFrame=0;
    const owner=create({current:()=>packet,point:e=>e.cell??null,viewport,send:v=>sent.push(plain(v)),blocked:()=>block,prepare(){},changed(){},
        requestFrame:callback=>{const id=++nextFrame;frames.set(id,callback);return id;},cancelFrame:id=>frames.delete(id)});
    return {owner,sent,captures,viewport,get packet(){return packet;},set packet(value){packet=value;},set blocked(value){block=value;},
        event(extra={}){return {button:0,buttons:0,pointerId:9,isTrusted:true,isPrimary:true,cell:{col:4,row:3},preventDefault(){},...extra};},
        update(extra){packet={...packet,...extra};owner.update(packet);},
        flush(time){const callbacks=[...frames.values()];frames.clear();for(const callback of callbacks)callback(time);},frames};
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
        let toolsOpen=false,menuOpen=false;
        const actions=[],context={contextMenu:{isOpen:()=>menuOpen},tools:{open:()=>toolsOpen},stationMouse:{local:()=>local,wheel:()=>!local},cancelHover(){},blocked:()=>false,dragging:null,hasSelection:()=>false,
            viewport:{scrollWidth:100,clientWidth:100,scrollHeight:100,clientHeight:100},latest:{connected:true,navigation:{kind:"article_list"}},
            navigate:intent=>actions.push(intent.action),zoom:1,zoomTo:zoom=>actions.push(zoom)};
        vm.createContext(context);vm.runInContext(surface.slice(surface.indexOf("function wheel("),surface.indexOf("const zoomIn=")),context);
        const event={deltaY:120,preventDefault(){}};context.wheel(event);context.wheel({...event,ctrlKey:true});
        assert.deepEqual(actions,local?["next_page",0.75]:[0.75]);
        actions.length=0;toolsOpen=true;context.wheel(event);context.wheel({...event,ctrlKey:true});
        assert.deepEqual(actions,[]); // chrome interaction cannot scroll PTT underneath it
        toolsOpen=false;menuOpen=true;context.wheel(event);context.wheel({...event,ctrlKey:true});assert.deepEqual(actions,[]);

    }
});

const samples=env=>env.sent.filter(value=>value.kind==="station_mouse_sample");
test("terminal wheels page despite overflow while selection and editing suppress navigation",()=>{
    for(const kind of ["article_list","board_list","article_reading","document_list","main_menu","system_menu","command_help"]){
        const actions=[],context={contextMenu:{isOpen:()=>false},tools:{open:()=>false},stationMouse:{wheel:()=>false},cancelHover(){},blocked:()=>false,
            dragging:null,hasSelection:()=>false,viewport:{scrollWidth:100,clientWidth:100,scrollHeight:100,clientHeight:100},
            latest:{connected:true,navigation:{kind}},navigate:intent=>actions.push(intent.action)};
        vm.createContext(context);vm.runInContext(surface.slice(surface.indexOf("function wheel("),surface.indexOf("const zoomIn=")),context);
        const event={deltaY:120,preventDefault(){}};
        context.wheel(event);context.wheel({...event,deltaY:-120});
        assert.deepEqual(actions,["next_page","previous_page"]);
        actions.length=0;
        context.hasSelection=()=>true;context.wheel(event);
        context.hasSelection=()=>false;context.blocked=()=>true;context.wheel(event);
        context.blocked=()=>false;context.viewport.scrollHeight=1010;context.wheel(event);
        assert.deepEqual(actions,["next_page"]);actions.length=0;
        context.viewport.scrollHeight=100;context.wheel({...event,shiftKey:true});context.wheel({...event,altKey:true});
        assert.deepEqual(actions,[]);
    }
});
test("1003 coalesces a burst to one latest cell with an independent sample sequence",()=>{
    const env=environment();env.update({mouse:{...env.packet.mouse,modes:{epoch:5,tracking:"motion"}}});
    for(let col=0;col<80;col++)env.owner.move(env.event({cell:{col,row:3}}));
    assert.equal(samples(env).length,0);assert.equal(env.frames.size,1);env.flush();
    assert.equal(samples(env).length,1);
    assert.deepEqual(samples(env)[0],{kind:"station_mouse_sample",trusted:true,sample:{lease:{stamp:{generation:7,revision:4,input_epoch:2},mode_epoch:5,policy_epoch:3,transaction:80},
        sequence:1,report:{kind:"motion",col:79,row:3,button:null,modifiers:{shift:false,alt:false,ctrl:false}}}});
    assert.equal(env.owner.active(),false);
});
test("animation-frame pacing retains one latest cell rather than a half-second burst backlog",()=>{
    const env=environment();env.update({mouse:{...env.packet.mouse,modes:{epoch:5,tracking:"motion"}}});
    env.owner.move(env.event());env.flush(0);assert.equal(samples(env).length,1);
    for(let col=0;col<20;col++)env.owner.move(env.event({cell:{col,row:4}}));
    env.flush(16);assert.equal(samples(env).length,1);assert.equal(env.frames.size,1);
    env.owner.move(env.event({cell:{col:79,row:4}}));env.flush(34);
    assert.equal(samples(env).length,2);assert.equal(samples(env).at(-1).sample.report.col,79);
    assert.equal(env.frames.size,0);
});
test("1002 motion is held and ordered before release; foreign or untrusted moves never enter the buffer",()=>{
    const env=environment();env.update({mouse:{...env.packet.mouse,modes:{epoch:5,tracking:"drag"}}});
    env.owner.move(env.event());env.flush();assert.equal(samples(env).length,0);
    env.owner.down(env.event());const lease=reports(env)[0].event.lease;
    assert.equal(env.owner.down(env.event({shiftKey:true})),true);
    assert.equal(env.owner.active(),true);assert.equal(reports(env).length,1);
    for(const extra of [{isTrusted:false},{isPrimary:false},{pointerId:10},{metaKey:true}])env.owner.move(env.event({buttons:1,...extra}));
    env.flush();assert.equal(samples(env).length,0);
    env.owner.move(env.event({cell:{col:6,row:4},shiftKey:true,buttons:1}));
    env.owner.up(env.event({cell:{col:7,row:4}}));
    assert.deepEqual(env.sent.map(value=>value.kind),["station_mouse","station_mouse_sample","station_mouse"]);
    assert.deepEqual(samples(env)[0].sample.lease,lease);
    assert.equal(samples(env)[0].sample.report.button,"left");assert.equal(samples(env)[0].sample.report.modifiers.shift,true);
    assert.equal(env.frames.size,0);env.flush();assert.equal(samples(env).length,1);
});
test("missing physical up cancels the owned button and drops its buffered motion without a release",()=>{
    const env=environment();env.update({mouse:{...env.packet.mouse,modes:{epoch:5,tracking:"drag"}}});
    env.owner.down(env.event());
    env.owner.move(env.event({cell:{col:6,row:3},buttons:1}));
    assert.equal(env.frames.size,1);
    env.owner.move(env.event({cell:null,buttons:0,metaKey:true}));
    env.flush();env.owner.up(env.event());
    assert.equal(env.owner.active(),false);assert.equal(env.captures.size,0);assert.equal(env.frames.size,0);
    assert.equal(cancels(env).length,1);assert.equal(samples(env).length,0);assert.equal(reports(env).length,1);
});
test("drag motion verifies each owned physical button mask and ignores foreign pointer state",()=>{
    for(const [button,buttons,name] of [[0,1,"left"],[1,4,"middle"],[2,2,"right"]]){
        const env=environment();env.update({mouse:{...env.packet.mouse,modes:{epoch:5,tracking:"drag"}}});
        env.owner.down(env.event({button}));
        env.owner.move(env.event({cell:{col:6,row:3},buttons}));
        env.owner.move(env.event({pointerId:10,cell:null,buttons:0}));
        env.flush();assert.equal(samples(env).length,1);assert.equal(samples(env)[0].sample.report.button,name);
        assert.equal(env.owner.active(),true);
        env.owner.move(env.event({buttons:buttons===1?2:1}));env.flush();
        assert.equal(cancels(env).length,1);assert.equal(env.owner.active(),false);assert.equal(samples(env).length,1);
    }
});
test("unowned physical buttons cannot masquerade as no-button motion or flush an older hover",()=>{
    for(const buttons of [1,2,4,3,undefined,NaN]){
        const env=environment();env.update({mouse:{...env.packet.mouse,modes:{epoch:5,tracking:"motion"}}});
        env.owner.move(env.event({buttons:0}));assert.equal(env.frames.size,1);
        env.owner.move(env.event({buttons}));env.flush();
        assert.equal(samples(env).length,0);assert.equal(env.frames.size,0);assert.equal(cancels(env).length,0);
    }
});
test("pointer cancellation revokes an already queued unheld sample and drops the next buffered cell",()=>{
    const env=environment();env.update({mouse:{...env.packet.mouse,modes:{epoch:5,tracking:"motion"}}});
    env.owner.move(env.event());env.flush();assert.equal(samples(env).length,1);
    env.owner.move(env.event({cell:{col:6,row:3}}));assert.equal(env.frames.size,1);
    const context={stationMouse:env.owner,cancelHover(){},released:null,cancelDrag(){},clearSelection(){}};
    handler("pointercancel","deactivate",context)();env.flush();
    assert.deepEqual(env.sent.at(-1),{kind:"station_mouse_revoke",generation:7});
    assert.equal(samples(env).length,1);assert.equal(env.frames.size,0);assert.equal(env.owner.active(),false);
});
test("wheel normalization has one bounded latest tick, no release, and no old fractional source",()=>{
    const env=environment();
    env.owner.wheel(env.event({deltaY:20}));env.flush();assert.equal(samples(env).length,0);
    env.owner.wheel(env.event({deltaY:20}));env.flush();assert.equal(samples(env)[0].sample.report.direction,"down");
    for(let count=0;count<1000;count++)env.owner.wheel(env.event({deltaY:-120,altKey:true}));
    assert.equal(env.frames.size,1);env.flush();assert.equal(samples(env).length,2);
    assert.deepEqual(samples(env)[1].sample.report,{kind:"wheel",col:4,row:3,direction:"up",modifiers:{shift:false,alt:true,ctrl:false}});
    assert.equal(reports(env).length,0);assert.equal(env.owner.active(),false);
    env.owner.wheel(env.event({deltaY:20}));env.update({frame:{revision:5}});
    env.owner.wheel(env.event({deltaY:20}));env.flush();assert.equal(samples(env).length,2);
    env.owner.wheel(env.event({deltaY:1,deltaMode:1}));env.flush();assert.equal(samples(env).length,3);
    env.owner.wheel(env.event({deltaY:-1,deltaMode:2}));env.flush();assert.equal(samples(env).at(-1).sample.report.direction,"up");
});
test("samples are revoked on cancellation, source changes, black margins and destruction",()=>{
    for(const stop of [env=>env.owner.revoke(),env=>env.update({generation:8}),env=>env.update({frame:{revision:5}}),
        env=>env.update({mouse:{...env.packet.mouse,policy_epoch:4}}),env=>env.owner.move(env.event({cell:null})),env=>env.owner.destroy()]){
        const env=environment();env.update({mouse:{...env.packet.mouse,modes:{epoch:5,tracking:"motion"}}});
        env.owner.move(env.event());env.owner.wheel(env.event({deltaY:120}));stop(env);env.flush();
        assert.equal(samples(env).length,0);assert.equal(env.frames.size,0);
    }
    const env=environment();
    for(const extra of [{isTrusted:false},{ctrlKey:true},{shiftKey:true},{metaKey:true},{deltaY:Infinity},{cell:null}])env.owner.wheel(env.event({deltaY:120,...extra}));
    env.flush();assert.equal(samples(env).length,0);
    env.owner.destroy();assert.equal(env.owner.clean(),true);
});

test("a local Shift drag completes even if Shift is released before pointerup",()=>{
    const calls=[],context={dragging:{pointer:9,moved:true},stationMouse:{up(){assert.fail("station must not consume local drag");}},pointermove(){},pointerHit:()=>null,cellAt:()=>({col:4,row:4}),
        viewport:{hasPointerCapture:()=>true,releasePointerCapture:id=>calls.push(id)},surface:{dataset:{}},refreshKeepAlive(){}};
    handler("pointerup","click",context)({pointerId:9,shiftKey:false});assert.equal(context.dragging,null);assert.deepEqual(calls,[9]);
});

test("terminal wheel never falls back to WebView scrolling, including rejected gestures",()=>{
    for(const variant of ['normal','overflow','horizontal','selection','drag','blocked','tools','unknown','disconnected','shift','alt','meta','station']){
        let prevented=0,station=0;const actions=[];
        const context={contextMenu:{isOpen:()=>false},tools:{open:()=>variant==='tools'},stationMouse:{wheel:()=>{station++;return variant==='station';}},cancelHover(){},blocked:()=>variant==='blocked',
            dragging:variant==='drag'?{}:null,hasSelection:()=>variant==='selection',viewport:{scrollWidth:1000,clientWidth:100,scrollHeight:1000,clientHeight:100},
            latest:{connected:variant!=='disconnected',navigation:{kind:variant==='unknown'?'unknown':'article_list'}},navigate:intent=>actions.push(intent.action)};
        vm.createContext(context);vm.runInContext(surface.slice(surface.indexOf('function wheel('),surface.indexOf('const zoomIn=')),context);
        const e={deltaY:variant==='horizontal'?0:120,deltaX:120,shiftKey:variant==='shift',altKey:variant==='alt',metaKey:variant==='meta',preventDefault(){prevented++;}};
        context.wheel(e);assert.ok(prevented>0,variant);
        assert.deepEqual(actions,['normal','overflow'].includes(variant)?['next_page']:[],variant);
        if(variant==='station')assert.equal(station,1);
    }
});
test("web article wheel still uses local reading and normalizes pixel, line and page input",()=>{
    for(const [deltaMode,expected] of [[0,2],[1,48],[2,800]]){
        let prevented=0;const moves=[];
        const context={contextMenu:{isOpen:()=>false},tools:{open:()=>false},cancelHover(){},blocked:()=>false,dragging:null,hasSelection:()=>false,
            latest:{article:{id:1},navigation:{kind:'article_reading'}},viewport:{clientHeight:400},readingMove:(intent)=>moves.push(intent),stationMouse:{wheel(){assert.fail('station wheel');}},navigate(){assert.fail('station paging');}};
        vm.createContext(context);vm.runInContext(surface.slice(surface.indexOf('function wheel('),surface.indexOf('const zoomIn=')),context);
        context.wheel({deltaY:2,deltaMode,preventDefault(){prevented++;}});
        assert.equal(prevented,1);assert.equal(moves[0].delta,expected);
    }
});
