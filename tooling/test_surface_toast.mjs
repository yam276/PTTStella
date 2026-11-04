import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import vm from 'node:vm';
const context=vm.createContext({});
vm.runInContext(readFileSync(new URL('../assets/surface-toast.js',import.meta.url),'utf8'),context);
function fixture(reduced=false){
 let time=0,next=0,restored=0;const jobs=new Map(),listeners=new Set(),callbacks=[];
 const dismiss={},inside={},root={hidden:true,dataset:{},style:{},contains:v=>v===dismiss||v===inside};
 const message={textContent:''},document={activeElement:null};
 const motion={matches:reduced,addEventListener:(type,f)=>{assert.equal(type,'change');listeners.add(f);},removeEventListener:(type,f)=>listeners.delete(f)};
 const toast=context.createSurfaceToast({root,message,dismiss,document,motion,restore:()=>restored++,now:()=>time,later:(f,delay)=>{const id=++next;callbacks.push(f);jobs.set(id,{f,due:time+delay});return id;},cancel:id=>jobs.delete(id)});
 function advance(ms){const end=time+ms;for(;;){const next=[...jobs].filter(([,j])=>j.due<=end).sort((a,b)=>a[1].due-b[1].due)[0];if(!next)break;time=next[1].due;jobs.delete(next[0]);next[1].f();}time=end;}
 const event=(type,extra={})=>toast.bindings.find(b=>b[1]===type)[2]({isTrusted:true,preventDefault(){},stopPropagation(){},...extra});
 return {toast,root,message,document,dismiss,inside,jobs,callbacks,advance,event,listeners,show:toast.show,motion:value=>{motion.matches=value;for(const f of listeners)f();},restoreCount:()=>restored};
}
test('copy stays readable for two seconds then fades for half a second; save uses three seconds',()=>{
 for(const [kind,delay] of [['copy',2000],['success',3000]]){
  const f=fixture();f.show('<img src=x> 中文通知',kind);assert.equal(f.message.textContent,'<img src=x> 中文通知');assert.equal(f.jobs.size,1);
  f.advance(delay-1);assert.equal(f.root.dataset.fading,'false');f.advance(1);assert.equal(f.root.dataset.fading,'true');assert.equal(f.root.style.transitionDuration,'500ms');
  f.advance(499);assert.equal(f.root.hidden,false);f.advance(1);assert.equal(f.root.hidden,true);assert.equal(f.jobs.size,0);assert.equal(f.restoreCount(),0);
 }
});
test('unclassified notices and errors remain until explicitly dismissed; successes cannot replace an error',()=>{
 for(const kind of ['persistent','error']){const f=fixture();f.show('needs attention',kind);f.advance(600000);assert.equal(f.root.hidden,false);assert.equal(f.jobs.size,0);}
 const f=fixture();f.show('save failed','error');f.show('copied','copy');f.show('saved','success');assert.equal(f.message.textContent,'save failed');assert.equal(f.root.dataset.level,'error');
 f.event('click');f.show('copied','copy');f.advance(2500);assert.equal(f.root.hidden,true);
});
test('a replacement including identical text gets its own lifetime and revoked callbacks cannot hide it',()=>{
 const f=fixture();f.show('saved','success');const old=f.callbacks.at(-1);f.advance(2900);f.show('saved','success');old();assert.equal(f.jobs.size,1);f.advance(2999);assert.equal(f.root.dataset.fading,'false');f.advance(1);
 const fading=f.callbacks.at(-1);f.show('new failure','error');fading();assert.equal(f.message.textContent,'new failure');assert.equal(f.root.hidden,false);assert.equal(f.jobs.size,0);
});
test('hover pauses both readable and fade time, and focus must also leave before resuming',()=>{
 const f=fixture();f.show('copy','copy');f.advance(1000);f.event('pointerenter');f.advance(60000);assert.equal(f.jobs.size,0);f.event('focusin');f.event('pointerleave');assert.equal(f.jobs.size,0);
 f.event('focusout',{relatedTarget:f.dismiss});assert.equal(f.jobs.size,0);f.event('focusout',{relatedTarget:{}});f.advance(1000);assert.equal(f.root.dataset.fading,'true');f.advance(200);
 f.event('pointerenter');assert.equal(f.root.dataset.fading,'false');f.advance(60000);f.event('pointerleave');assert.equal(f.root.style.transitionDuration,'300ms');f.advance(299);assert.equal(f.root.hidden,false);f.advance(1);assert.equal(f.root.hidden,true);
});
test('reduced motion skips fading, including changing preference while a fade is paused',()=>{
 const f=fixture(true);f.show('copy','copy');f.advance(1999);assert.equal(f.root.hidden,false);f.advance(1);assert.equal(f.root.hidden,true);
 const g=fixture();g.show('save','success');g.advance(3100);g.event('pointerenter');g.motion(true);assert.equal(g.root.hidden,false);assert.equal(g.jobs.size,0);g.event('pointerleave');assert.equal(g.root.hidden,true);
});
test('trusted dismissal alone restores owned focus; teardown removes timer and motion listener',()=>{
 const f=fixture();f.show('error','error');f.document.activeElement=f.dismiss;f.event('focusin');f.event('click',{isTrusted:false});assert.equal(f.root.hidden,false);
 f.event('click');assert.equal(f.restoreCount(),1);f.document.activeElement=null;f.show('copy','copy');f.advance(2100);const stale=f.callbacks.at(-1);f.toast.destroy();stale();
 assert.equal(f.jobs.size,0);assert.equal(f.listeners.size,0);assert.equal(f.toast.clean(),true);f.show('late','success');assert.equal(f.root.hidden,true);assert.equal(f.restoreCount(),1);
});
test('duplicate resume events cannot create timers, and a dismissed hovered toast cannot pause a later one',()=>{
 const f=fixture();f.show('copy','copy');f.event('pointerleave');f.event('focusout',{relatedTarget:null});assert.equal(f.jobs.size,1);
 f.event('pointerenter');f.event('click');f.show('next','copy');assert.equal(f.jobs.size,1);f.advance(2500);assert.equal(f.root.hidden,true);
});
test('production feedback routes use status rather than translated text and keep autosaves inline',async()=>{
 const source=readFileSync(new URL('../assets/surface.js',import.meta.url),'utf8');
 const calls=[],saved=[],profiles=[];const env={surface:{dataset:{}},settingsUi:{saved:v=>saved.push(v),profile:v=>profiles.push(v)},showNotice:(...args)=>calls.push(args),refreshExportControls(){},restoreEditingFocus(){}};
 for(const key of ['settings_saved','profile_result','export_result']){
  const line=source.split('\n').find(line=>line.trimStart().startsWith(key+':'));
  const handler=vm.runInNewContext('({'+line.trim()+'})',env)[key];
  if(key==='settings_saved'){await handler({status:'saved',notice:'same'});assert.equal(calls.length,0);await handler({status:'rejected',notice:'failure'});assert.equal(calls.at(-1)[0],'failure');}
  if(key==='profile_result'){await handler({status:'exported',notice:'export'});assert.deepEqual(calls.at(-1),['export','success']);await handler({status:'rejected',notice:'export failed'});assert.deepEqual(calls.at(-1),['export failed','error']);}
  if(key==='export_result')for(const [status,kind]of [['copied','copy'],['saved','success'],['failed','error']]){await handler({status,notice:'unrelated language'});assert.deepEqual(calls.at(-1),['unrelated language',kind]);}
 }
 assert.equal(saved.length,2);assert.equal(profiles.length,2);
});
