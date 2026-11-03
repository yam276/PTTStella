import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import vm from 'node:vm';
import {translator} from './i18n_test_support.mjs';
const create=vm.runInNewContext(readFileSync(new URL('../assets/reply-navigation.js',import.meta.url),'utf8')+'\ncreateReplyNavigation');
const data=()=>({generation:3,ptt_features:true,article:{id:7},navigation:{kind:'article_reading'},reply_navigation:{author_rows:[3,8,18],mine_rows:[5,6],focused_rows:[11],marks:[{row:3,author:true,mine:false,first:true},{row:5,author:false,mine:true,first:true},{row:6,author:false,mine:true,first:false},{row:8,author:true,mine:true,first:true},{row:18,author:true,mine:false,first:true}]}});
function fixture(){
 let packet=data(),ready=true,visible={first:0,last:Infinity};const nodes=new Map(),jumps=[],focus=[],calls=[];
 const node=id=>{if(!nodes.has(id))nodes.set(id,{id,hidden:true,style:{},dataset:{},children:[],attrs:{},setAttribute(k,v){this.attrs[k]=v;},focus(){document.activeElement=this;focus.push(id);},contains(v){return v===this||v?.id?.startsWith('reply-navigation-');},append(n){this.children.push(n);n.parentElement=this;},remove(){this.parentElement.children=this.parentElement.children.filter(x=>x!==this);},getBoundingClientRect:()=>({left:0,top:0})});return nodes.get(id);};
 const document={activeElement:null,getElementById:node,createElement:()=>({dataset:{},style:{},setAttribute(){},remove(){this.parentElement.children=this.parentElement.children.filter(x=>x!==this);}})};
 node('reply-markers').parentElement=node('host');const choices=['author','mine','focused'].map(value=>{const n=node('reply-navigation-'+value);n.dataset.replyGroup=value;return n;});node('reply-navigation-group').querySelectorAll=()=>choices;
 const ui=create({document,tr:translator('zh-TW'),current:()=>packet,allowed:()=>ready,prepare:()=>calls.push('prepare'),restore:()=>calls.push('restore'),changed:()=>calls.push('changed'),anchor:()=>visible,jump:m=>jumps.push(m.row)});
 function fire(id,type='click',extra={}){const e={target:node(id),currentTarget:node(id),isTrusted:true,preventDefault(){},stopPropagation(){},...extra};const binding=ui.bindings.find(([n,t])=>n===node(id)&&t===type);binding[2](e);}
 ui.update(packet);
 return {ui,node,fire,jumps,focus,calls,set(p){packet=p;ui.update(p);},packet:()=>packet,anchor(v){visible=v;},ready(v){ready=v;}};
}
test('author/me/focused routes wrap only explicit clicks and never send station input',()=>{
 const f=fixture();assert.equal(f.ui.open({isTrusted:false}),false);assert.equal(f.ui.open({isTrusted:true}),true);assert.equal(f.focus.at(-1),'reply-navigation-author');
 f.fire('reply-navigation-next');f.fire('reply-navigation-next');f.fire('reply-navigation-prev');assert.deepEqual(f.jumps,[3,8,3]);
 f.fire('reply-navigation-mine');f.fire('reply-navigation-prev');assert.equal(f.jumps.at(-1),6);assert.equal(f.node('reply-navigation-count').textContent,'第 2／2 則');
 f.fire('reply-navigation-focused');f.fire('reply-navigation-next');assert.equal(f.jumps.at(-1),11);
 f.ready(false);f.fire('reply-navigation-next');f.ready(true);f.fire('reply-navigation-next','click',{isTrusted:false});assert.equal(f.jumps.length,5);
});
test('rule updates never jump, hidden rows disappear from navigation and article switches close stale controls',()=>{
 const f=fixture();f.ui.open({isTrusted:true});f.fire('reply-navigation-next');const p=data();p.reply_navigation.author_rows=[18];p.reply_navigation.marks=[];f.set(p);assert.deepEqual(f.jumps,[3]);assert.equal(f.node('reply-navigation-count').textContent,'第 0／1 則');
 f.set({...data(),article:{id:8}});assert.equal(f.node('reply-navigation').hidden,true);f.fire('reply-navigation-next');assert.deepEqual(f.jumps,[3]);
 f.ui.open({isTrusted:true});f.set({...data(),generation:4});assert.equal(f.ui.isOpen(),false);
 f.set({});assert.equal(f.ui.open({isTrusted:true}),false);f.ui.destroy();assert.equal(f.ui.clean(),true);
});
test('sidebars use verified blank separator column, show both roles, clip at footer and never alter glyphs',()=>{
 const f=fixture(),p=data(),before=JSON.stringify(p),grid={ratio:1,x:c=>c*12,bottom:r=>(r+1)*24,footerHeight:24,reading:{rowTop:r=>r*24,clippedAt:()=>-1}};
 const surface={getBoundingClientRect:()=>({left:0,top:0})},viewport={getBoundingClientRect:()=>({top:0,bottom:300})};
 f.ui.layout(p,grid,surface,viewport);const nodes=f.node('reply-markers').children;assert.equal(nodes.length,4);assert.equal(nodes[0].style.left,'25px');assert.equal(nodes[3].dataset.role,'both');assert.equal(JSON.stringify(p),before);
 const moved={getBoundingClientRect:()=>({left:60,top:-120})};f.ui.layout(p,grid,moved,viewport);assert.ok(f.node('reply-markers').children.every(m=>parseFloat(m.style.top)>=0));
 f.set({...p,article:{id:8}});f.ui.destroy();assert.equal(f.node('reply-markers').children.length,0);assert.equal(f.ui.clean(),true);
});
test('Escape restores BBS focus; late callbacks after destruction cannot reopen or navigate',()=>{
 const f=fixture();f.ui.open({isTrusted:true});f.fire('reply-navigation','keydown',{key:'Escape'});assert.equal(f.ui.isOpen(),false);assert.ok(f.calls.includes('restore'));
 f.ui.destroy();f.ui.open({isTrusted:true});f.fire('reply-navigation-next');assert.deepEqual(f.jumps,[]);assert.equal(f.ui.clean(),true);
});


