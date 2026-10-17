// Production handlers with explicit event/state doubles; not native pointer proof.
import test from "node:test";
import assert from "node:assert/strict";
import {readFileSync} from "node:fs";
import vm from "node:vm";

const source=readFileSync(new URL("../assets/surface.js",import.meta.url),"utf8");
function handler(name,next,context){
    vm.createContext(context);
    vm.runInContext(source.slice(source.indexOf(`function ${name}(`),source.indexOf(`function ${next}(`)),context);
    return context[name];
}

test("right-click on either wrapped fragment requests its Rust link index",()=>{
    for(const index of [0,1]){
        const calls=[],event={preventDefault(){calls.push("prevent");}};
        const context={linkAt:()=>index,cellAt:()=>({row:8+index,col:4}),composing:false,
            cancelDrag:()=>calls.push("cancel"),focus:()=>calls.push("focus"),requestSelection:value=>calls.push(JSON.parse(JSON.stringify(value)))};
        handler("selectLink","activate",context)(event);
        assert.deepEqual(calls,["prevent","cancel","focus",{mode:"link",index}]);
    }
});

test("right-click outside a link or while composing leaves selection unchanged",()=>{
    for(const [index,composing] of [[-1,false],[0,true]]){
        const context={linkAt:()=>index,cellAt:()=>null,composing};
        handler("selectLink","activate",context)({preventDefault(){assert.fail("unexpected interception");}});
    }
});

test("link activation uses the released hit and exact frame stamp once",()=>{
    const sent=[],context={released:{hit:{kind:"link",index:1},stamp:{revision:4}},
        latest:{presentation:3},generation:2,revision:4,blocked:()=>false,sameStamp:(a,b)=>a.revision===b.revision,
        stamp:()=>({revision:4}),refreshKeepAlive(){},send:value=>sent.push(JSON.parse(JSON.stringify(value)))};
    const click=handler("click","cancelDrag",context);
    click({button:0,detail:1,isTrusted:true});click({button:0,detail:1,isTrusted:true});
    assert.deepEqual(sent,[{kind:"open_link",presentation:3,generation:2,revision:4,index:1,trusted:true}]);
    context.released={hit:{kind:"link",index:0},stamp:{revision:3}};
    click({button:0,detail:1,isTrusted:true});
    assert.equal(sent.length,1);
    context.released=null; // A moved/cancelled pointer transaction cannot activate.
    click({button:0,detail:1,isTrusted:true});assert.equal(sent.length,1);
});


test("local navigation sends an explicit pointer or toolbar source",()=>{
    const sent=[],context={canNavigate:()=>true,dragging:null,policy(){},latest:{navigation:{}},clearSelection(){},setHover(){},refreshNavigation(){},
        surface:{dataset:{}},stamp:()=>({generation:7,revision:4,input_epoch:2}),send:v=>sent.push(JSON.parse(JSON.stringify(v))),focus(){}};
    vm.createContext(context);
    vm.runInContext(source.slice(source.indexOf("function navigate("),source.indexOf("const leaveLink="))+"\nglobalThis.buttons=[browseReturn,browseUp,browseDown];",context);
    context.navigate({action:"next_item"},{isTrusted:true});
    for(const button of context.buttons)button({isTrusted:true});
    assert.deepEqual(sent.map(v=>[v.intent.action,v.source]),[["next_item","pointer"],["back","button"],["previous_page","button"],["next_page","button"]]);
    assert.ok(sent.every(v=>v.kind==="navigate"&&v.trusted&&v.stamp.generation===7));
});
