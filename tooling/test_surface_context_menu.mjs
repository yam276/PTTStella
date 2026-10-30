import test from "node:test";
import assert from "node:assert/strict";
import {readFileSync} from "node:fs";
import vm from "node:vm";
const source=readFileSync(new URL("../assets/surface-context-menu.js",import.meta.url),"utf8");
const scope=vm.createContext({});vm.runInContext(source,scope);
const {contextMenuItems,contextMenuPosition,createSurfaceContextMenu}=scope;
const base={link:-1,selection:false,ready:true,exportReady:true,local:true,ptt:true,reading:false};
test('station editor offers paste instead of browsing tools, and retains ANSI selection copy',()=>{
    assert.deepEqual([...contextMenuItems({...base,editor:true}).keys()],['paste']);
    const selected=contextMenuItems({...base,editor:true,selection:true,selectionReady:true,rowsReady:true});
    assert.deepEqual([...selected.keys()],['copy','copy-ansi','export-rows','copy-image-rows','paste','editor-color']);
    assert.equal(contextMenuItems({...base,editor:true,local:false}).get('paste'),false);
    assert.equal(contextMenuItems({...base,editor:true,ready:false}).get('paste'),false);
});
test("items follow selected content and capabilities, without inactive feature placeholders",()=>{
    assert.deepEqual([...contextMenuItems(base).keys()],["select-all","export-page","copy-image-page","bookmarks","settings"]);
    const entries=contextMenuItems({...base,link:1,selection:true,selectionReady:false,rowsReady:false,local:false,reading:true,images:true,inlineReady:false,navigate:true,first:true,last:false});
    assert.equal(entries.get("copy"),false);assert.equal(entries.get("copy-link"),true);assert.equal(entries.get("open-link"),false);
    assert.deepEqual([...entries.keys()],["copy","copy-ansi","export-rows","copy-image-rows","copy-link","open-link","select-link"]);
    for(const name of ["inline","back","previous","next","select-all","export-page","copy-image-page","bookmarks","filters","vim","settings"])assert.equal(entries.has(name),false);
    assert.deepEqual([...contextMenuItems({...base,ptt:false}).keys()],["select-all","export-page","copy-image-page","settings"]);
});
test("menu position fits the bottom/right corner and negative coordinates",()=>{
    assert.deepEqual({...contextMenuPosition(990,790,270,400,1000,800)},{left:722,top:392});
    assert.deepEqual({...contextMenuPosition(-30,-50,270,400,1000,800)},{left:8,top:8});
});
function fixture(){
    const doc={activeElement:null},win={innerWidth:500,innerHeight:400};let identity="page-a",actions=[],restored=0,prepared=0;
    const buttons=["copy","next","settings"].map(name=>({dataset:{contextAction:name},hidden:false,disabled:false,
        focus(){doc.activeElement=this;},closest(){return this;}}));
    const root={hidden:true,style:{},querySelectorAll:()=>buttons,getBoundingClientRect:()=>({width:200,height:150}),contains:target=>buttons.includes(target)};
    const menu=createSurfaceContextMenu({root,document:doc,window:win,snapshot:()=>identity,prepare:()=>prepared++,
        execute:(action,context,e)=>actions.push([action,context.index,e.isTrusted]),restore:()=>restored++,changed(){}});
    const event=extra=>({isTrusted:true,clientX:490,clientY:390,preventDefault(){this.prevented=true;},stopPropagation(){this.stopped=true;},stopImmediatePropagation(){this.stopped=true;},...extra});
    const fire=(target,type,e)=>menu.bindings.find(b=>b[0]===target&&b[1]===type)[2](e);
    const open=(e=event())=>menu.open(e,{index:2,entries:new Map([["copy",true],["next",false],["settings",true]])});
    return {doc,win,root,buttons,menu,event,fire,open,actions,setIdentity:v=>identity=v,counts:()=>({restored,prepared})};
}
test("trusted opening, keyboard navigation skips disabled rows, one action restores focus",()=>{
    const f=fixture();f.open(f.event({isTrusted:false}));assert.equal(f.menu.isOpen(),false);
    f.open();assert.equal(f.doc.activeElement,f.buttons[0]);assert.equal(f.root.style.left,"292px");assert.equal(f.root.style.top,"242px");
    const down=f.event({key:"ArrowDown"});f.fire(f.doc,"keydown",down);assert.equal(down.stopped,true);assert.equal(f.doc.activeElement,f.buttons[2]);
    f.fire(f.doc,"keydown",f.event({key:"Enter",repeat:true}));assert.equal(f.actions.length,0);
    f.fire(f.doc,"keydown",f.event({key:"Enter"}));assert.deepEqual(f.actions,[["settings",2,true]]);assert.equal(f.menu.isOpen(),false);
    assert.equal(f.counts().restored,1);
});
test("stale context and untrusted click cannot run commands; paint invalidation closes",()=>{
    const f=fixture();f.open();f.fire(f.root,"click",f.event({target:f.buttons[0],isTrusted:false}));assert.equal(f.actions.length,0);
    f.setIdentity("page-b");f.fire(f.root,"click",f.event({target:f.buttons[0]}));assert.equal(f.actions.length,0);assert.equal(f.menu.isOpen(),false);
    f.open();f.setIdentity("page-c");f.menu.validate();assert.equal(f.root.hidden,true);
});
test("outside dismissal consumes the gesture rather than activating underlying BBS content",()=>{
    const f=fixture();f.open();const down=f.event({target:{}});f.fire(f.doc,"pointerdown",down);
    assert.equal(down.prevented,true);assert.equal(down.stopped,true);assert.equal(f.menu.isOpen(),false);
    const click=f.event({target:{}});f.fire(f.doc,"click",click);assert.equal(click.stopped,true);assert.equal(f.actions.length,0);
});
test("escape, scroll, blur, resize and destroy leave no open menu or callable actions",()=>{
    for(const reason of ["Escape","Tab","scroll","blur","resize","destroy"]){
        const f=fixture();f.open();
        if(["Escape","Tab"].includes(reason))f.fire(f.doc,"keydown",f.event({key:reason}));
        else if(reason==="destroy")f.menu.destroy();
        else f.fire(reason==="scroll"?f.doc:f.win,reason,f.event({target:{}}));
        assert.equal(f.menu.isOpen(),false,reason);assert.equal(f.actions.length,0);
        f.menu.destroy();assert.equal(f.menu.clean(),true);f.open();assert.equal(f.menu.isOpen(),false);
    }
});

