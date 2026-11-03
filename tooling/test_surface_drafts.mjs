import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import vm from 'node:vm';
import {translator} from './i18n_test_support.mjs';
const source=readFileSync(new URL('../assets/surface-drafts.js',import.meta.url),'utf8');
const {createDraftEditor,draftDisplay,draftSource}=vm.runInNewContext(source+'\n({createDraftEditor,draftDisplay,draftSource})',{TextEncoder});
function fixture(){
 const nodes=new Map(),sent=[],timers=new Map();let timer=0,packet={connected:true,generation:7,frame:{revision:3},input_epoch:2,draft_scope:{board:'Stella',account:'StellaTest'}};
 const node=id=>{if(!nodes.has(id))nodes.set(id,{id,open:false,hidden:false,value:'',children:[],dataset:{},style:{},selectionStart:0,selectionEnd:0,setAttribute(){},append(n){this.children.push(n);},replaceChildren(...n){this.children=n;},showModal(){this.open=true;},close(){this.open=false;},focus(){},setRangeText(text,start,end){this.value=this.value.slice(0,start)+text+this.value.slice(end);this.selectionStart=this.selectionEnd=start+text.length;}});return nodes.get(id);};
 const document={getElementById:node,createElement:()=>node('option-'+nodes.size),querySelector:()=>node('draft-dialog').open?node('draft-dialog'):null};
 const window={setTimeout:fn=>{timers.set(++timer,fn);return timer;},clearTimeout:id=>timers.delete(id)};
 const ui=createDraftEditor({document,window,tr:translator('zh-TW'),send:e=>sent.push(JSON.parse(JSON.stringify(e))),current:()=>packet,prepare(){},restore(){},changed(){},render(){},notice(){},palette:Array(16).fill('#000')});
 const event=(extra={})=>({isTrusted:true,preventDefault(){},stopPropagation(){},...extra});
 function fire(id,type='click',extra={}){const n=node('draft-'+id),binding=ui.bindings.find(([target,t])=>target===n&&t===type);assert.ok(binding,id+' '+type);binding[2](event({target:n,...extra}));}
 function open(){ui.open(event());const request=sent.at(-1).request;ui.receive({state:'list',request,scope:packet.draft_scope,entries:[]});ui.receive({state:'selected',token:1,entry:{scope:packet.draft_scope,revision:0,title:'',body:''}});}
 return {ui,node:id=>node('draft-'+id),sent,fire,event,open,tick(){const callbacks=[...timers.values()];timers.clear();callbacks.forEach(f=>f());},set(p){packet={...packet,...p};ui.update();},get:()=>packet,timers};
}
test('visible ESC editing is reversible including literal escape markers and Chinese',()=>{for(const text of ['中文\n\x1b[1;31m紅\x1b[m','␛literal␛␛', '␛\x1b[32m中','\x1b␛','\x1b\x1b','\x1be','␛e'])assert.equal(draftSource(draftDisplay(text)),text);});
test('composition never saves partial Chinese; saves coalesce and a conflict acknowledgement never replaces live text',()=>{
 const f=fixture();f.open();f.fire('dialog','compositionstart');f.node('body').value='中文';f.fire('body','input',{isComposing:true});f.tick();assert.equal(f.sent.filter(e=>e.kind==='draft_edit').length,0);
 f.fire('dialog','compositionend');f.tick();const first=f.sent.at(-1);assert.equal(first.edit.body,'中文');
 f.node('body').value='中文␛[31m紅␛[m';f.fire('body','input');f.tick();assert.equal(f.sent.filter(e=>e.kind==='draft_edit').length,1);
 f.ui.receive({state:'saved',token:1,sequence:1,conflicted:true});assert.equal(f.sent.at(-1).edit.body,'中文\x1b[31m紅\x1b[m');assert.equal(f.node('body').value,'中文␛[31m紅␛[m');
 f.ui.receive({state:'saved',token:1,sequence:2,conflicted:false});assert.equal(f.ui.snapshot(),null);
 f.ui.destroy();assert.equal(f.ui.clean(),true);assert.equal(f.timers.size,0);
});
test('disconnect retains source, native close snapshot includes debounced input and restart offers explicit restore',()=>{
 const f=fixture();f.open();f.node('body').value='斷線之前␛[32m綠色';f.fire('body','input');f.set({connected:false,generation:8,draft_scope:null});
 assert.equal(f.ui.snapshot().body,'斷線之前\x1b[32m綠色');f.tick();const save=f.sent.at(-1);assert.equal(save.kind,'draft_edit');f.ui.receive({state:'saved',token:1,sequence:save.edit.sequence});
 f.fire('close');assert.equal(f.node('dialog').open,false);assert.equal(f.node('body').value,'斷線之前␛[32m綠色');
 f.ui.open(f.event());f.ui.receive({state:'list',request:f.sent.at(-1).request,scope:{board:'Stella',account:'StellaTest'},entries:[{id:'saved',title:'本機',saved_at:1,conflict:false}]});
 assert.equal(f.node('chooser').hidden,false);assert.equal(f.sent.at(-1).kind,'draft_open');f.fire('restore');assert.equal(f.sent.at(-1).kind,'draft_choose');assert.equal(f.sent.at(-1).id,'saved');
});
test('a rejected save retains unsaved source; transfer needs a separate explicit confirmation and matching editor token',()=>{
 const f=fixture();f.open();f.node('body').value='不自動發表';f.fire('body','input');f.tick();f.ui.receive({state:'error',message:'disk full'});assert.equal(f.node('notice').textContent,'disk full');assert.equal(f.ui.snapshot().body,'不自動發表');
 f.fire('save');const seq=f.sent.at(-1).edit.sequence;f.ui.receive({state:'saved',token:1,sequence:seq});
 f.set({editor:true,draft_post_token:1});f.fire('transfer');assert.equal(f.node('confirm').hidden,false);assert.equal(f.sent.at(-1).kind,'draft_edit');
 f.fire('transfer-confirm');assert.equal(f.sent.at(-1).kind,'draft_station');assert.equal(f.sent.at(-1).transfer,true);assert.ok(!f.sent.some(e=>e.kind==='key'||e.kind==='text'));
});
test('untrusted input, stale choices and callbacks after disposal cannot save or transfer',()=>{
 const f=fixture();f.ui.open(f.event({isTrusted:false}));assert.equal(f.sent.length,0);f.open();const before=f.sent.length;f.node('body').value='fake';f.fire('body','input',{isTrusted:false});f.tick();assert.equal(f.sent.length,before);
 f.ui.destroy();f.fire('save');f.fire('transfer-confirm');f.tick();assert.equal(f.sent.length,before);
});


