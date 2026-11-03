import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import vm from 'node:vm';
import {translator} from './i18n_test_support.mjs';
const source=readFileSync(new URL('../assets/surface-drafts.js',import.meta.url),'utf8');
function fixture(){
 let clock=10000,timer=0,packet={connected:true,generation:7,frame:{revision:3},input_epoch:2,draft_scope:{board:'Stella',account:'StellaTest'},draft_post_scope:{board:'Stella',account:'StellaTest'},editor:true};
 const nodes=new Map(),sent=[],timers=new Map();
 const node=id=>{if(!nodes.has(id))nodes.set(id,{id,open:false,hidden:false,value:'',children:[],dataset:{},style:{},attrs:{},setAttribute(k,v){this.attrs[k]=v;},append(n){this.children.push(n);},replaceChildren(...n){this.children=n;},showModal(){this.open=true;},close(){this.open=false;},focus(){},getBoundingClientRect:()=>({left:0,top:0,width:1000,height:700})});return nodes.get(id);};
 const document={getElementById:node,createElement:()=>node('option-'+nodes.size),querySelector:()=>node('draft-dialog').open?node('draft-dialog'):null};
 const window={setTimeout:(fn,ms)=>{timers.set(++timer,{fn,at:clock+ms});return timer;},clearTimeout:id=>timers.delete(id)};
 let options,value='',selection=[],mode='color',select='line',dead=false,projected=null;
 const editor={value:()=>value,load:v=>{value=v;},ranges:()=>selection,apply:changes=>{for(const c of [...changes].reverse())value=value.slice(0,c.from)+c.insert+value.slice(c.to);options.onChange();},project:d=>projected=d,mode:m=>mode=m,selection:m=>select=m,readOnly(){},composing:()=>false,focus(){},destroy(){dead=true;}};
 const createDraftEditor=vm.runInNewContext(source+'\ncreateDraftEditor',{TextEncoder,graphicCode:()=>null,Date:class extends Date{static now(){return clock;}}});
 const ui=createDraftEditor({document,window,tr:translator('zh-TW'),send:e=>sent.push(JSON.parse(JSON.stringify(e))),current:()=>packet,prepare(){},restore(){},changed(){},notice(){},palette:Array(16).fill('#000'),createEditor:o=>{options=o;return editor;}});
 const event=(extra={})=>({isTrusted:true,preventDefault(){},stopPropagation(){},...extra});
 function fire(id,type='click',extra={}){const n=node('draft-'+id),binding=ui.bindings.find(([target,t])=>target===n&&t===type);assert.ok(binding,id+' '+type);binding[2](event({target:n,...extra}));}
 function open(){ui.open(event());const request=sent.at(-1).request;ui.receive({state:'list',request,scope:packet.draft_scope,can_create:true,entries:[]});ui.receive({state:'selected',token:1,entry:{scope:packet.draft_scope,revision:0,title:'',body:''}});ui.receive({state:'projected',token:1,revision:0,decorations:[]});}
 function advance(ms){clock+=ms;let ready;do{ready=[...timers].filter(([,v])=>v.at<=clock);for(const [id,v]of ready){timers.delete(id);v.fn();}}while(ready.length);}
 return {ui,node:id=>node('draft-'+id),sent,fire,event,open,advance,set(p){packet={...packet,...p};ui.update();},timers,
 edit(v){value=v;options.onChange();},value:()=>value,select:r=>selection=r,context:()=>options.onContext(event({clientX:990,clientY:690})),mode:()=>mode,selection:()=>select,dead:()=>dead,projected:()=>projected};
}
const saves=f=>f.sent.filter(e=>e.kind==='draft_edit');
test('typing waits for an idle interval, throttles saves, and palette or internal focus never triggers disk I/O',()=>{
 const f=fixture();f.open();f.edit('中');f.advance(500);f.edit('中文');f.advance(1999);assert.equal(saves(f).length,0);f.advance(1);assert.equal(saves(f).length,1);
 f.edit('中文三');f.ui.receive({state:'saved',token:1,sequence:1,conflicted:true});f.advance(2000);assert.equal(saves(f).length,1);f.advance(3000);assert.equal(saves(f).length,2);assert.equal(saves(f)[1].edit.body,'中文三');
 f.ui.receive({state:'saved',token:1,sequence:2});assert.equal(f.ui.snapshot(),null);
 assert.ok(!f.ui.bindings.some(([n,t])=>n===f.node('dialog')&&t==='focusout'));
 f.ui.destroy();assert.ok(f.ui.clean());assert.ok(f.dead());assert.equal(f.timers.size,0);
});
test('composition never saves partial Chinese, and shutdown snapshot uses committed source',()=>{
 const f=fixture();f.open();f.edit('中文');f.fire('dialog','compositionstart');f.edit('中文ㄅ');f.advance(10000);assert.equal(saves(f).length,0);assert.equal(f.ui.snapshot().body,'中文');
 f.fire('dialog','compositionend');f.advance(2000);assert.equal(saves(f)[0].edit.body,'中文ㄅ');
});
test('disconnect retains ANSI source, close flushes pending work, and reopen explicitly offers saved drafts',()=>{
 const f=fixture();f.open();f.edit('斷線之前\x1b[32m綠色');f.set({connected:false,generation:8,draft_scope:null,draft_post_scope:null});
 assert.equal(f.ui.snapshot().body,f.value());f.fire('close');assert.equal(f.node('dialog').open,false);assert.equal(saves(f)[0].edit.body,f.value());f.ui.receive({state:'saved',token:1,sequence:1});
 f.ui.open(f.event());f.ui.receive({state:'list',request:f.sent.at(-1).request,scope:{board:'Stella',account:'StellaTest'},entries:[{id:'saved',title:'本機',saved_at:1}]});
 assert.equal(f.node('chooser').hidden,false);f.fire('restore');assert.equal(f.sent.at(-1).id,'saved');
});
test('failed save preserves live source; transfer needs separate confirmation and a native editor scope',()=>{
 const f=fixture();f.open();f.edit('不自動發表');f.fire('save');f.ui.receive({state:'error',message:'disk full'});assert.equal(f.ui.snapshot().body,'不自動發表');
 f.fire('save');f.ui.receive({state:'saved',token:1,sequence:saves(f).at(-1).edit.sequence});f.fire('transfer');assert.equal(f.node('confirm').hidden,false);f.fire('transfer-confirm');assert.equal(f.sent.at(-1).kind,'draft_station');assert.ok(!f.sent.some(e=>e.kind==='key'||e.kind==='text'));
});
test('right-click palette preserves ranges and applies immediately without saving or moving document layout',()=>{
 const f=fixture();f.open();f.edit('中文\n\n\x1b[32m第三行\x1b[m');f.select([{from:0,to:f.value().length}]);f.context();assert.equal(f.node('color-panel').hidden,false);assert.equal(f.node('color-panel').style.left,'650px');
 const b=f.node('foreground').children[9];f.select([]);f.fire('color-panel','click',{target:{closest:()=>b}});const color=f.sent.at(-1);assert.equal(color.kind,'draft_color');assert.equal(color.color.foreground,9);assert.equal(color.body,f.value());assert.equal(saves(f).length,0);
 f.ui.receive({state:'colored',token:1,revision:color.revision,changes:[{from:0,to:2,insert:'\x1b[31m中文\x1b[m'}]});assert.match(f.value(),/^\x1b\[31m中文/);assert.equal(f.node('color-panel').hidden,true);
 f.fire('mode-source');assert.equal(f.mode(),'source');f.fire('mode-plain');assert.equal(f.mode(),'plain');f.fire('select-rectangle');assert.equal(f.selection(),'rectangle');
});
test('late projections and color errors cannot overwrite live source or acknowledge an unrelated save',()=>{
 const f=fixture();f.open();f.edit('中文');f.advance(32);const p=f.sent.at(-1);assert.equal(p.kind,'draft_project');f.edit('新中文');f.fire('save');
 f.ui.receive({state:'projected',token:1,revision:p.revision,decorations:['stale']});assert.notDeepEqual(f.projected(),['stale']);
 f.ui.receive({state:'decoration_error',token:1,revision:p.revision,coloring:true,message:'color failed'});f.ui.receive({state:'saved',token:1,sequence:1});assert.equal(f.ui.snapshot(),null);assert.equal(f.value(),'新中文');
});
test('untrusted actions and callbacks after disposal cannot save or transfer',()=>{
 const f=fixture();f.ui.open(f.event({isTrusted:false}));assert.equal(f.sent.length,0);f.open();const before=f.sent.length;f.ui.destroy();f.fire('save');f.fire('transfer-confirm');f.advance(10000);assert.equal(f.sent.length,before);
});
test('native Ctrl+P is not intercepted; full-size composer has a floating palette',()=>{
 const s=readFileSync(new URL('../assets/surface.js',import.meta.url),'utf8');assert.doesNotMatch(s,/control===\s*["']p["'].*draftUi\.open/);
 const css=readFileSync(new URL('../assets/app.css',import.meta.url),'utf8');assert.match(css,/#draft-color-panel\{position:absolute/);assert.match(css,/\.draft-dialog\{width:calc\(100vw - 24px\);height:calc\(100vh - 24px\)/);
});

test('a board list with no template cannot create a new blank document',()=>{
 const f=fixture();f.ui.open(f.event());const request=f.sent.at(-1).request;f.ui.receive({state:'list',request,scope:{board:'Stella',account:'StellaTest'},can_create:false,entries:[]});
 assert.equal(f.node('new').hidden,true);assert.equal(f.sent.at(-1).kind,'draft_open');f.fire('new');assert.equal(f.sent.at(-1).kind,'draft_open');
});

test('closing while a color operation is pending cannot discard its later edit',()=>{
 const f=fixture();f.open();f.edit('中文');f.select([{from:0,to:2}]);f.context();const b=f.node('foreground').children[9];f.fire('color-panel','click',{target:{closest:()=>b}});const request=f.sent.at(-1);f.fire('close');assert.equal(f.node('dialog').open,true);
 f.ui.receive({state:'colored',token:1,revision:request.revision,changes:[{from:0,to:2,insert:'\x1b[31m中文\x1b[m'}]});f.fire('close');assert.equal(saves(f).at(-1).edit.body,'\x1b[31m中文\x1b[m');
});

test('save before choosing a recovered draft cannot dirty or lock the chooser',()=>{
 const f=fixture();f.ui.open(f.event());const request=f.sent.at(-1).request;f.ui.receive({state:'list',request,scope:{board:'Stella',account:'StellaTest'},can_create:true,entries:[{id:'saved',title:'中文',saved_at:1}]});
 assert.equal(f.node('save').disabled,true);assert.equal(f.node('transfer').disabled,true);f.fire('save');f.fire('restore');assert.equal(f.sent.at(-1).kind,'draft_choose');
});