test("menu copy sends only a closed Rust target, never arbitrary clipboard text",()=>{
    const surface=readFileSync(new URL("../assets/surface.js",import.meta.url),"utf8"),sent=[];
    const state={stopped:false,blocked:()=>false,contextMenuItems:()=>new Map([["copy",true],["copy-link",true],["copy-ansi",true]]),contextState:()=>({}),
        latest:{presentation:3},generation:7,revision:4,selectionCache:{request:11},send:p=>sent.push(JSON.parse(JSON.stringify(p)))};
    vm.createContext(state);vm.runInContext(surface.slice(surface.indexOf("function contextAction("),surface.indexOf("function activate(")),state);
    state.contextAction("copy",{index:2},{isTrusted:true});state.contextAction("copy-link",{index:2},{isTrusted:true});
    state.contextAction("copy-link",{index:2},{isTrusted:false});
    state.contextAction("copy-ansi",{index:2},{isTrusted:true});
    assert.deepEqual(sent,[
        {kind:"copy_text",presentation:3,generation:7,revision:4,target:{kind:"selection",request:11},trusted:true},
        {kind:"copy_text",presentation:3,generation:7,revision:4,target:{kind:"link",index:2},trusted:true},
        {kind:"copy_text",presentation:3,generation:7,revision:4,target:{kind:"selection_ansi",request:11},trusted:true},
    ]);
});

test("a cancelled dismissing gesture cannot swallow a later independent click",()=>{
    const f=fixture();f.open();f.fire(f.doc,"pointerdown",f.event({target:{}}));
    f.fire(f.doc,"pointerdown",f.event({target:{}}));
    const next=f.event({target:{}});f.fire(f.doc,"click",next);assert.equal(next.stopped,undefined);
});