test('4000-row marker geometry visits only a bounded viewport range rather than walking all replies per scroll',()=>{
 const f=fixture(),p=data();p.reply_navigation.marks=Array.from({length:4000},(_,row)=>({row,author:true,mine:false,first:true}));let visits=0;
 const grid={ratio:2,x:c=>c*24,bottom:r=>(r+1)*48,footerHeight:24,reading:{rowTop:r=>{visits++;return r*48;},clippedAt:()=>-1}};
 f.ui.layout(p,grid,{getBoundingClientRect:()=>({left:0,top:-24000})},{getBoundingClientRect:()=>({top:0,bottom:600})});
 assert.ok(visits<90,visits);assert.ok(f.node('reply-markers').children.length<=24);f.ui.destroy();assert.equal(f.ui.clean(),true);
});

test('first jump starts near the current reading position and manual scrolling does not snap back to the first reply',()=>{
 const f=fixture();f.anchor({first:9,last:20});f.ui.open({isTrusted:true});f.fire('reply-navigation-next');assert.equal(f.jumps.at(-1),18);
 f.anchor({first:4,last:12});f.fire('reply-navigation-next');assert.equal(f.jumps.at(-1),8);
 f.anchor({first:15,last:20});f.fire('reply-navigation-mine');f.fire('reply-navigation-prev');assert.equal(f.jumps.at(-1),6);
});


test('the navigation-tool entry can open its own panel while tool chrome is open, but jumping still observes the interaction gate',()=>{
 const text=readFileSync(new URL('../assets/surface.js',import.meta.url),'utf8');
 const block=text.slice(text.indexOf('replyNavigator=createReplyNavigation('),text.indexOf('const bindings=[...replyNavigator.bindings'));
 let prepared=0,received;const c={document:{},tr(){},latest:{connected:true},stopped:false,blocked:()=>false,tools:{busy:()=>true,close:()=>prepared++},articleFind:null,cancelDrag(){},clearSelection(){},cancelHover(){},focus(){},findArticleMatch(){},refreshKeepAlive(){},createReplyNavigation:args=>{received=args;return{};}};
 vm.runInNewContext(block,c);assert.equal(received.allowed(true),true);assert.equal(received.allowed(),false);received.prepare({isTrusted:true});assert.equal(prepared,1);
});
