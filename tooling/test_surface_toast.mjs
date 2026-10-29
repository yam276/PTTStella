import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import vm from 'node:vm';
const context=vm.createContext({});
vm.runInContext(readFileSync(new URL('../assets/surface-toast.js',import.meta.url),'utf8'),context);
function fixture(){
 let time=0,next=0,restored=0,observer;const jobs=new Map();
 const dismiss={},inside={};const root={hidden:true,contains:v=>v===dismiss||v===inside};
 const message={textContent:''},document={activeElement:null};
 class Observer{constructor(cb){this.cb=cb;observer=this;this.disconnected=false;}observe(node,options){assert.equal(node,message);assert.equal(options.characterData,true);}disconnect(){this.disconnected=true;}}
 const toast=context.createSurfaceToast({root,message,dismiss,document,MutationObserver:Observer,restore:()=>restored++,now:()=>time,later:(f,delay)=>{const id=++next;jobs.set(id,{f,due:time+delay});return id;},cancel:id=>jobs.delete(id)});
 function advance(ms){time+=ms;for(const [id,job] of [...jobs])if(job.due<=time){jobs.delete(id);job.f();observer.cb();}}
 const show=text=>{message.textContent=text;observer.cb();};
 const event=(type,extra={})=>toast.bindings.find(b=>b[1]===type)[2]({isTrusted:true,preventDefault(){},stopPropagation(){},...extra});
 return {toast,root,message,document,dismiss,inside,jobs,advance,show,event,observer,restored:()=>restored};
}
test('plain-text notification is visible for eight seconds, with one bounded timer',()=>{
 const f=fixture();assert.equal(f.root.hidden,true);assert.equal(f.jobs.size,0);
 f.show('<img src=x onerror=bad()>\n中文通知');assert.equal(f.root.hidden,false);assert.equal(f.message.textContent,'<img src=x onerror=bad()>\n中文通知');
 assert.equal(f.jobs.size,1);f.advance(7999);assert.equal(f.root.hidden,false);f.advance(1);assert.equal(f.root.hidden,true);assert.equal(f.jobs.size,0);
});
test('a newer notification replaces the old one and receives its own full duration',()=>{
 const f=fixture();f.show('first');f.advance(5000);f.show('new');assert.equal(f.jobs.size,1);f.advance(7999);assert.equal(f.message.textContent,'new');f.advance(1);assert.equal(f.root.hidden,true);
});
test('pointer hover pauses the remaining lifetime without polling or an interval',()=>{
 const f=fixture();f.show('read this');f.advance(3000);f.event('pointerenter');assert.equal(f.jobs.size,0);f.advance(60000);assert.equal(f.root.hidden,false);
 f.event('pointerleave');f.advance(4999);assert.equal(f.root.hidden,false);f.advance(1);assert.equal(f.root.hidden,true);
});
test('focus and hover must both leave before expiration resumes',()=>{
 const f=fixture();f.show('read');f.advance(1000);f.event('pointerenter');f.event('focusin');f.event('pointerleave');assert.equal(f.jobs.size,0);
 f.event('focusout',{relatedTarget:f.dismiss});assert.equal(f.jobs.size,0);f.event('focusout',{relatedTarget:{}});f.advance(6999);assert.equal(f.root.hidden,false);f.advance(1);assert.equal(f.root.hidden,true);
});
test('only an explicit trusted dismissal restores owned focus, and the next toast still expires',()=>{
 const f=fixture();f.show('first');f.document.activeElement=f.dismiss;f.event('focusin');f.event('click',{isTrusted:false});assert.equal(f.root.hidden,false);
 f.event('click');assert.equal(f.root.hidden,true);assert.equal(f.restored(),1);f.show('second');assert.equal(f.jobs.size,1);f.advance(8000);assert.equal(f.root.hidden,true);assert.equal(f.restored(),1);
});
test('idle expiry never steals focus and shutdown disconnects observer and timer',()=>{
 const f=fixture();f.show('a');f.advance(8000);assert.equal(f.restored(),0);f.show('b');f.toast.destroy();assert.equal(f.observer.disconnected,true);assert.equal(f.jobs.size,0);assert.equal(f.toast.clean(),true);
 f.show('late');assert.equal(f.root.hidden,true);assert.equal(f.jobs.size,0);
});
