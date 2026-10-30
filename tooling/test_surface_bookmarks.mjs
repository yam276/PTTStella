// Executes the production owner with DOM/event doubles; not native IME evidence.
import test from "node:test";
import assert from "node:assert/strict";
import {readFileSync} from "node:fs";
import vm from "node:vm";
import {translator} from "./i18n_test_support.mjs";

const entries=JSON.parse(readFileSync(new URL("../tests/fixtures/bookmarks.json",import.meta.url),"utf8")).entries;
function environment(){
    const nodes=new Map(),sent=[],focus=[],destinations=[],painted=[],notices=[];let active=false,foreign=false,packet={};
    const node=id=>{if(!nodes.has(id))nodes.set(id,{id,value:"",textContent:"",disabled:false,open:false,children:[],
        showModal(){this.open=true;},close(){this.open=false;},focus(){focus.push(id);},replaceChildren(...children){this.children=children;}});return nodes.get(id);};
    const window={};
    const document={getElementById:node,createElement:()=>({}),querySelector:()=>foreign?{}:[...nodes.values()].find(node=>node.open)};
    const create=vm.runInNewContext(readFileSync(new URL("../assets/surface-bookmarks.js",import.meta.url),"utf8")+"\ncreateBookmarksDialog",{document});
    const ui=create({window,tr:translator("en-US"),send:value=>sent.push(JSON.parse(JSON.stringify(value))),prepare(){},restore:()=>focus.push("restore"),changed(){},
        openDestination:(e,value)=>{destinations.push(value);return true;},render:(canvas,paint)=>painted.push(paint),notice:v=>notices.push(v),current:()=>({...packet,destination:active?{state:"active"}:{state:"idle"}})});
    function fire(id,event="click",extra={}){
        const e={target:node(id),isTrusted:true,preventDefault(){},stopPropagation(){},...extra};
        const binding=ui.bindings.find(([target,type])=>target===node(id)&&type===event);assert.ok(binding,`${id} ${event}`);binding[2](e);
    }
    const snapshot=(rows=entries,writable=true)=>({entries:rows,writable,warning:writable?null:"invalid source"});
    function load(rows=entries,writable=true){ui.receiveOpen({request:sent.at(-1).request,data:snapshot(rows,writable)});}
    function result(request,status="saved",rows=entries){ui.receive({request,data:snapshot(rows),feedback:{status,notice:status==="saved"?"saved":"failure"}});}
    return {node,ui,sent,window,focus,destinations,painted,notices,setCurrent:value=>{packet=value;ui.update();},fire,load,result,snapshot,active:value=>{active=value;ui.update();},foreign:value=>foreign=value};
}
test("opening and selection stay local, and Go hands one explicit draft to the existing destination owner",()=>{
    const env=environment();env.fire("bookmarks-open");assert.deepEqual(env.sent,[{kind:"bookmarks_open",request:1,trusted:true}]);
    env.load();assert.equal(env.node("bookmarks-list").children.length,2);
    assert.equal(env.node("bookmarks-list").children[0].textContent,"自製合成書籤 — #00000101@Stella");
    env.node("bookmarks-list").value="#00000101@Stella";env.fire("bookmarks-list","change");
    assert.equal(env.node("bookmarks-target").value,"#00000101@Stella");assert.equal(env.node("bookmarks-label").value,"自製合成書籤");
    env.active(true);env.fire("bookmarks-go");assert.deepEqual(env.destinations,[]);
    env.active(false);env.fire("bookmarks-go");assert.deepEqual(env.destinations,["#00000101@Stella"]);
    assert.equal(env.node("bookmarks-dialog").open,false);assert.equal(env.node("bookmarks-target").value,"");assert.equal(env.sent.length,1);
});
test("editing is single-flight, IME-safe, and failed results retain the exact draft and saved list",()=>{
    const env=environment();env.fire("bookmarks-open");env.load();
    env.node("bookmarks-target").value="https://www.ptt.cc/bbs/Stella/M.1.A.001.html";env.node("bookmarks-label").value="草稿";
    env.fire("bookmarks-dialog","compositionstart");env.fire("bookmarks-save");
    env.fire("bookmarks-dialog","keydown",{target:env.node("bookmarks-label"),key:"Enter"});assert.equal(env.sent.length,1);
    env.fire("bookmarks-dialog","compositionend");
    env.fire("bookmarks-dialog","keydown",{target:env.node("bookmarks-label"),key:"Enter",keyCode:229});assert.equal(env.sent.length,1);
    env.fire("bookmarks-dialog","keydown",{target:env.node("bookmarks-label"),key:"Enter",keyCode:13});
    assert.deepEqual(env.sent[1],{kind:"bookmarks_edit",request:2,value:"https://www.ptt.cc/bbs/Stella/M.1.A.001.html",label:"草稿",remove:false,trusted:true});
    env.fire("bookmarks-save");env.fire("bookmarks-new");assert.equal(env.sent.length,2);
    env.result(99);assert.equal(env.node("bookmarks-save").disabled,true);
    env.result(2,"failed");assert.equal(env.node("bookmarks-target").value,"https://www.ptt.cc/bbs/Stella/M.1.A.001.html");
    assert.equal(env.node("bookmarks-label").value,"草稿");assert.equal(env.node("bookmarks-note").textContent,"failure");
    assert.equal(env.node("bookmarks-save").disabled,false);assert.equal(env.node("bookmarks-list").children.length,2);
});
test("deletion uses the selected saved identity and new-draft/cancel never save",()=>{
    const env=environment();env.fire("bookmarks-open");env.load();
    env.node("bookmarks-list").value="#00000101@Stella";env.fire("bookmarks-list","change");
    env.node("bookmarks-target").value="#00000102@Other";env.fire("bookmarks-remove");
    assert.equal(env.sent[1].value,"#00000101@Stella");assert.equal(env.sent[1].remove,true);
    env.result(2,"saved",entries.slice(1));assert.equal(env.node("bookmarks-list").children.length,1);
    assert.equal(env.node("bookmarks-remove").disabled,true);
    env.fire("bookmarks-new");assert.equal(env.node("bookmarks-target").value,"");
    env.fire("bookmarks-dialog","keydown",{target:env.node("bookmarks-close"),key:"Enter"});assert.equal(env.sent.length,2);
    env.node("bookmarks-target").value="unused";env.fire("bookmarks-dialog","cancel");
    assert.equal(env.node("bookmarks-target").value,"");assert.equal(env.sent.length,2);
});
test("late reads cannot reopen a closed editor or overwrite a later draft",()=>{
    const env=environment();env.fire("bookmarks-open");env.fire("bookmarks-dialog","cancel");
    const before=env.focus.length;env.ui.receiveOpen({request:1,data:env.snapshot()});assert.equal(env.focus.length,before);
    env.fire("bookmarks-open");env.load();env.node("bookmarks-target").value="newer draft";
    env.ui.receiveOpen({request:1,data:env.snapshot([])});
    assert.equal(env.node("bookmarks-target").value,"newer draft");assert.equal(env.node("bookmarks-list").children.length,2);
});
test("a completed save after close updates data without reopening or stealing focus, including reopen while busy",()=>{
    const env=environment();env.fire("bookmarks-open");env.load();env.node("bookmarks-target").value="#00000101@Stella";env.fire("bookmarks-save");
    env.fire("bookmarks-close");const before=env.focus.length;env.result(2);assert.equal(env.focus.length,before);assert.equal(env.node("bookmarks-dialog").open,false);
    env.fire("bookmarks-open");env.load();env.node("bookmarks-target").value="#00000101@Stella";env.fire("bookmarks-save");
    env.fire("bookmarks-close");env.fire("bookmarks-open");env.load();const next=env.focus.length;
    assert.equal(env.node("bookmarks-target").disabled,true);
    env.result(4);assert.equal(env.focus.length,next);assert.equal(env.node("bookmarks-target").value,"");assert.equal(env.node("bookmarks-target").disabled,false);
});
test("untrusted, readonly, foreign-modal and disposed operations cannot edit or navigate",()=>{
    const env=environment();env.fire("bookmarks-open","click",{isTrusted:false});assert.deepEqual(env.sent,[]);
    env.foreign(true);env.fire("bookmarks-open");assert.deepEqual(env.sent,[]);env.foreign(false);
    env.fire("bookmarks-open");env.load([],false);assert.equal(env.node("bookmarks-save").disabled,true);
    env.node("bookmarks-target").value="#00000101@Stella";env.fire("bookmarks-save");assert.equal(env.sent.length,1);
    assert.equal(env.node("bookmarks-note").textContent,"invalid source");
    env.ui.revoke();env.ui.receiveOpen({request:1,data:env.snapshot()});env.result(2);env.ui.destroy();
    for(const id of ["bookmarks-open","bookmarks-save","bookmarks-remove","bookmarks-go"])env.fire(id);
    env.fire("bookmarks-dialog","compositionstart");env.ui.update();assert.equal(env.ui.clean(),true);
    assert.equal(env.sent.length,1);assert.deepEqual(env.destinations,[]);
});