test('three account policies require a visible Rust word and local PTT reading; focused replies have restore',()=>{
    const state={...base,reading:true,word:{row:5,start:3,end:8,id:'Alice'},focused:true};
    const actions=['account-mask','account-mask-row','account-allow'];
    for(const action of actions)assert.equal(contextMenuItems(state).get(action),true);
    assert.equal(contextMenuItems(state).get('reply-focus-clear'),true);
    for(const change of [{word:null},{ptt:false},{reading:false},{local:false}])for(const action of actions)assert.equal(contextMenuItems({...state,...change}).has(action),false);
});
test('context account actions send bounded coordinates and explicit policy, not arbitrary ID text',()=>{
    const surface=readFileSync(new URL('../assets/surface.js',import.meta.url),'utf8'),sent=[];
    const state={stopped:false,blocked:()=>false,contextMenuItems:()=>new Map(['account-mask','account-mask-row','account-allow','reply-focus-clear'].map(x=>[x,true])),contextState:()=>({}),
      latest:{presentation:3},stamp:()=>({generation:7,revision:4,input_epoch:2}),send:p=>sent.push(JSON.parse(JSON.stringify(p)))};
    vm.createContext(state);vm.runInContext(surface.slice(surface.indexOf('function contextAction('),surface.indexOf('function activate(')),state);
    for(const action of ['account-mask','account-mask-row','account-allow','reply-focus-clear'])state.contextAction(action,{index:-1,word:{row:5,start:3,id:'Alice'}},{isTrusted:true});
    assert.deepEqual(sent.slice(0,3).map(p=>[p.kind,p.row,p.col,p.policy]),[['account_rule',5,3,'mask'],['account_rule',5,3,'mask_row'],['account_rule',5,3,'allow']]);
    assert.equal(sent[3].row,null);assert.ok(sent.every(p=>!('id' in p)));
});

test('recognized list author offers policies without reply focus or reading navigation',()=>{
 const state={...base,authors:true,word:{row:3,start:17,end:30,id:'alice'}};
 for(const action of ['account-mask','account-mask-row','account-allow'])assert.equal(contextMenuItems(state).get(action),true);
 for(const action of ['reply-focus','reply-focus-clear','previous','next'])assert.equal(contextMenuItems(state).has(action),false);
 for(const change of [{word:null},{authors:false},{ptt:false},{local:false}])assert.equal(contextMenuItems({...state,...change}).has('account-mask'),false);
});

test('existing policy replaces only its add entry with an explicit removal action',()=>{
 for(const [policy,name] of [['mask','mask'],['mask_row','mask-row'],['allow','allow']]){
  const entries=contextMenuItems({...base,reading:true,word:{id:'Alice',policy}});
  assert.equal(entries.has('account-'+name),false);
  assert.equal(entries.get('account-remove-'+name),true);
  assert.equal([...entries.keys()].filter(k=>k.startsWith('account-')).length,3);
  assert.equal(contextMenuItems({...base,reading:true,ready:false,word:{id:'Alice',policy}}).get('account-remove-'+name),false);
 }
});
test('removal sends the matching policy and stamped cell without accepting a client ID',()=>{
 const surface=readFileSync(new URL('../assets/surface.js',import.meta.url),'utf8'),sent=[];
 const state={stopped:false,blocked:()=>false,contextState:()=>({}),contextMenuItems:()=>new Map(['mask','mask-row','allow'].map(p=>['account-remove-'+p,true])),
 latest:{presentation:3},stamp:()=>({generation:7,revision:4,input_epoch:2}),send:p=>sent.push(JSON.parse(JSON.stringify(p)))};
 vm.createContext(state);vm.runInContext(surface.slice(surface.indexOf('function contextAction('),surface.indexOf('function activate(')),state);
 for(const name of ['mask','mask-row','allow'])state.contextAction('account-remove-'+name,{word:{row:5,start:3,id:'Alice'}},{isTrusted:true});
 assert.deepEqual(sent.map(p=>[p.policy,p.remove,p.row,p.col]),[['mask',true,5,3],['mask_row',true,5,3],['allow',true,5,3]]);
 assert.ok(sent.every(p=>!('id' in p)));
});

test('image clipboard choices use the same page and row availability as file export',()=>{
 for(const ready of [true,false]){
  const entries=contextMenuItems({...base,exportReady:ready,selection:true,rowsReady:ready});
  assert.equal(entries.has('copy-image-page'),false);assert.equal(entries.get('copy-image-rows'),ready);
 }
 assert.equal(contextMenuItems(base).has('copy-image-rows'),false);
});

