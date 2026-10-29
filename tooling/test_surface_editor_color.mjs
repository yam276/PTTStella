import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import vm from 'node:vm';
const source=readFileSync(new URL('../assets/surface-editor-color.js',import.meta.url),'utf8');
const scope=vm.createContext({});vm.runInContext(source,scope);
test('PTT palette converts foreground brightness and background independently',()=>{
 const foreground=['0;30','0;31','0;32','0;33','0;34','0;35','0;36','0;37','0;1;30','0;1;31','0;1;32','0;1;33','0;1;34','0;1;35','0;1;36','0;1;37'];
 for(let f=0;f<16;f++)for(let b=0;b<8;b++)assert.equal(scope.editorColorCode(f,b),foreground[f]+';'+(40+b));
 for(const [f,b] of [[16,0],[-1,0],[1.5,0],[0,8],[0,-1],['1',0]])assert.throws(()=>scope.editorColorCode(f,b));
});
function fixture(){
 const nodes=new Map();let key='one',sent=[],restored=0;
 const node=()=>({children:[],style:{},dataset:{},attrs:{},append(b){this.children.push(b);},replaceChildren(){this.children=[];},setAttribute(k,v){this.attrs[k]=v;},focus(){},closest(){return this;}});
 const document={getElementById(id){if(!nodes.has(id))nodes.set(id,node());return nodes.get(id);},createElement:node};
 const root={...node(),open:false,contains:b=>[...nodes.values()].some(n=>n===b||n.children.includes(b)),showModal(){this.open=true;},close(){this.open=false;}};
 const ui=scope.createEditorColor({document,root,palette:Array.from({length:16},(_,i)=>'colour'+i),tr:k=>k,identity:()=>key,send:p=>sent.push(JSON.parse(JSON.stringify(p))),restore:()=>restored++,changed(){}});
 const click=(b,trusted=true)=>ui.bindings.find(b=>b[1]==='click')[2]({target:b,isTrusted:trusted});
 const open=()=>ui.open({presentation:3,request:11},{generation:7,revision:8,input_epoch:9});
 return {document,root,ui,nodes,sent,click,open,setKey:k=>key=k,restored:()=>restored};
}
test('picker offers exactly 16 foreground and 8 background swatches; Apply sends typed colours only',()=>{
 const f=fixture();f.open();assert.equal(f.root.open,true);
 const fg=f.nodes.get('editor-color-foreground').children,bg=f.nodes.get('editor-color-background').children;
 assert.equal(fg.length,16);assert.equal(bg.length,8);
 f.click(fg[9]);f.click(bg[4]);assert.equal(f.nodes.get('editor-color-code').textContent,'ESC[0;1;31;44m');
 assert.equal(f.nodes.get('editor-color-preview').style.color,'colour9');
 f.click(f.nodes.get('editor-color-apply'),false);assert.equal(f.sent.length,0);assert.equal(f.root.open,true);
 f.click(f.nodes.get('editor-color-apply'));assert.equal(f.root.open,false);
 assert.deepEqual(f.sent,[{kind:'editor_color',presentation:3,request:11,stamp:{generation:7,revision:8,input_epoch:9},color:{foreground:9,background:4},trusted:true}]);
 assert.equal(f.restored(),1);
});
test('cancel, stale selection, generation change and teardown send no station command',()=>{
 for(const how of ['cancel','stale','validate','destroy','escape']){
  const f=fixture();f.open();
  if(how==='cancel')f.click(f.nodes.get('editor-color-cancel'));
  else if(how==='destroy')f.ui.destroy();
  else if(how==='escape')f.ui.bindings.find(b=>b[1]==='keydown')[2]({key:'Escape',isTrusted:true,preventDefault(){},stopImmediatePropagation(){}});
  else{f.setKey('other');if(how==='validate')f.ui.validate();else f.click(f.nodes.get('editor-color-apply'));}
  assert.equal(f.root.open,false,how);assert.equal(f.sent.length,0,how);f.ui.destroy();assert.equal(f.ui.clean(),true);
 }
});
test('Defaults resets to normal white on black and preserves readable labels',()=>{
 const f=fixture();f.open();f.click(f.nodes.get('editor-color-reset'));assert.equal(f.nodes.get('editor-color-code').textContent,'ESC[0;37;40m');
 const swatches=f.nodes.get('editor-color-foreground').children;assert.equal(swatches[7].attrs['aria-pressed'],'true');assert.match(swatches[15].attrs['aria-label'],/editor-color-bright/);
});
