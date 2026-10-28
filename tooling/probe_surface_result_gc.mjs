// Production retention owner with an independent weak channel, like Desktop's
// FinalizationRegistry-owned evaluator. Keep the window alive throughout.
import assert from 'node:assert/strict';
import vm from 'node:vm';
import {readFileSync} from 'node:fs';
const retain=vm.runInNewContext(readFileSync(new URL('../assets/surface-lifetime.js',import.meta.url),'utf8')+'\nretainSurfaceResult');
assert.equal(typeof global.gc,'function');
class Window extends EventTarget {
    count=0;
    addEventListener(...args){super.addEventListener(...args);this.count++;}
    removeEventListener(...args){super.removeEventListener(...args);this.count--;}
}
async function collect(){for(let i=0;i<8;i++){await new Promise(setImmediate);global.gc();}}
function channel(window,pin){
    let object={request:7};const ref=new WeakRef(object);
    if(pin)retain(object,window);
    object=null;return ref;
}
const window=new Window();
const old=channel(window,false);await collect();assert.equal(old.deref(),undefined,'negative control should reproduce lost Eval');
const kept=channel(window,true);await collect();assert.ok(kept.deref(),'final result owner must survive GC before Rust join');
assert.equal(window.count,1);
// Rust has consumed the complete result; its native callback releases the pin.
window.dispatchEvent(new Event('pttstella:release-result'));
await collect();assert.equal(kept.deref(),undefined,'no channel retained after release');
assert.equal(window.count,0);
console.log(JSON.stringify({unretained_collected:true,retained_alive_until_join:true,released_collected:true,listeners_after_release:0}));