test('decoded pictures offer original-image actions and links without page or account commands',()=>{
 for(const ready of [true,false]){
  const entries=contextMenuItems({...base,picture:true,pictureReady:ready,link:3,selection:true,word:{id:'Alice'},reading:true,images:true,navigate:true});
  assert.deepEqual([...entries.keys()],['copy-picture','save-picture','copy-link','open-link']);
  assert.equal(entries.get('copy-picture'),ready);assert.equal(entries.get('save-picture'),ready);
 }
});
test('account and selected-text contexts omit general navigation and settings',()=>{
 for(const context of [{word:{id:'Alice'},reading:true},{selection:true,selectionReady:true,rowsReady:true}]){
  const entries=contextMenuItems({...base,...context,navigate:true,images:true});
  for(const name of ['back','previous','next','select-all','export-page','copy-image-page','bookmarks','filters','vim','settings','inline'])assert.equal(entries.has(name),false,name);
 }
});
test('original-image output carries only an acknowledged index and stamp, with no URL or pixels',()=>{
 const surface=readFileSync(new URL('../assets/surface.js',import.meta.url),'utf8'),sent=[];
 const state={stopped:false,blocked:()=>false,contextMenuItems:()=>new Map([['copy-picture',true],['save-picture',true]]),contextState:()=>({}),latest:{presentation:3},stamp:()=>({generation:7,revision:4,input_epoch:2}),send:p=>sent.push(JSON.parse(JSON.stringify(p)))};
 vm.createContext(state);vm.runInContext(surface.slice(surface.indexOf('function contextAction('),surface.indexOf('function activate(')),state);
 for(const action of ['copy-picture','save-picture'])state.contextAction(action,{index:2},{isTrusted:true});
 assert.deepEqual(sent.map(p=>[p.kind,p.index,p.destination]),[['picture',2,'clipboard'],['picture',2,'file']]);
 assert.ok(sent.every(p=>p.stamp.generation===7&&!('url'in p)&&!('png'in p)));
});

test('editor paste action requests the owned clipboard by exact stamp, never clipboard text',()=>{
    const surface=readFileSync(new URL('../assets/surface.js',import.meta.url),'utf8'),sent=[];
    const f={stopped:false,blocked:()=>false,contextMenuItems:()=>new Map([['paste',true]]),contextState:()=>({}),
      latest:{presentation:3},stamp:()=>({generation:7,revision:4,input_epoch:2}),keyboardState:{enabled:false},
      cancelComposition(){},cancelDrag(){},clearSelection(){},sendInput:p=>sent.push(JSON.parse(JSON.stringify(p)))};
    vm.createContext(f);vm.runInContext(surface.slice(surface.indexOf('function contextAction('),surface.indexOf('function activate(')),f);
    f.contextAction('paste',{index:-1},{isTrusted:false});assert.deepEqual(sent,[]);
    f.contextAction('paste',{index:-1},{isTrusted:true});
    assert.deepEqual(sent,[{kind:'paste_clipboard',presentation:3,stamp:{generation:7,revision:4,input_epoch:2},trusted:true}]);
    assert.equal(f.pastePending,true);
});

test('ordinary native editor right-click opens paste menu while browsing background still opens floating tools',()=>{
    const surface=readFileSync(new URL('../assets/surface.js',import.meta.url),'utf8');
    for(const [editor,reading] of [[true,false],[false,false],[false,true]]){
        const calls=[],f={stopped:false,composing:false,pastePending:false,document:{querySelector:()=>null},latest:{editor,reply_rows:[],navigation:{kind:reading?'article_reading':'article_list'}},
          stationMouse:{local:()=>true},filterUi:{blocked:()=>false},contextMenu:{close(){},open:()=>calls.push('menu')},
          cellAt:()=>null,linkAt:()=>-1,inlineImages:{linkAtPoint:()=>-1},accountWordAt:()=>null,
          hasSelection:()=>false,toolAccess:{right_click_menu:true},tools:{showAt:()=>calls.push('tools')},contextState:()=>({}),contextMenuItems:()=>new Map()};
        vm.createContext(f);vm.runInContext(surface.slice(surface.indexOf('function openContextMenu('),surface.indexOf('function contextAction(')),f);
        f.openContextMenu({isTrusted:true,shiftKey:false,preventDefault(){}});assert.deepEqual(calls,[editor||reading?'menu':'tools']);
    }
});

test('bookmark action agrees with full current-article availability instead of silently accepting a loading page',()=>{
 assert.equal(contextMenuItems({...base,reading:true,bookmark:false}).get('bookmark-current'),false);
 assert.equal(contextMenuItems({...base,reading:true,bookmark:true}).get('bookmark-current'),base.ready);
 assert.equal(contextMenuItems({...base,reading:true,bookmark:true,selection:true}).has('bookmark-current'),false);
});

test('article update toggle is confined to ordinary article background menus',()=>{
 const state={...base,reading:true,updates:true};
 assert.equal(contextMenuItems(state).get('article-updates'),base.ready&&base.local);
 for(const patch of [{reading:false},{updates:false},{word:{id:'Alice'}},{selection:true},{link:0},{picture:true},{editor:true}]){
  assert.equal(contextMenuItems({...state,...patch}).has('article-updates'),false);
 }
});
