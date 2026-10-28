import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import vm from 'node:vm';
const source=readFileSync(new URL('../assets/surface-packets.js',import.meta.url),'utf8');
const {expandSurfaceFrame,createSurfaceRouter}=vm.runInNewContext(source+'\n({expandSurfaceFrame,createSurfaceRouter})');
const fixture=JSON.parse(readFileSync(new URL('../tests/fixtures/render-wire.json',import.meta.url),'utf8'));
const plain=value=>JSON.parse(JSON.stringify(value));
test('surface bootstrap decodes the initial localization before constructing its localizer',async()=>{
    const surface=readFileSync(new URL('../assets/surface.js',import.meta.url),'utf8');
    const bootstrap=surface.slice(0,surface.indexOf('const canvas='));
    const packet={kind:'localization',payload:{language:'zh-TW'}};
    let received=0;
    const context={retainSurfaceResult(){},window:{},dioxus:{recv:async()=>JSON.stringify(packet)},document:{documentElement:{}},fluent:{},createSurfaceI18n(data){assert.deepEqual(plain(data),packet);received++;}};
    await vm.runInNewContext(source+'\n(async()=>{'+bootstrap+'})()',context);
    assert.equal(received,1);assert.equal(context.document.documentElement.lang,'zh-TW');
});
test('Rust run codec expands exact Unicode, geometry, both colours and unmapped status',()=>{
    assert.deepEqual(plain(expandSurfaceFrame(fixture.compact)),fixture.expanded);
    assert.ok(!Object.hasOwn(fixture.compact,'glyphs'));
});
test('typed compact transport expands every paint consumer before its handler',()=>{
    const kinds=source.match(/const kinds=\[([\s\S]*?)\];/)[1].match(/"([^"]+)"/g).map(v=>JSON.parse(v));
    const calls=[],router=createSurfaceRouter(Object.fromEntries(kinds.map(kind=>[kind,data=>calls.push({kind,data})])));
    const paint={generation:3,presentation:4,article:{id:5},frame:fixture.compact,source_frame:fixture.compact};
    for(const [kind,payload,read] of [
        ['paint',paint,data=>data],['settings_open',{sample:paint},data=>data.sample],
        ['font_preview',{request:7,packet:paint},data=>data.packet],
        ['profile_result',{status:'imported',profile:{sample:paint}},data=>data.profile.sample]]){
        router(JSON.stringify({kind,payload}));const output=read(calls.at(-1).data);
        assert.deepEqual(plain(output.frame),fixture.expanded);assert.deepEqual(plain(output.source_frame),fixture.expanded);
        assert.equal(output.generation,3);assert.equal(output.presentation,4);assert.equal(output.article.id,5);
    }
    const good=calls.length;
    for(const patch of [{glyph_encoding:'future'}, {glyphs:[]}, {rows:0}, {cols:9000},
        {runs:[[0,0,1,7,7,false,['a',null]]]}, {runs:[[0,0,1,262144,7,false,['a']]]},
        {runs:[[0,3,2,7,7,false,['中']]]}, {runs:[[2,0,1,7,7,false,['a']]]}]){
        assert.throws(()=>router(JSON.stringify({kind:'paint',payload:{...paint,frame:{...fixture.compact,...patch}}})));
        assert.equal(calls.length,good);
    }
    // A valid primary frame with a damaged source frame is still atomic.
    assert.throws(()=>router({kind:'paint',payload:{...paint,source_frame:{...fixture.compact,cols:0}}}));
    assert.equal(calls.length,good);
});
