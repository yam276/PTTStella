import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import vm from 'node:vm';
import {translator} from './i18n_test_support.mjs';

const source=readFileSync(new URL('../assets/article-find.js',import.meta.url),'utf8');
const style={fg:7,bg:0,bright:false,inverse:false};
const glyph=(row,col,text,width=1,left=style,right=left)=>({row,col,text,width,left,right});
const packet=(patch={})=>({generation:1,presentation:3,input_epoch:2,ptt_features:true,navigation:{kind:'article_reading'},article:{id:1},frame:{revision:7,rows:41,cols:80,glyphs:[]},...patch});
const hits=[{row:0,start:0,end:5,index:0},{row:0,start:10,end:15,index:1},{row:39,start:0,end:5,index:2}];
function environment(initial=packet()){
    const nodes=new Map(),timers=new Map(),jumps=[],actions=[],sent=[];let sequence=0,current=initial,allowed=true,version=0,dataVersion=0;
    const document={getElementById(id){if(!nodes.has(id))nodes.set(id,{id,hidden:true,value:'',textContent:'',attrs:{},setAttribute(k,v){this.attrs[k]=v;},focus(){actions.push('focus');},select(){actions.push('select');},contains(node){return node.id.startsWith('article-find');}});return nodes.get(id);}};
    const create=vm.runInNewContext(source+'\ncreateArticleFind',{setTimeout:fn=>{timers.set(++sequence,fn);return sequence;},clearTimeout:id=>timers.delete(id)});
    const ui=create({document,tr:translator('zh-TW'),current:()=>current,allowed:()=>allowed,prepare:()=>actions.push('prepare'),restore:()=>actions.push('restore'),
        redraw:()=>actions.push('draw'),changed:()=>actions.push('changed'),jump:m=>jumps.push(JSON.parse(JSON.stringify(m))),send:m=>sent.push(JSON.parse(JSON.stringify(m)))});
    const node=id=>document.getElementById('article-find'+(id?'-'+id:''));
    const event=(extra={})=>({isTrusted:true,key:'f',code:'KeyF',ctrlKey:true,target:node('input'),preventDefault(){this.prevented=true;},stopPropagation(){this.stopped=true;},...extra});
    const fire=(target,type,extra={})=>{const e=event(extra),binding=ui.bindings.find(([t,k])=>t===target&&k===type);binding[2](e);return e;};
    const key=extra=>fire(document,'keydown',extra);
    const edit=(value,extra={})=>{node('input').value=value;return fire(node('input'),'input',extra);};
    const reply=(patch={})=>{const s={generation:current.generation,revision:current.frame.revision,presentation:current.presentation??0,input_epoch:current.input_epoch??0,article:current.article?.id??null};
        const data={request:sent.at(-1).request,version:++version,data_version:++dataVersion,opened:true,source:s,hits,active:0,total:3,limited:false,jump:true,...patch};ui.receive(data);return data;};
    ui.update(initial);
    return {ui,node,key,edit,fire,sent,jumps,actions,timers,reply,set(p){current=p;ui.update(p);},allow(v){allowed=v;},tick(){for(const [id,fn]of [...timers]){timers.delete(id);fn();}}};
}
test('trusted Ctrl/Cmd F uses typed local intent; fullscreen and station slash remain untouched',()=>{
    const e=environment();assert.equal(e.key({}).prevented,true);assert.equal(e.sent.at(-1).command.action,'open');
    assert.deepEqual(e.sent.at(-1).stamp,{generation:1,revision:7,input_epoch:2});assert.equal(e.sent.at(-1).presentation,3);
    assert.equal(e.key({ctrlKey:true,metaKey:true}).prevented,undefined);
    for(const patch of [{isTrusted:false},{isComposing:true},{altKey:true},{shiftKey:true}]){e.ui.close();assert.equal(e.key(patch).prevented,undefined);assert.equal(e.ui.isOpen(),false);}
    assert.equal(e.key({key:'/',code:'Slash',ctrlKey:false}).prevented,undefined);
    e.allow(false);assert.equal(e.key({}).prevented,undefined);e.allow(true);
    e.set(packet({navigation:{kind:'article_list'}}));assert.equal(e.key({}).prevented,undefined);
    e.set(packet({ptt_features:false}));assert.equal(e.key({}).prevented,undefined);
});
test('adapter never reads glyph text; Rust hit replies alone drive display styles and navigation',()=>{
    const p=packet();Object.defineProperty(p.frame,'glyphs',{get(){throw new Error('DOM must not index glyphs');}});
    const e=environment(p);e.key({});e.edit('hElLo');assert.equal(e.sent.at(-1).command.action,'pending');e.tick();
    assert.deepEqual(e.sent.at(-1).command,{action:'edit',query:'hElLo'});assert.equal(e.ui.highlighter(),null);
    const first=e.reply();assert.equal(e.node('status').textContent,'1 / 3');assert.equal(e.jumps.at(-1).row,0);
    const highlight=e.ui.highlighter();assert.equal(highlight(glyph(0,0,'H')).bg,4);assert.equal(highlight(glyph(0,10,'h')).bg,11);assert.equal(highlight(glyph(0,7,' ')),null);
    e.key({key:'Enter',code:'Enter',ctrlKey:false});assert.deepEqual(e.sent.at(-1).command,{action:'step',back:false});
    e.reply({hits:null,data_version:first.data_version,active:1});assert.equal(e.node('status').textContent,'2 / 3');assert.equal(e.jumps.at(-1).start,10);
    e.key({key:'Enter',code:'Enter',ctrlKey:false,shiftKey:true});assert.deepEqual(e.sent.at(-1).command,{action:'step',back:true});
});
test('pending edits immediately clear display and Enter flushes without skipping the first result',()=>{
    const e=environment();e.key({});e.edit('hello');e.tick();e.reply();e.edit('new');
    assert.equal(e.ui.highlighter(),null);assert.equal(e.timers.size,1);
    e.key({key:'Enter',code:'Enter',ctrlKey:false});assert.equal(e.timers.size,0);assert.deepEqual(e.sent.at(-1).command,{action:'edit',query:'new'});
    e.reply({hits:[],active:null,total:0});assert.equal(e.node('status').textContent,'找不到');assert.equal(e.node('next').disabled,true);
    e.edit('');e.tick();e.reply({hits:[],active:null,total:0});assert.equal(e.node('status').textContent,'輸入文字');
});
test('Chinese IME stays in the local field and no query leaves before composition ends',()=>{
    const e=environment();e.key({});e.edit('中',{isComposing:true});e.tick();assert.equal(e.sent.at(-1).command.action,'pending');
    assert.equal(e.key({key:'Enter',code:'Enter',ctrlKey:false,isComposing:true}).prevented,undefined);
    assert.equal(e.key({key:'Escape',code:'Escape',ctrlKey:false,keyCode:229}).prevented,undefined);assert.equal(e.ui.isOpen(),true);
    e.fire(e.node('input'),'compositionend',{});e.tick();assert.deepEqual(e.sent.at(-1).command,{action:'edit',query:'中'});
    e.reply({hits:[{row:0,start:0,end:2,index:0}],total:1});assert.equal(e.ui.highlighter()(glyph(0,0,'中',2)).bg,4);
});
test('untrusted edits and clicks cannot send local search intents',()=>{
    const e=environment();e.key({});const n=e.sent.length;
    e.edit('bad',{isTrusted:false});e.fire(e.node('next'),'click',{isTrusted:false});e.fire(e.node('close'),'click',{isTrusted:false});e.tick();
    assert.equal(e.sent.length,n);assert.equal(e.ui.isOpen(),true);
});
test('late request/source replies and deltas with missing data cannot revive old highlights',()=>{
    const e=environment();e.key({});e.edit('old');e.tick();const old=e.sent.at(-1).request;e.edit('new');e.tick();
    e.reply({request:old});assert.equal(e.ui.highlighter(),null);
    e.reply({source:{generation:1,revision:6,presentation:3,input_epoch:2,article:1}});assert.equal(e.ui.highlighter(),null);
    const good=e.reply();assert.equal(e.node('status').textContent,'1 / 3');
    e.ui.receive({...good,active:2});assert.equal(e.node('status').textContent,'1 / 3','duplicate version rejected');
    e.reply({hits:null,data_version:999,active:1});assert.equal(e.node('status').textContent,'1 / 3');
});
test('same article reprojection reissues a bounded query but never jumps on passive results',()=>{
    const e=environment();e.key({});e.edit('hello');e.tick();e.reply();const jumps=e.jumps.length,requests=e.sent.length;
    e.set(packet({presentation:4,frame:{revision:8,rows:41,cols:80}}));assert.equal(e.ui.highlighter(),null);e.tick();
    assert.equal(e.sent.length,requests,'Rust sync reprojects the existing query without resetting its anchor');
    e.reply({active:1,jump:false});assert.equal(e.node('status').textContent,'2 / 3');assert.equal(e.jumps.length,jumps);
    e.set(packet({article:{id:2}}));assert.equal(e.ui.isOpen(),false);assert.equal(e.node('input').value,'');
    e.key({});e.set(packet({generation:2}));assert.equal(e.ui.isOpen(),false);
});
test('terminal revision changes close search; the UI does not request station pages',()=>{
    const e=environment(packet({article:null,frame:{revision:1,rows:24,cols:80}}));e.key({});e.edit('term');e.tick();
    e.reply({hits:[{row:0,start:0,end:4,index:0}],total:1});assert.equal(e.node('status').textContent,'1 / 1');assert.ok(e.node('input').title.includes('目前頁'));
    assert.ok(e.sent.every(p=>p.kind==='article_find'));
    e.set(packet({article:null,frame:{revision:2,rows:24,cols:80}}));assert.equal(e.ui.isOpen(),false);
});
test('reply bounds reject malformed glyph ranges and cap results; count labels use Rust limit flag',()=>{
    for(const hits of [[{row:40,start:0,end:1,index:0}],[{row:0,start:1,end:81,index:0}],[{row:0,start:0,end:1,index:3}],Array(4097).fill({row:0,start:0,end:1,index:0})]){
        const e=environment();e.key({});e.edit('a');e.tick();e.reply({hits});assert.equal(e.ui.highlighter(),null);
    }
    const e=environment();e.key({});e.edit('a');e.tick();
    e.reply({hits:Array.from({length:4096},(_,index)=>({row:Math.floor(index/128),start:index%80,end:index%80+1,index})),total:4096,limited:true});
    assert.equal(e.node('status').textContent,'1 / 4,096+');
});
test('UTF16 query limit preserves supplementary characters and rejects lone surrogates',()=>{
    const e=environment();e.key({});e.edit('😀'.repeat(128)+'z');e.tick();assert.equal(e.sent.at(-1).command.query,'😀'.repeat(128));
    e.edit('a'.repeat(255)+'😀');e.tick();assert.equal(e.sent.at(-1).command.query,'a'.repeat(255));
    e.edit('中\ud800');e.tick();assert.equal(e.sent.at(-1).command.query,'中');
});
test('close/destroy cancel debounce, release styles and notify Rust once; backend closure does not echo',()=>{
    const e=environment();e.key({});e.edit('late');e.key({key:'Escape',code:'Escape',ctrlKey:false});const n=e.sent.length;
    e.tick();assert.equal(e.sent.length,n);assert.equal(e.sent.at(-1).command.action,'close');assert.equal(e.ui.isOpen(),false);assert.equal(e.node('input').value,'');
    e.key({});const before=e.sent.length;e.reply({opened:false});assert.equal(e.sent.length,before);assert.equal(e.ui.isOpen(),false);
    e.key({});e.edit('pending');assert.equal(e.ui.clean(),false);e.ui.destroy();e.tick();assert.equal(e.ui.clean(),true);assert.equal(e.ui.highlighter(),null);assert.equal(e.key({}).prevented,undefined);
});
test('highlight renderer overrides both halves for contrast but default and PNG rendering preserve source colours',()=>{
    const fills=[],texts=[],ctx={setTransform(){},fillRect(){fills.push(this.fillStyle);},measureText:()=>({width:12}),save(){},beginPath(){},rect(){},clip(){},translate(){},scale(){},restore(){},fillText(){texts.push(this.fillStyle);}};
    const render=vm.runInNewContext(readFileSync(new URL('../assets/surface-renderer.js',import.meta.url),'utf8')+'\nrender',{fontDescription:()=>''});
    const p={frame:{rows:1,cols:2,cursor_visible:false,glyphs:[glyph(0,0,'中',2,{fg:1,bg:4},{fg:2,bg:5})]},settings:{baseline:20}};
    const target={width:24,height:24,getContext:()=>ctx};
    render(target,p,{highlight:()=>({fg:0,bg:11})});assert.deepEqual(texts,['#000000','#000000']);assert.deepEqual(fills.slice(1),['#ffff00','#ffff00']);
    fills.length=texts.length=0;render(target,p,{cursor:false});assert.deepEqual(texts,['#800000','#008000']);assert.deepEqual(fills.slice(1),['#000080','#800080']);
});
