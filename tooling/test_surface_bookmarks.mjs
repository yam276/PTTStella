// Executes the production owner with DOM/event doubles; not native IME evidence.
import test from "node:test";
import assert from "node:assert/strict";
import {readFileSync} from "node:fs";
import vm from "node:vm";
import {translator} from "./i18n_test_support.mjs";

const entries=JSON.parse(readFileSync(new URL("../tests/fixtures/bookmarks.json",import.meta.url),"utf8")).entries;
function environment(){
    const nodes=new Map(),sent=[],focus=[],destinations=[];let active=false,foreign=false;
    const node=id=>{if(!nodes.has(id))nodes.set(id,{id,value:"",textContent:"",disabled:false,open:false,children:[],
        showModal(){this.open=true;},close(){this.open=false;},focus(){focus.push(id);},replaceChildren(...children){this.children=children;}});return nodes.get(id);};
    const document={getElementById:node,createElement:()=>({}),querySelector:()=>foreign?{}:[...nodes.values()].find(node=>node.open)};
    const create=vm.runInNewContext(readFileSync(new URL("../assets/surface-bookmarks.js",import.meta.url),"utf8")+"\ncreateBookmarksDialog",{document});
    const ui=create({tr:translator("en-US"),send:value=>sent.push(JSON.parse(JSON.stringify(value))),prepare(){},restore:()=>focus.push("restore"),changed(){},
        openDestination:(e,value)=>{destinations.push(value);return true;},current:()=>({destination:active?{state:"active"}:{state:"idle"}})});
    function fire(id,event="click",extra={}){
        const e={target:node(id),isTrusted:true,preventDefault(){},...extra};
        const binding=ui.bindings.find(([target,type])=>target===node(id)&&type===event);assert.ok(binding,`${id} ${event}`);binding[2](e);
    }
    const snapshot=(rows=entries,writable=true)=>({entries:rows,writable,warning:writable?null:"invalid source"});
    function load(rows=entries,writable=true){ui.receiveOpen({request:sent.at(-1).request,data:snapshot(rows,writable)});}
    function result(request,status="saved",rows=entries){ui.receive({request,data:snapshot(rows),feedback:{status,notice:status==="saved"?"saved":"failure"}});}
    return {node,ui,sent,focus,destinations,fire,load,result,snapshot,active:value=>{active=value;ui.update();},foreign:value=>foreign=value};
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
