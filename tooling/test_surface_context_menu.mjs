import test from "node:test";
import assert from "node:assert/strict";
import {readFileSync} from "node:fs";
import vm from "node:vm";
const source=readFileSync(new URL("../assets/surface-context-menu.js",import.meta.url),"utf8");
const scope=vm.createContext({});vm.runInContext(source,scope);
const {contextMenuItems,contextMenuPosition,createSurfaceContextMenu}=scope;
const base={link:-1,selection:false,ready:true,exportReady:true,local:true,ptt:true,reading:false};
test("items follow selected content and capabilities, without inactive feature placeholders",()=>{
    assert.deepEqual([...contextMenuItems(base).keys()],["select-all","export-page","bookmarks","filters","vim","settings"]);
    const entries=contextMenuItems({...base,link:1,selection:true,selectionReady:false,rowsReady:false,local:false,reading:true,images:true,inlineReady:false,navigate:true,first:true,last:false});
    assert.equal(entries.get("copy"),false);assert.equal(entries.get("copy-link"),true);assert.equal(entries.get("open-link"),false);
    assert.equal(entries.get("inline"),false);assert.equal(entries.get("previous"),false);assert.equal(entries.get("next"),true);
    assert.deepEqual([...contextMenuItems({...base,ptt:false}).keys()],["select-all","export-page","settings"]);
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
    const state={stopped:false,blocked:()=>false,contextMenuItems:()=>new Map([["copy",true],["copy-link",true]]),contextState:()=>({}),
        latest:{presentation:3},generation:7,revision:4,selectionCache:{request:11},send:p=>sent.push(JSON.parse(JSON.stringify(p)))};
    vm.createContext(state);vm.runInContext(surface.slice(surface.indexOf("function contextAction("),surface.indexOf("function activate(")),state);
    state.contextAction("copy",{index:2},{isTrusted:true});state.contextAction("copy-link",{index:2},{isTrusted:true});
    state.contextAction("copy-link",{index:2},{isTrusted:false});
    assert.deepEqual(sent,[
        {kind:"copy_text",presentation:3,generation:7,revision:4,target:{kind:"selection",request:11},trusted:true},
        {kind:"copy_text",presentation:3,generation:7,revision:4,target:{kind:"link",index:2},trusted:true},
    ]);
});

test("a cancelled dismissing gesture cannot swallow a later independent click",()=>{
    const f=fixture();f.open();f.fire(f.doc,"pointerdown",f.event({target:{}}));
    f.fire(f.doc,"pointerdown",f.event({target:{}}));
    const next=f.event({target:{}});f.fire(f.doc,"click",next);assert.equal(next.stopped,undefined);
});
