// Production handlers with explicit event/state doubles; not native pointer proof.
import test from "node:test";
import assert from "node:assert/strict";
import {readFileSync} from "node:fs";
import vm from "node:vm";

const source=readFileSync(new URL("../assets/surface.js",import.meta.url),"utf8")+readFileSync(new URL("../assets/surface-renderer.js",import.meta.url),"utf8")+"\nfunction rendererEnd(){}";
function handler(name,next,context){
    vm.createContext(context);
    vm.runInContext(source.slice(source.indexOf("function navigationBackRegion("),source.indexOf("function hitAt(")),context);
    vm.runInContext(source.slice(source.indexOf("function accountWordAt("),source.indexOf("function contextState(")),context);
    vm.runInContext(source.slice(source.indexOf(`function ${name}(`),source.indexOf(`function ${next}(`)),context);
    return context[name];
}

test("right-click uses the Rust link index without replacing the existing selection",()=>{
    for(const index of [0,1]){
        const calls=[],context={stationMouse:{local:()=>true},linkAt:()=>index,cellAt:()=>({row:8+index,col:4}),
            inlineImages:{linkAtPoint:()=>-1},composing:false,pastePending:false,stopped:false,latest:{output_pending:false},
            toolAccess:{right_click_menu:false},hasSelection:()=>false,document:{querySelector:()=>null},filterUi:{blocked:()=>false},contextState:i=>i,contextMenuItems:i=>i,
            contextMenu:{close:()=>calls.push("close"),open:(e,data)=>calls.push(data.index)}};
        handler("openContextMenu","contextAction",context)({isTrusted:true,preventDefault(){}});
        assert.deepEqual(calls,["close",index]);
    }
});
test("station right-click stays remote; Shift opens local menu, and untrusted or composing events cannot",()=>{
    for(const [shiftKey,isTrusted,composing,expected] of [[false,true,false,0],[true,true,false,1],[true,false,false,0],[true,true,true,0]]){
        let opened=0;
        const context={stationMouse:{local:()=>false},linkAt:()=>-1,cellAt:()=>null,inlineImages:{linkAtPoint:()=>-1},
            composing,pastePending:false,stopped:false,latest:{},toolAccess:{right_click_menu:false},hasSelection:()=>false,document:{querySelector:()=>null},filterUi:{blocked:()=>false},
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
    const sent=[],context={navigationSequence:0,stationMouse:{local:()=>true,revoke(){}},canNavigate:()=>true,dragging:null,policy(){},latest:{navigation:{}},clearSelection(){},setHover(){},refreshNavigation(){},
        surface:{dataset:{}},stamp:()=>({generation:7,revision:4,input_epoch:2}),send:v=>sent.push(JSON.parse(JSON.stringify(v))),focus(){}};
    vm.createContext(context);
    vm.runInContext(source.slice(source.indexOf("function navigationBackRegion("),source.indexOf("function hitAt(")),context);
    vm.runInContext(source.slice(source.indexOf("function readingMove("),source.indexOf("const leaveLink="))+"\nglobalThis.buttons=[browseReturn,browseUp,browseDown];",context);
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
    handler("render","rendererEnd",context)(target,packet,options);
    return rectangles.slice(1); // The first rectangle clears the canvas.
}

test("empty right-click opens radial tools; selections, Shift and opt-out keep the contextual actions",()=>{
    for(const [selection,shiftKey,enabled,radial] of [[false,false,true,true],[true,false,true,false],[false,true,true,false],[false,false,false,false]]) {
        const calls=[],context={stationMouse:{local:()=>true},linkAt:()=>-1,cellAt:()=>({row:3,col:10}),inlineImages:{linkAtPoint:()=>-1},
            composing:false,pastePending:false,stopped:false,latest:{output_pending:false},document:{querySelector:()=>null},filterUi:{blocked:()=>false},
            toolAccess:{right_click_menu:enabled},hasSelection:()=>selection,tools:{showAt:e=>calls.push(['radial',e.isTrusted])},
            contextState:()=>({}),contextMenuItems:()=>new Map(),contextMenu:{close(){},open:()=>calls.push(['context'])}};
        handler("openContextMenu","contextAction",context)({isTrusted:true,shiftKey,clientX:300,clientY:200,preventDefault(){}});
        assert.deepEqual(calls,radial?[["radial",true]]:[["context"]]);
    }
});

test("main menu and board rows are clickable only within Rust-verified bounds",()=>{
    const context={blocked:()=>false,dragging:null,stationMouse:{local:()=>true},canNavigate:()=>true,linkAt:()=>-1,
        latest:{navigation:{kind:"main_menu",first_row:12,last_row:15,cursor_row:13}}};
    const hit=handler("hitAt","setHover",context);
    for(const row of [12,13,14,15])assert.deepEqual(JSON.parse(JSON.stringify(hit({row,col:40}))),{kind:"menu",row});
    for(const row of [0,11,16,23])assert.equal(hit({row,col:40}),null);
    assert.equal(hit({row:13,col:0}),null);assert.equal(hit({row:13,col:19}),null);
    context.latest.navigation={kind:"board_list",first_row:3,last_row:5,cursor_row:4};
    assert.equal(hit({row:3,col:79}).kind,"menu");assert.equal(hit({row:6,col:30}),null);
    context.dragging={moved:true};assert.equal(hit({row:3,col:30}),null);
});

test("nested system menu rows can open and their left quarter returns without making blank rows clickable",()=>{
    const context={blocked:()=>false,dragging:null,stationMouse:{local:()=>true},canNavigate:()=>true,linkAt:()=>-1,
        latest:{frame:{cols:80},navigation:{kind:"system_menu",first_row:10,last_row:12,cursor_row:11}}};
    const hit=handler("hitAt","setHover",context);
    assert.equal(hit({row:11,col:19}).kind,"back");
    assert.equal(hit({row:11,col:20}).kind,"menu");
    assert.equal(hit({row:13,col:40}),null);
    context.latest.navigation.kind="unknown";context.canNavigate=()=>false;assert.equal(hit({row:11,col:19}),null);
});

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


test("article rows open across their width while the left edge returns; unknown rows remain inert",()=>{
    const context={blocked:()=>false,dragging:null,stationMouse:{local:()=>true},canNavigate:()=>true,linkAt:()=>-1,
        latest:{navigation:{kind:"article_list",articles:[{row:4,start:2,end:8}]}}};
    const hit=handler("hitAt","setHover",context);
    for(const col of [17,18,20,21,30,79])assert.deepEqual(JSON.parse(JSON.stringify(hit({row:4,col}))),{kind:"article",row:4});
    assert.equal(hit({row:3,col:40}),null);
    assert.equal(hit({row:4,col:0}).kind,"back");
    assert.equal(hit({row:0,col:0}),null);assert.equal(hit({row:23,col:0}),null);
    context.latest.navigation={kind:"article_reading"};assert.equal(hit({row:10,col:0}).kind,"back");
    assert.equal(hit({row:10,col:16}).kind,"back");
    // Reading halves are covered with viewport geometry in test_article_pointer.
    context.linkAt=()=>2;assert.equal(hit({row:10,col:4}).kind,"link");context.linkAt=()=>-1;
    context.stationMouse.local=()=>false;assert.equal(hit({row:10,col:0}),null);
    context.stationMouse.local=()=>true;context.canNavigate=()=>false;assert.equal(hit({row:10,col:0}),null);
    context.canNavigate=()=>true;context.dragging={moved:true};assert.equal(hit({row:10,col:0}),null);
});

test("the left edge sends the existing back intent once and refuses a stale gesture",()=>{
    const calls=[],context={stationMouse:{local:()=>true},released:{hit:{kind:"back"},stamp:{revision:4}},
        blocked:()=>false,sameStamp:(a,b)=>a.revision===b.revision,stamp:()=>({revision:4}),refreshKeepAlive(){},
        navigate:(intent,e)=>calls.push([intent.action,e.isTrusted])};
    const click=handler("click","cancelDrag",context);
    click({button:0,detail:1,isTrusted:true});click({button:0,detail:1,isTrusted:true});
    assert.deepEqual(calls,[["back",true]]);
    context.released={hit:{kind:"back"},stamp:{revision:3}};click({button:0,detail:1,isTrusted:true});assert.equal(calls.length,1);
});

test("hover keeps full-row width while Back hides the row marker",()=>{
    const context={surface:{dataset:{}},navigationMarker:{style:{},dataset:{}},hoveredRow:{row:4,start:0,kind:"article"},
        latest:{frame:{cols:80},navigation:{kind:"article_list",articles:[{row:3}]}},grid:{ratio:2,x:x=>x*24,y:y=>y*48,bottom:y=>(y+1)*48},canNavigate:()=>true};
    const draw=handler("drawNavigationMarker","refreshNavigation",context);draw();
    assert.equal(context.navigationMarker.style.left,"0px");assert.equal(context.navigationMarker.style.width,"960px");
    assert.equal(context.navigationMarker.style.height,"24px");
    context.hoveredRow=null;context.surface.dataset.pointerAction="back";draw();
    assert.equal(context.navigationMarker.hidden,true);
    context.canNavigate=()=>false;draw();assert.equal(context.navigationMarker.hidden,true);
    const css=readFileSync(new URL("../assets/app.css",import.meta.url),"utf8");
    assert.match(css,/@media \(prefers-reduced-motion: reduce\) \{ #navigation-marker \{ animation: none/);
});

test("mail and archive rows use only verified item targets, retain links and quarter-width Back",()=>{
 const context={blocked:()=>false,dragging:null,stationMouse:{local:()=>true},canNavigate:()=>true,linkAt:()=>-1,
 latest:{frame:{cols:80},navigation:{kind:"document_list",first_row:3,last_row:4,cursor_row:4}}};
 const hit=handler("hitAt","setHover",context);
 for(const category of ["mail","archive"]){context.latest.navigation.category=category;
  assert.equal(hit({row:4,col:19}).kind,"back");assert.equal(hit({row:4,col:20}).kind,"menu");assert.equal(hit({row:5,col:20}),null);
 }
 context.linkAt=()=>0;assert.equal(hit({row:4,col:19}).kind,"link");
 context.dragging={moved:true};assert.equal(hit({row:4,col:20}),null);
});

test('article Back ends before the author and leaves the operation/header rows inert',()=>{
 const context={blocked:()=>false,dragging:null,stationMouse:{local:()=>true},canNavigate:()=>true,linkAt:()=>-1,latest:{navigation:{kind:'article_list',articles:[{row:3},{row:4}]}}};
 const hit=handler('hitAt','setHover',context);
 for(const col of [0,8,11,16])assert.equal(hit({row:3,col}).kind,'back');
 for(const col of [17,20,29,30,79])assert.equal(hit({row:3,col}).kind,'article');
 for(const row of [0,1,2])assert.equal(hit({row,col:0}),null);
 context.latest.navigation={kind:'board_list',first_row:3,last_row:5};assert.equal(hit({row:3,col:9}).kind,'back');assert.equal(hit({row:3,col:10}).kind,'menu');
});

test("ordinary right-click on text URLs or inline images takes precedence over radial tools and account words",()=>{
    for(const [textIndex,imageIndex,point] of [[0,-1,{row:3,col:10}],[2,-1,{row:3,col:10}],[-1,0,null]]){
        for(const selection of [false,true]){
            const calls=[],context={stationMouse:{local:()=>true},linkAt:()=>textIndex,cellAt:()=>point,inlineImages:{linkAtPoint:()=>imageIndex},
                composing:false,pastePending:false,stopped:false,latest:{output_pending:false,account_words:[{row:3,start:0,end:40,id:"example"}],reply_rows:[3]},
                document:{querySelector:()=>null},filterUi:{blocked:()=>false},toolAccess:{right_click_menu:true},hasSelection:()=>selection,
                tools:{showAt:()=>calls.push("radial")},contextState:(index,word,replyRow)=>({index,word,replyRow}),contextMenuItems:s=>s,
                contextMenu:{close(){},open:(event,data)=>calls.push(JSON.parse(JSON.stringify(data)))}};
            handler("openContextMenu","contextAction",context)({isTrusted:true,shiftKey:false,preventDefault(){}});
            const expected=Math.max(textIndex,imageIndex);
            assert.deepEqual(calls,[{index:expected,word:null,replyRow:null,entries:{index:expected,word:null,replyRow:null}}]);
        }
    }
});
test("non-link account words still open account actions while output, paste and filter transactions block both menus",()=>{
    for(const [change,expected] of [[{},1],[{output_pending:true},0],[{pastePending:true},0],[{filterBlocked:true},0]]){
        let opened=0;
        const context={stationMouse:{local:()=>true},linkAt:()=>-1,cellAt:()=>({row:3,col:10}),inlineImages:{linkAtPoint:()=>-1},
            composing:false,pastePending:change.pastePending??false,stopped:false,latest:{output_pending:change.output_pending??false,account_words:[{row:3,start:4,end:12,id:"Alice"}]},
            document:{querySelector:()=>null},filterUi:{blocked:()=>change.filterBlocked??false},toolAccess:{right_click_menu:true},hasSelection:()=>false,
            tools:{showAt:()=>assert.fail("account must not open tools")},contextState:()=>({}),contextMenuItems:()=>new Map(),
            contextMenu:{close(){},open:(event,data)=>{opened++;assert.equal(data.word.id,"Alice");assert.equal(data.index,-1);}}};
        handler("openContextMenu","contextAction",context)({isTrusted:true,preventDefault(){}});assert.equal(opened,expected);
    }
});
