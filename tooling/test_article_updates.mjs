import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import vm from 'node:vm';
import {translator} from './i18n_test_support.mjs';
const create=vm.runInNewContext(readFileSync(new URL('../assets/article-updates.js',import.meta.url),'utf8')+'\ncreateArticleUpdates');
function setup(enabled=true){
    const node=()=>({hidden:true,textContent:'',setAttribute(){}}),root=node(),status=node(),toggle=node(),fresh=node(),sent=[],moves=[];
    let packet={generation:7,connected:true,article:{id:1,load:'complete',reply_count:10}},at=700,end=700;
    const ui=create({root,status,toggle,fresh,tr:translator('zh-TW'),current:()=>packet,send:v=>sent.push(v),bottom:()=>end,position:()=>at,move:v=>{at=v;moves.push(v);},restore(){},changed(){}});
    const paint=(count,{generation=7,id=1,bottom=end}={})=>{packet={...packet,generation,article:{id,load:'complete',reply_count:count}};const follow=ui.beforePaint(packet);end=bottom;ui.afterPaint(packet,follow);return follow;};
    const receive=(patch={})=>ui.receive({generation:7,article:1,enabled:true,available:true,busy:false,failed:false,...patch});
    const click=(target,trusted=true)=>ui.bindings.find(([n])=>n===target)[2]({isTrusted:trusted});
    ui.afterPaint(packet,false);receive({enabled});
    return {ui,root,status,toggle,fresh,sent,moves,paint,receive,click,scroll:v=>{at=v;ui.scroll();},at:()=>at};
}
test('new replies follow only at the old bottom; reading earlier retains position and offers a local jump',()=>{
    const e=setup();assert.equal(e.paint(13,{bottom:800}),true);assert.deepEqual(e.moves,[800]);assert.equal(e.fresh.hidden,true);
    e.scroll(200);assert.equal(e.paint(16,{bottom:900}),false);assert.equal(e.at(),200);assert.equal(e.fresh.hidden,false);assert.ok(e.fresh.textContent.includes('3'));
    e.click(e.fresh,false);assert.equal(e.at(),200);
    e.click(e.fresh);assert.equal(e.at(),900);assert.equal(e.fresh.hidden,true);assert.deepEqual(e.sent,[]);
});
test('pause, stale status, new article and connection changes cannot follow or send old actions',()=>{
    const e=setup();e.receive({enabled:false});assert.equal(e.paint(12,{bottom:750}),false);
    e.click(e.toggle,false);assert.equal(e.sent.length,0);e.click(e.toggle);assert.equal(e.sent.length,1);assert.equal(e.sent[0].enabled,true);
    e.receive();e.paint(3,{id:2,bottom:200});assert.equal(e.moves.length,0);assert.equal(e.fresh.hidden,true);assert.equal(e.toggle.hidden,true);
    e.click(e.toggle);assert.equal(e.sent.length,1);
    e.receive({article:2});e.receive({article:1,enabled:false});assert.equal(e.toggle.hidden,false);e.click(e.toggle);assert.equal(e.sent.at(-1).article,2);assert.equal(e.sent.at(-1).enabled,false);
    e.paint(2,{generation:8,id:3});e.click(e.toggle);assert.equal(e.sent.length,2);
});
test('no-change paint does not scroll, interaction pause persists, invalid packets and destroyed callbacks are inert',()=>{
    const e=setup();e.ui.refresh('selection');const paused=e.status.textContent;
    e.paint(10);assert.equal(e.moves.length,0);assert.equal(e.status.textContent,paused);
    e.ui.receive({generation:7,article:1,enabled:'yes'});assert.equal(e.status.textContent,paused);
    e.ui.refresh(null);assert.notEqual(e.status.textContent,paused);
    e.receive({available:false,enabled:false});assert.equal(e.toggle.hidden,true);e.click(e.toggle);assert.equal(e.sent.length,0);
    e.ui.destroy();e.receive();e.click(e.fresh);e.click(e.toggle);e.ui.scroll();assert.equal(e.root.hidden,true);assert.equal(e.sent.length,0);assert.equal(e.moves.length,0);
});

test('initial full loading is the baseline, never new replies before opting in',()=>{
    const e=setup(false);e.paint(40);assert.equal(e.fresh.hidden,true);assert.deepEqual(e.moves,[]);
    e.receive();e.scroll(200);e.paint(43);assert.ok(e.fresh.textContent.includes('3'));
    e.receive({enabled:false});e.paint(43);assert.equal(e.fresh.hidden,false,'pausing retains already unread replies');
    e.paint(2,{id:2});assert.equal(e.fresh.hidden,true,'another article has its own baseline');
});
