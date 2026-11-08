import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {JSDOM} from 'jsdom';
const dom=new JSDOM('<!doctype html><body><div id="editor"></div>',{pretendToBeVisual:true});
for(const name of ['window','document','MutationObserver','HTMLElement','Node','Window'])globalThis[name]=dom.window[name];
Object.defineProperty(globalThis,'navigator',{value:dom.window.navigator,configurable:true});
globalThis.requestAnimationFrame=dom.window.requestAnimationFrame.bind(dom.window);globalThis.cancelAnimationFrame=dom.window.cancelAnimationFrame.bind(dom.window);
// jsdom supplies editing DOM but no layout. Geometry is deliberately not claimed here.
dom.window.Range.prototype.getClientRects=()=>[];dom.window.Range.prototype.getBoundingClientRect=()=>({left:0,right:0,top:0,bottom:0,width:0,height:0});
const genericCanvas=readFileSync(new URL('../../assets/app.css',import.meta.url),'utf8').match(/^canvas \{[^}]+\}/m);
assert.ok(genericCanvas,'the product canvas rule must be exercised');
const style=document.createElement('style');style.textContent=genericCanvas[0];document.head.append(style);
const observers=[];dom.window.ResizeObserver=class{constructor(callback){this.callback=callback;this.connected=false;observers.push(this);}observe(target){this.target=target;this.connected=true;}disconnect(){this.connected=false;}};
const {create}=await import('./editor.js');
const {undo,redo}=await import('@codemirror/commands');
const {EditorSelection}=await import('@codemirror/state');
const palette=['#000000','#800000','#008000','#808000','#000080','#800080','#008080','#c0c0c0','#808080','#ff0000','#00ff00','#ffff00','#0000ff','#ff00ff','#00ffff','#ffffff'];
test('actual editor preserves Chinese/ANSI through all modes, coloring is undoable, and loading resets history',()=>{
 let edits=0,graphics=0;const e=create({graphicCode:ch=>ch==='█'?0x2588:null,renderGlyph:()=>graphics++,parent:document.getElementById('editor'),palette,label:'正文',onChange:()=>edits++,onContext(){}});
 e.load('abcdefgh\n中文第二行\nlast');e.view.dispatch({selection:EditorSelection.single(2,5)});assert.deepEqual(e.ranges(),[{from:2,to:5}]);
 e.view.dispatch({selection:EditorSelection.single(2,12)});assert.deepEqual(e.ranges(),[{from:2,to:12}]);
 const raw='中文\x1b[31m紅\x1b[m\n第三行';e.load(raw);
 e.project([{from:2,to:7,hidden:true},{from:7,to:8,hidden:false,left:{fg:1,bg:0},right:{fg:1,bg:0}},{from:8,to:11,hidden:true}]);
 assert.equal(e.value(),raw);assert.ok(!e.view.contentDOM.textContent.includes('[31m'));
 for(const mode of ['source','plain','color']){e.mode(mode);assert.equal(e.value(),raw);}
 e.selection('rectangle');e.view.dispatch({selection:EditorSelection.create([EditorSelection.range(0,2),EditorSelection.range(12,15)])});assert.deepEqual(e.ranges(),[{from:0,to:2},{from:12,to:15}]);
 e.selection('line');assert.deepEqual(e.ranges(),[{from:0,to:12},{from:12,to:15}]);e.selection('text');e.view.dispatch({selection:EditorSelection.single(0,1)});assert.deepEqual(e.ranges(),[{from:0,to:1}]);e.selection('rectangle');
 e.apply([{from:0,to:2,insert:'\x1b[1;31m中文\x1b[m'}]);assert.equal(edits,1);assert.ok(e.value().startsWith('\x1b[1;31m中文'));
 assert.ok(undo(e.view));assert.equal(e.value(),raw);assert.ok(redo(e.view));
 e.load('新的文章');assert.equal(undo(e.view),false);assert.equal(e.value(),'新的文章');
 e.load('中文\n\n');e.selection('line');e.view.dispatch({selection:EditorSelection.single(0,4)});assert.deepEqual(e.ranges(),[{from:0,to:4}]);
 e.load('█中');e.project([{from:0,to:2,width:2,left:{fg:1,bg:0},right:{fg:1,bg:0}}]);assert.equal(graphics,1);assert.equal(e.value(),'█中');assert.equal(e.view.contentDOM.querySelectorAll('canvas').length,1);
 const graphic=e.view.contentDOM.querySelector('canvas');const css=window.getComputedStyle(graphic);
 assert.equal(css.position,'static');assert.equal(css.display,'inline-block');assert.equal(graphic.style.inset,'auto');
 assert.equal(graphic.style.width,'2ch');assert.equal(graphic.style.height,'2ch');
 e.load('中A');e.project([{from:0,to:1,width:2,left:{fg:1,bg:0},right:{fg:1,bg:0}}]);
 for(const mode of ['color','plain','source']){e.mode(mode);assert.equal(e.view.contentDOM.querySelector('span').style.width,'2ch');assert.equal(e.value(),'中A');}
 assert.ok(observers.some(o=>o.target===document.getElementById('editor')&&o.connected));
 e.destroy();assert.ok(observers.every(o=>!o.connected));for(const o of observers)o.callback();assert.equal(document.getElementById('editor').children.length,0);dom.window.close();
});
