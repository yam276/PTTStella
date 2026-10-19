// Production dialog with explicit DOM doubles. Socket/encoding proof uses the Rust loopback and native gate.
import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import vm from 'node:vm';
import {translator} from './i18n_test_support.mjs';
const target=(endpoint='telnet://example.com',encoding='gbk')=>({endpoint,encoding});
const data=()=>({sites:{version:1,default:target(),favourites:[{name:'GBK',target:target()},{name:'Big5',target:target('wss://example.com/bbs','big5')}]},current:target(),writable:true,warning:null});
function fixture(language='zh-TW'){
    const nodes=new Map(),sent=[],focus=[];
    const node=id=>{if(!nodes.has(id))nodes.set(id,{id,value:'',disabled:false,open:false,textContent:'',children:[],showModal(){this.open=true;},close(){this.open=false;},focus(){focus.push(id);},replaceChildren(...children){this.children=children;}});return nodes.get(id);};
    const document={getElementById:node,querySelector:()=>[...nodes.values()].find(n=>n.open),createElement:()=>({value:'',textContent:''})};
    const create=vm.runInNewContext(readFileSync(new URL('../assets/surface-sites.js',import.meta.url),'utf8')+'\ncreateSitesDialog',{document});
    const ui=create({tr:translator(language),send:value=>sent.push(JSON.parse(JSON.stringify(value))),prepare(){},restore(){focus.push('restore');},changed(){}});
    function fire(id,type='click',extra={}){const binding=ui.bindings.find(([element,event])=>element===node(id)&&event===type);assert.ok(binding);binding[2]({target:node(id),isTrusted:true,preventDefault(){},...extra});}
    function open(snapshot=data()){fire('sites-open');ui.receiveOpen({request:sent.at(-1).request,data:snapshot});}
    return {node,sent,focus,ui,fire,open};
}
test('choosing a favourite fills its saved encoding without sending or saving until an explicit action',()=>{
    for(const language of ['zh-TW','en-US']){
        const f=fixture(language);f.open();f.sent.length=0;f.node('sites-list').value='1';f.fire('sites-list','change');
        assert.equal(f.node('site-address').value,'wss://example.com/bbs');assert.equal(f.node('site-encoding').value,'big5');assert.deepEqual(f.sent,[]);
        f.node('site-address').value='draft';f.fire('site-address','input');f.fire('sites-close');assert.deepEqual(f.sent,[]);assert.equal(f.node('sites-dialog').open,false);
        f.ui.destroy();assert.equal(f.ui.clean(),true);
    }
});
test('failed saves preserve the draft and removed entries cannot silently select another favourite',()=>{
    const f=fixture();f.open();f.node('sites-list').value='0';f.fire('sites-list','change');f.node('site-name').value='Rename';f.fire('site-save');
    const first=f.sent.at(-1);f.fire('site-connect');f.fire('sites-dialog','cancel');assert.equal(f.sent.at(-1),first);assert.equal(f.node('sites-dialog').open,true);
    f.ui.receive({request:first.request-1,data:data(),feedback:{status:'saved'}});assert.equal(f.node('site-connect').disabled,true);
    f.ui.receive({request:first.request,data:data(),feedback:{status:'rejected',notice:'write failed'}});assert.equal(f.node('site-name').value,'Rename');assert.equal(f.node('sites-note').textContent,'write failed');
    f.fire('site-remove');const request=f.sent.at(-1).request;const removed=data();removed.sites.favourites.shift();
    f.ui.receive({request,data:removed,feedback:{status:'saved'}});assert.equal(f.node('sites-list').value,'');assert.equal(f.node('site-remove').disabled,true);
    f.ui.destroy();
});
test('untrusted and composition actions do not connect; a rejected address stays editable, successful enqueue closes',()=>{
    const f=fixture();f.open();f.sent.length=0;f.fire('site-connect','click',{isTrusted:false});f.fire('site-address','compositionstart');f.fire('site-connect');assert.deepEqual(f.sent,[]);
    f.fire('site-address','compositionend');f.node('site-address').value='https://example.com';f.fire('site-connect');let request=f.sent.at(-1).request;
    f.ui.receive({request,data:data(),feedback:{status:'rejected',notice:'bad address'}});assert.equal(f.node('site-address').value,'https://example.com');assert.equal(f.node('sites-dialog').open,true);
    f.node('site-address').value='telnet://example.com';f.fire('site-connect');request=f.sent.at(-1).request;
    assert.deepEqual(f.sent.at(-1),{kind:'sites_connect',request,address:'telnet://example.com',encoding:'gbk',trusted:true});
    f.ui.receive({request,data:data(),feedback:{status:'connecting'}});assert.equal(f.node('sites-dialog').open,false);assert.equal(f.focus.at(-1),'restore');
    f.ui.destroy();f.ui.receive({request,data:data(),feedback:{status:'saved'}});assert.equal(f.ui.clean(),true);
});
test('a corrupt station file disables persistence while allowing a deliberate manual connection',()=>{
    const f=fixture();f.open({...data(),writable:false,warning:'broken'});assert.equal(f.node('site-save').disabled,true);assert.equal(f.node('site-default').disabled,true);
    f.sent.length=0;f.fire('site-save');assert.deepEqual(f.sent,[]);f.fire('site-connect');assert.equal(f.sent.at(-1).kind,'sites_connect');f.ui.destroy();
});
