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

test("right-click uses the Rust link index without replacing the existing selection",()=>{
    for(const index of [0,1]){
        const calls=[],context={stationMouse:{local:()=>true},linkAt:()=>index,cellAt:()=>({row:8+index,col:4}),
            inlineImages:{linkAtPoint:()=>-1},composing:false,pastePending:false,stopped:false,latest:{output_pending:false},
            document:{querySelector:()=>null},filterUi:{blocked:()=>false},contextState:i=>i,contextMenuItems:i=>i,
            contextMenu:{close:()=>calls.push("close"),open:(e,data)=>calls.push(data.index)}};
        handler("openContextMenu","contextAction",context)({isTrusted:true,preventDefault(){}});
        assert.deepEqual(calls,["close",index]);
    }
});
test("station right-click stays remote; Shift opens local menu, and untrusted or composing events cannot",()=>{
    for(const [shiftKey,isTrusted,composing,expected] of [[false,true,false,0],[true,true,false,1],[true,false,false,0],[true,true,true,0]]){
        let opened=0;
        const context={stationMouse:{local:()=>false},linkAt:()=>-1,cellAt:()=>null,inlineImages:{linkAtPoint:()=>-1},
            composing,pastePending:false,stopped:false,latest:{},document:{querySelector:()=>null},filterUi:{blocked:()=>false},
            contextState:()=>({}),contextMenuItems:()=>new Map(),contextMenu:{close(){},open(){opened++;}}};
        handler("openContextMenu","contextAction",context)({shiftKey,isTrusted,preventDefault(){}});
        assert.equal(opened,expected);
    }
});

test("link activation uses the released hit and exact frame stamp once",()=>{
    const sent=[],context={stationMouse:{local:()=>true},released:{hit:{kind:"link",index:1},stamp:{revision:4}},
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
    const sent=[],context={stationMouse:{local:()=>true,revoke(){}},canNavigate:()=>true,dragging:null,policy(){},latest:{navigation:{}},clearSelection(){},setHover(){},refreshNavigation(){},
        surface:{dataset:{}},stamp:()=>({generation:7,revision:4,input_epoch:2}),send:v=>sent.push(JSON.parse(JSON.stringify(v))),focus(){}};
    vm.createContext(context);
    vm.runInContext(source.slice(source.indexOf("function navigate("),source.indexOf("const leaveLink="))+"\nglobalThis.buttons=[browseReturn,browseUp,browseDown];",context);
    context.navigate({action:"next_item"},{isTrusted:true});
    for(const button of context.buttons)button({isTrusted:true});
    assert.deepEqual(sent.map(v=>[v.intent.action,v.source]),[["next_item","pointer"],["back","button"],["previous_page","button"],["next_page","button"]]);
    assert.ok(sent.every(v=>v.kind==="navigate"&&v.trusted&&v.stamp.generation===7));
});


function renderedLinks(scale, links, options) {
    const rectangles=[];
    const ctx={setTransform(){},fillRect(...bounds){rectangles.push({colour:this.fillStyle,bounds});}};
    const target={width:960*scale,height:576*scale,getContext:()=>ctx};
    const context={palette:["#000000","#800000","#008000","#808000","#000080","#800080","#008080","#c0c0c0","#808080"]};
    const packet={frame:{cols:80,rows:24,glyphs:[],cursor_visible:false},links};
    handler("render","repaint",context)(target,packet,options);
    return rectangles.slice(1); // The first rectangle clears the canvas.
}

test("live links are underlined without hover, with grid-aligned wrapped spans at each scale",()=>{
    const links=[{row:3,start:8,end:20},{row:4,start:0,end:6}];
    for(const scale of [.5,1,1.25,2]) {
        const thickness=Math.max(1,Math.round(scale));
        assert.deepEqual(renderedLinks(scale,links,{linkHints:true}),[
            {colour:"#808080",bounds:[96*scale,96*scale-thickness,144*scale,thickness]},
            {colour:"#808080",bounds:[0,120*scale-thickness,72*scale,thickness]},
        ]);
        assert.deepEqual(renderedLinks(scale,[],{linkHints:true}),[]);
    }
});

test("hover brightens only its span and exports omit all local link hints",()=>{
    const links=[{row:3,start:8,end:20},{row:4,start:0,end:6}];
    assert.deepEqual(renderedLinks(1,links,{linkHints:true,hovered:1}).map(r=>r.colour),["#808080","#c0c0c0"]);
    assert.deepEqual(renderedLinks(1,links,{cursor:false,hovered:1}),[]);
    // New paint packets own their spans; removed or masked links leave no decoration.
    assert.deepEqual(renderedLinks(1,links.slice(1),{linkHints:true}),[
        {colour:"#808080",bounds:[0,119,72,1]},
    ]);
});