const reading={generation:7,input_epoch:2,connected:true,ptt_features:true,mouse:{route:"local"},frame:{revision:4},article:{id:9,first_line:1,end:true,load:"complete"}};
test("current article capture sends only acknowledged identity, stays single-flight and updates an already open list",()=>{
 const e=environment();e.setCurrent({...reading,article:{...reading.article,load:"loading"}});e.fire("bookmarks-current");assert.equal(e.sent.length,0);
 e.setCurrent(reading);e.fire("bookmarks-current","click",{isTrusted:false});assert.equal(e.sent.length,0);
 e.fire("bookmarks-current");assert.equal(e.node("bookmarks-capture").hidden,false);assert.deepEqual(e.sent[0],{kind:"bookmark_current",request:1,stamp:{generation:7,revision:4,input_epoch:2},article:9,trusted:true});
 e.fire("bookmarks-current");assert.equal(e.sent.length,1);e.ui.captureStage({request:1,cancellable:false});assert.equal(e.node("bookmarks-current-cancel").disabled,true);
 e.fire("bookmarks-current-cancel");assert.equal(e.sent.length,1);
 e.fire("bookmarks-open");e.load([]);const saved=[{...entries[0],cached_at:100,lines:45}];e.result(1,"saved",saved);
 assert.equal(e.node("bookmarks-list").children.length,1);assert.ok(e.node("bookmarks-list").children[0].textContent.includes("Local copy"));assert.equal(e.notices.at(-1),"saved");
});
test("capture cancellation preserves its request and failure never claims a saved local copy",()=>{
 const e=environment();e.setCurrent(reading);e.fire("bookmarks-current");e.fire("bookmarks-current-cancel");
 assert.deepEqual(e.sent[1],{kind:"bookmark_cancel",request:1,trusted:true});e.result(1,"failed",[]);
 assert.equal(e.node("bookmarks-current-cancel").hidden,true);assert.equal(e.node("bookmarks-capture").hidden,true);assert.equal(e.node("bookmarks-current").disabled,false);assert.equal(e.notices.at(-1),"failure");
});
function archive(){return {target:{board:"Stella",aid:"00000101"},saved_at:100,settings:{},frame:{rows:45,cols:80,glyphs:Array.from({length:45},(_,row)=>({row,col:0,width:1,text:String(row)}))}};}
test("offline reader uses the shared renderer with bounded visible rows and never sends navigation or text",()=>{
 const e=environment();e.fire("bookmarks-open");e.load([{...entries[0],cached_at:100,lines:45}]);e.node("bookmarks-list").value="#00000101@Stella";e.fire("bookmarks-list","change");e.fire("bookmarks-read");
 assert.deepEqual(e.sent[1],{kind:"archive_open",request:2,value:"#00000101@Stella",trusted:true});e.ui.receiveArchive({request:2,article:archive(),message:null});
 assert.ok(e.node("bookmarks-archive-info").textContent.startsWith("#00000101@Stella"));assert.equal(e.node("bookmarks-reader").hidden,false);assert.equal(e.painted.at(-1).frame.rows,23);
 e.fire("bookmarks-next");assert.equal(e.painted.at(-1).frame.glyphs[0].text,"22");assert.equal(e.painted.at(-1).frame.glyphs[0].row,0);
 e.fire("bookmarks-first");assert.equal(e.painted.at(-1).frame.glyphs[0].text,"0");
 e.fire("bookmarks-reader","keydown",{key:"End"});assert.equal(e.painted.at(-1).frame.glyphs.at(-1).text,"44");assert.equal(e.focus.at(-1),"bookmarks-canvas");
 e.fire("bookmarks-reader","keydown",{key:"ArrowUp"});assert.equal(e.painted.at(-1).frame.glyphs[0].text,"21");
 assert.equal(e.sent.length,2);assert.deepEqual(e.destinations,[]);e.ui.destroy();assert.equal(e.ui.clean(),true);
});
test("closing and reopening cannot show a late local article or replace another selection",()=>{
 const e=environment();e.fire("bookmarks-open");e.load([{...entries[0],cached_at:100}]);e.node("bookmarks-list").value="#00000101@Stella";e.fire("bookmarks-list","change");e.fire("bookmarks-read");
 e.fire("bookmarks-close");e.fire("bookmarks-open");const before=e.focus.length;e.ui.receiveArchive({request:2,article:archive()});
 assert.equal(e.painted.length,0);assert.equal(e.focus.length,before);assert.equal(e.node("bookmarks-reader").hidden,true);
});

