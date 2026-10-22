import test from 'node:test';
import assert from 'node:assert/strict';
import vm from 'node:vm';
import {readFileSync} from 'node:fs';
const create=vm.runInNewContext(readFileSync(new URL('../assets/gif-playback.js',import.meta.url),'utf8')+'\ncreateGifPlayback');
test('only the two closest visible GIFs decode, with old decoders released before replacement',()=>{
 const events=[],document={hasFocus:()=>true,visibilityState:'visible'},window={};
 const owner=create({document,window,limit:2}),entries=[];
 const item=(name,score)=>{const image={hidden:false,removeAttribute(){delete this.src;events.push([name,'stop']);}},still={hidden:false};
  const input={image,still,cost:1,url:'blob:'+name,score:()=>score.value,changed:p=>events.push([name,p])};
  const control=owner.add(input);entries.push({image,still,control,score});return control;};
 const a=item('a',{value:30}),b=item('b',{value:10}),c=item('c',{value:20});
 assert.deepEqual(entries.map(e=>e.control.playing()),[false,true,true]);
 entries[0].score.value=0;events.length=0;owner.refresh();
 assert.deepEqual(entries.map(e=>e.control.playing()),[true,true,false]);
 assert.ok(events.findIndex(e=>e[0]==='c'&&e[1]==='stop')<events.findIndex(e=>e[0]==='a'&&e[1]===true));
 const fire=(target,type)=>owner.bindings.find(b=>b[0]===target&&b[1]===type)[2]();
 fire(window,'blur');assert.ok(entries.every(e=>!e.control.playing()&&!e.image.src&&!e.still.hidden));
 fire(window,'focus');assert.equal(a.playing(),true);b.setEnabled(false);assert.equal(c.playing(),true);
 document.visibilityState='hidden';fire(document,'visibilitychange');assert.ok(entries.every(e=>!e.control.playing()));
 document.visibilityState='visible';fire(document,'visibilitychange');assert.equal(a.playing(),true);
 a.remove();assert.equal(entries[0].image.src,undefined);
 owner.destroy();assert.equal(owner.clean(),true);assert.ok(entries.every(e=>!e.image.src));
});
test('offscreen, manually paused and destroyed entries cannot restart on focus',()=>{
 const document={visibilityState:'visible'},window={},owner=create({document,window,limit:1});let score=Infinity;
 const image={removeAttribute(){delete this.src;}},still={};const entry=owner.add({image,still,url:'blob:gif',score:()=>score});
 assert.equal(entry.playing(),false);score=0;owner.refresh();assert.equal(entry.playing(),true);
 entry.setEnabled(false);owner.refresh();assert.equal(entry.playing(),false);
 owner.destroy();entry.setEnabled(true);owner.refresh();assert.equal(image.src,undefined);
 assert.equal(owner.add({image,still,url:'blob:late',score:()=>0}).playing(),false);assert.equal(owner.clean(),true);
});

test('declared decoded frame pixels share one budget, including missing legacy metadata',()=>{
 const owner=create({document:{visibilityState:'visible'},limit:2,pixelBudget:100});
 const add=(cost,score)=>owner.add({image:{removeAttribute(){delete this.src;}},still:{},url:'blob:gif',cost,score:()=>score});
 const a=add(80,0),b=add(40,1),c=add(20,2);
 assert.equal(a.playing(),true);assert.equal(b.playing(),false);assert.equal(c.playing(),true);
 a.remove();assert.equal(b.playing(),true);assert.equal(c.playing(),true);
 const unknown=add(undefined,-1);assert.equal(unknown.playing(),true);assert.equal(b.playing(),false);assert.equal(c.playing(),false);
 owner.destroy();assert.equal(owner.clean(),true);
});
