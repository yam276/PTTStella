import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import vm from 'node:vm';
import {translator} from './i18n_test_support.mjs';

const source=readFileSync(new URL('../assets/article-find.js',import.meta.url),'utf8');
const style={fg:7,bg:0,bright:false,inverse:false};
const glyph=(row,col,text,width=1,left=style,right=left)=>({row,col,text,width,left,right});
const packet=(glyphs,patch={})=>({generation:1,ptt_features:true,navigation:{kind:'article_reading'},article:{id:1,first_line:1,last_line:40,end:true},frame:{rows:41,cols:80,glyphs},...patch});
function environment(initial=packet([glyph(0,0,'Hello'),glyph(0,10,'hello'),glyph(39,0,'HELLO')])){
    const nodes=new Map(),timers=new Map(),jumps=[],actions=[];let sequence=0,current=initial,allowed=true;
    const document={getElementById(id){if(!nodes.has(id))nodes.set(id,{id,hidden:true,value:'',textContent:'',attrs:{},setAttribute(k,v){this.attrs[k]=v;},focus(){actions.push('focus');},select(){actions.push('select');},contains(node){return node.id.startsWith('article-find');}});return nodes.get(id);}};
    const create=vm.runInNewContext(source+'\ncreateArticleFind',{setTimeout:fn=>{timers.set(++sequence,fn);return sequence;},clearTimeout:id=>timers.delete(id)});
    const ui=create({document,tr:translator('zh-TW'),current:()=>current,allowed:()=>allowed,prepare:()=>actions.push('prepare'),restore:()=>actions.push('restore'),
        redraw:()=>actions.push('draw'),changed:()=>actions.push('changed'),jump:m=>jumps.push(JSON.parse(JSON.stringify(m)))});
    const node=id=>document.getElementById('article-find'+(id?'-'+id:''));
    const event=(extra={})=>({isTrusted:true,key:'f',code:'KeyF',ctrlKey:true,target:node('input'),preventDefault(){this.prevented=true;},stopPropagation(){this.stopped=true;},...extra});
    const fire=(target,type,extra={})=>{const e=event(extra),binding=ui.bindings.find(([t,k])=>t===target&&k===type);assert.ok(binding);binding[2](e);return e;};
    const key=extra=>fire(document,'keydown',extra);
    const edit=(text,extra={})=>{node('input').value=text;fire(node('input'),'input',extra);};
    const tick=()=>{const work=[...timers.values()];timers.clear();for(const fn of work)fn();};
    ui.update(current);
    return {ui,node,event,key,edit,tick,timers,jumps,actions,fire,document,set(packet){current=packet;ui.update(current);},allow(value){allowed=value;}};
}
test('Cmd/Ctrl F opens a local search, selects existing text, and never consumes station slash',()=>{
    const e=environment();
    assert.equal(e.key({key:'/',code:'Slash',ctrlKey:false}).prevented,undefined);assert.equal(e.ui.isOpen(),false);
    assert.equal(e.key({ctrlKey:true,metaKey:true}).prevented,undefined,'macOS fullscreen must retain Command Control F');
    assert.equal(e.key({ctrlKey:false,metaKey:true}).prevented,true);assert.equal(e.ui.isOpen(),true);
    e.edit('hello');e.tick();assert.equal(e.node('status').textContent,'1 / 3');
    assert.equal(e.key({key:'Process',keyCode:229}).prevented,true,'physical Ctrl F works with an idle IME');
    assert.equal(e.node('input').value,'hello');assert.equal(e.actions.filter(a=>a==='prepare').length,1);
    assert.equal(e.key({ctrlKey:true,metaKey:true}).prevented,undefined,'fullscreen remains available while find is open');
    for(const patch of [{isTrusted:false},{isComposing:true},{altKey:true},{shiftKey:true}]){
        e.ui.close();assert.equal(e.key(patch).prevented,undefined);assert.equal(e.ui.isOpen(),false);
    }
    e.allow(false);assert.equal(e.key({}).prevented,undefined);
    e.allow(true);e.set(packet([],{navigation:{kind:'article_list'}}));assert.equal(e.key({}).prevented,undefined);
    e.set(packet([],{ptt_features:false}));assert.equal(e.key({}).prevented,undefined);
});
test('literal case-insensitive matches navigate forward/backward with wrap and readable distinct colours',()=>{
    const e=environment();e.key({});e.edit('hElLo');e.tick();assert.equal(e.jumps.at(-1).row,0);
    const highlighted=e.ui.highlighter();assert.equal(highlighted(glyph(0,0,'H')).fg,15);assert.equal(highlighted(glyph(0,0,'H')).bg,4);
    assert.equal(highlighted(glyph(0,10,'h')).fg,0);assert.equal(highlighted(glyph(0,10,'h')).bg,11);
    assert.equal(highlighted(glyph(0,7,' ')),null);
    e.key({key:'Enter',ctrlKey:false});assert.equal(e.node('status').textContent,'2 / 3');assert.equal(e.jumps.at(-1).start,10);
    e.key({key:'Enter',ctrlKey:false,shiftKey:true});assert.equal(e.node('status').textContent,'1 / 3');
    e.key({key:'Enter',ctrlKey:false,shiftKey:true});assert.equal(e.jumps.at(-1).row,39);assert.equal(e.node('status').textContent,'3 / 3');
    e.edit('.*');e.tick();assert.equal(e.node('status').textContent,'找不到');assert.equal(e.ui.highlighter(),null);
    e.edit('');e.tick();assert.equal(e.node('status').textContent,'輸入文字');assert.equal(e.node('next').disabled,true);
});
test('Chinese, UTF16 graphemes and dual-column glyphs map to original cells without changing the frame',()=>{
    const p=packet([glyph(0,0,'中',2),glyph(0,2,'文',2),glyph(0,4,'😀',2),glyph(0,6,'İ',1)]),snapshot=JSON.stringify(p);
    const e=environment(p);e.key({});e.edit('中文😀');e.tick();assert.deepEqual(e.jumps.at(-1),{row:0,start:0,end:6,index:0});
    e.edit('i');e.tick();assert.equal(e.jumps.at(-1).start,6);assert.equal(JSON.stringify(p),snapshot);
});
test('concealed cells and blacklists cannot be found or revealed, and footer text is excluded',()=>{
    const black={...style,fg:0};
    const e=environment(packet([glyph(0,0,'secret',1,black),glyph(0,1,'x'),glyph(0,2,'word',2,style,black),glyph(40,0,'footer')]));
    e.key({});for(const query of ['secret','word','footer','\0']){e.edit(query);e.tick();assert.equal(e.ui.highlighter(),null,query);}
    e.edit('x');e.tick();assert.equal(e.node('status').textContent,'1 / 1');assert.equal(e.ui.highlighter()(glyph(0,0,'s')),null);
});
test('two matches in one grapheme keep the active glyph readable and selected',()=>{
    const g=glyph(0,0,'e\u0301\u0301'),e=environment(packet([g]));e.key({});e.edit('\u0301');e.tick();
    assert.equal(e.node('status').textContent,'1 / 2');e.key({key:'Enter',ctrlKey:false});
    assert.equal(e.node('status').textContent,'2 / 2');assert.equal(e.ui.highlighter()(g).bg,4);
});
test('IME Enter/Escape and pending edits stay local; close cancels delayed work and clears ownership',()=>{
    const e=environment();e.key({});e.edit('hello',{isComposing:true});e.tick();assert.equal(e.jumps.length,0);
    assert.equal(e.key({key:'Enter',ctrlKey:false,isComposing:true}).prevented,undefined);
    assert.equal(e.key({key:'Escape',ctrlKey:false,keyCode:229}).prevented,undefined);assert.equal(e.ui.isOpen(),true);
    e.fire(e.node('input'),'compositionend',{});assert.equal(e.timers.size,1);
    e.key({key:'Enter',ctrlKey:false});assert.equal(e.node('status').textContent,'1 / 3','flush does not skip the first match');
    e.allow(false);assert.equal(e.key({key:'Escape',ctrlKey:false}).prevented,undefined,'a modal/context menu keeps Escape');assert.equal(e.ui.isOpen(),true);e.allow(true);
    e.edit('late');e.key({key:'Escape',ctrlKey:false});e.tick();assert.equal(e.ui.isOpen(),false);assert.equal(e.ui.highlighter(),null);assert.equal(e.node('input').value,'');
    e.key({});e.edit('late');assert.equal(e.ui.clean(),false);e.ui.destroy();e.tick();assert.equal(e.ui.isOpen(),false);assert.equal(e.key({}).prevented,undefined);assert.equal(e.ui.clean(),true);
});
test('same-article reprojection retains query and selection, but switching or reconnecting discards them',()=>{
    const e=environment();e.key({});e.edit('hello');e.tick();e.key({key:'Enter',ctrlKey:false});
    e.set(packet([glyph(0,0,'Hello'),glyph(0,10,'hello'),glyph(1,0,'hello')]));assert.equal(e.node('status').textContent,'2 / 3');assert.equal(e.ui.isOpen(),true);
    e.set(packet([glyph(0,0,'Hello')],{article:{id:2,first_line:1,last_line:40}}));assert.equal(e.ui.isOpen(),false);assert.equal(e.ui.highlighter(),null);
    e.key({});e.edit('Hello');e.tick();e.set(packet([],{generation:2}));assert.equal(e.ui.isOpen(),false);
});
test('terminal mode searches the displayed body only and never requests another page',()=>{
    const e=environment(packet([glyph(0,0,'term'),glyph(23,0,'term')],{article:null,frame:{rows:24,cols:80,glyphs:[glyph(0,0,'term'),glyph(23,0,'term')]}}));
    e.key({});e.edit('term');e.tick();assert.equal(e.node('status').textContent,'1 / 1');
    assert.ok(e.node('input').title.includes('目前頁'));
    e.set(packet([],{article:null,frame:{rows:24,revision:2,cols:80,glyphs:[]}}));assert.equal(e.ui.isOpen(),false,'a new terminal page clears the previous query');
});
test('disabled search never indexes a long frame, active queries reuse its index, and result growth is bounded',()=>{
    let reads=0;const glyphs=Array.from({length:10000},(_,i)=>({...glyph(Math.floor(i/80),i%80,'a'),get text(){reads++;return 'a';}}));
    const p=packet(glyphs,{article:{id:1,first_line:1,last_line:125},frame:{rows:126,cols:80,glyphs}}),e=environment(p);
    for(let i=0;i<10;i++)e.ui.update(p);assert.equal(reads,0);
    e.key({});e.edit('a');e.tick();assert.equal(reads,10000);assert.equal(e.node('status').textContent,'1 / 4,096+');
    e.edit('aa');e.tick();assert.equal(reads,10000);e.ui.update(p);assert.equal(reads,10000);
    e.edit('');e.tick();e.edit('aa');e.tick();assert.equal(reads,20000,'clearing the query releases the old index');
    e.ui.close();e.ui.update(p);assert.equal(e.ui.highlighter(),null);
});
test('highlight renderer overrides both halves for contrast but default and PNG rendering preserve source colours',()=>{
    const fills=[],texts=[],ctx={setTransform(){},fillRect(){fills.push(this.fillStyle);},measureText:()=>({width:12}),save(){},beginPath(){},rect(){},clip(){},translate(){},scale(){},restore(){},fillText(){texts.push(this.fillStyle);}};
    const render=vm.runInNewContext(readFileSync(new URL('../assets/surface-renderer.js',import.meta.url),'utf8')+'\nrender',{fontDescription:()=>''});
    const p={frame:{rows:1,cols:2,cursor_visible:false,glyphs:[glyph(0,0,'中',2,{fg:1,bg:4},{fg:2,bg:5})]},settings:{baseline:20}};
    const target={width:24,height:24,getContext:()=>ctx};
    render(target,p,{highlight:()=>({fg:0,bg:11})});assert.deepEqual(texts,['#000000','#000000']);assert.deepEqual(fills.slice(1),['#ffff00','#ffff00']);
    fills.length=texts.length=0;render(target,p,{cursor:false});assert.deepEqual(texts,['#800000','#008000']);assert.deepEqual(fills.slice(1),['#000080','#800080']);
});