const articleList={generation:7,input_epoch:2,presentation:1,connected:true,ptt_features:true,mouse:{route:'local'},frame:{revision:4},article_mode:'web',navigation:{kind:'article_list',cursor_row:4,articles:[{row:3,title_start:30,bookmark:true},{row:4,title_start:30,bookmark:true},{row:5,title_start:30,bookmark:false}]}};
test('list capture pins the right-click row rather than the highlighted article and rejects stale targets',()=>{
 const e=environment();e.setCurrent(articleList);assert.equal(e.ui.listAvailable(),true);
 assert.equal(e.ui.setListTarget(3),true);e.fire('bookmarks-current');
 assert.deepEqual(e.sent[0],{kind:'bookmark_list',request:1,stamp:{generation:7,revision:4,input_epoch:2},row:3,trusted:true});
 assert.equal(e.node('bookmarks-current').disabled,true);e.result(1,'failed',[]);
 e.setCurrent({...articleList,frame:{revision:5}});assert.equal(e.ui.available(),false,'never retarget to the cursor after a stale right-click');e.fire('bookmarks-current');assert.equal(e.sent.length,1);
 e.ui.clearListTarget();assert.equal(e.ui.listAvailable(),true);e.fire('bookmarks-current');assert.equal(e.sent.at(-1).row,4);
});
test('list collection excludes deleted rows, terminal mode, stale input, remote mouse and untrusted actions',()=>{
 const e=environment();e.setCurrent(articleList);assert.equal(e.ui.setListTarget(5),false);assert.equal(e.ui.available(),false,'deleted title never falls back to the cursor');e.ui.clearListTarget();
 e.ui.capture({isTrusted:false},3);assert.equal(e.sent.length,0);
 for(const patch of [{article_mode:'terminal'},{input_pending:true},{output_pending:true},{ptt_features:false},{connected:false},{mouse:{route:'station'}}]){
  e.setCurrent({...articleList,...patch});assert.equal(e.ui.listAvailable(3),false);e.ui.capture({isTrusted:true},3);assert.equal(e.sent.length,0);
 }
});

test('losing window focus cancels a list collection once and never starts another article',()=>{
 const e=environment();e.setCurrent(articleList);e.fire('bookmarks-current');
 const blur=e.ui.bindings.find(([target,event])=>target===e.window&&event==='blur')[2];
 blur({isTrusted:true});blur({isTrusted:true});
 assert.equal(e.sent.length,2);assert.deepEqual(e.sent[1],{kind:'bookmark_cancel',request:1,trusted:true});
 e.result(1,'failed',[]);e.setCurrent({...articleList,generation:8});blur({isTrusted:true});assert.equal(e.sent.length,2);
});