test('draft preview participates in dialog layout instead of covering its header and actions',()=>{
 const css=readFileSync(new URL('../assets/app.css',import.meta.url),'utf8');const rule=css.match(/#draft-canvas\{([^}]+)\}/)[1];assert.match(rule,/position:static/);assert.match(rule,/height:auto/);assert.match(rule,/inset:auto/);
 const app=readFileSync(new URL('../src/ui/app.rs',import.meta.url),'utf8');assert.match(app,/id:"draft-canvas",width:"960",height:"576"/);
});


test('a failed save after closing can reopen the retained editor for retry, and closing mid-IME preserves committed source',()=>{
 const f=fixture();f.open();f.node('body').value='已確認中文';f.fire('body','input');f.tick();f.fire('close');assert.equal(f.node('dialog').open,false);f.ui.receive({state:'error',message:'disk full'});
 f.ui.open(f.event());assert.equal(f.node('dialog').open,true);assert.equal(f.node('body').value,'已確認中文');f.fire('save');const seq=f.sent.at(-1).edit.sequence;f.ui.receive({state:'saved',token:1,sequence:seq});
 f.fire('dialog','compositionstart');f.node('body').value='已確認中文ㄅ';f.fire('body','input',{isComposing:true});assert.equal(f.ui.snapshot().body,'已確認中文');
});
