// Production owner with explicit DOM/event doubles; native IME is a separate gate.
import test from "node:test";
import assert from "node:assert/strict";
import {readFileSync} from "node:fs";
import vm from "node:vm";
import {translator} from "./i18n_test_support.mjs";

function environment(){
    const nodes=new Map(),sent=[],focus=[];let editing=false,foreign=false;
    const element=tag=>({tag,children:[],dataset:{},disabled:false,textContent:"",append(child){this.children.push(child);},replaceChildren(...children){this.children=children;},querySelectorAll(){return this.children.flatMap(child=>child.tag==="button"?[child]:child.querySelectorAll());},contains(child){return this.querySelectorAll().includes(child);},closest(){return this.tag==="button"?this:null;}});
    const node=id=>{if(!nodes.has(id))nodes.set(id,{...element(id),id,value:"",disabled:false,hidden:true,open:false,textContent:"",focus(){focus.push(id);},showModal(){this.open=true;},close(){this.open=false;}});return nodes.get(id);};
    const document={createElement:element,getElementById:node,querySelector:()=>foreign?{}:[...nodes.values()].find(node=>node.open)};
    const create=vm.runInNewContext(readFileSync(new URL("../assets/surface-navigation.js",import.meta.url),"utf8")+"\ncreateNavigationDialog",{document});
    let packet={connected:true,generation:7,input_epoch:2,frame:{revision:4},destination:{state:"idle"}};
    const ui=create({tr:translator("en-US"),send:value=>sent.push(JSON.parse(JSON.stringify(value))),stamp:()=>({generation:packet.generation,revision:packet.frame.revision,input_epoch:packet.input_epoch}),
        prepare:()=>focus.push("prepare"),restore:()=>focus.push("restore"),changed(){},current:()=>packet,isEditing:()=>editing});
    function fire(id,event,extra={}){
        const e={target:id==="destination-dialog"?node("destination-name"):node(id),isTrusted:true,prevented:false,stopped:false,preventDefault(){this.prevented=true;},stopPropagation(){this.stopped=true;},...extra};
        const entry=ui.bindings.find(([target,type])=>target===(id==="document"?document:node(id))&&type===event);assert.ok(entry,`${id} ${event}`);entry[2](e);return e;
    }
    return {node,ui,sent,focus,fire,edit:value=>editing=value,foreign:value=>foreign=value,update:value=>{packet={...packet,...value};ui.update(packet);}};
}

test("Cmd/Ctrl K is local, composition-aware, and cancel before confirm sends nothing",()=>{
    const env=environment();
    for(const modifier of ["metaKey","ctrlKey"]){
        const event=env.fire("document","keydown",{key:"k",code:"KeyK",[modifier]:true});
        assert.ok(event.prevented&&event.stopped&&env.node("destination-dialog").open);
        env.node("destination-name").value="draft";env.fire("destination-dialog","cancel");
        assert.equal(env.node("destination-dialog").open,false);assert.equal(env.node("destination-name").value,"");
    }
    for(const extra of [{isTrusted:false},{repeat:true},{isComposing:true},{altKey:true}]){
        env.fire("document","keydown",{key:"k",code:"KeyK",metaKey:true,...extra});assert.equal(env.node("destination-dialog").open,false);
    }
    env.edit(true);env.fire("document","keydown",{key:"k",metaKey:true});assert.equal(env.node("destination-dialog").open,false);
    env.edit(false);env.foreign(true);env.fire("document","keydown",{key:"k",metaKey:true});assert.equal(env.node("destination-dialog").open,false);
    assert.deepEqual(env.sent,[]);
});

test("IME confirmation and legacy 229 do not submit, but the next ordinary Enter does once",()=>{
    const env=environment();env.fire("destination-open","click");env.node("destination-name").value="Stella";
    env.fire("destination-name","compositionstart");
    assert.equal(env.fire("destination-dialog","keydown",{key:"Enter",keyCode:13}).prevented,false);
    env.fire("destination-name","compositionend");
    assert.equal(env.fire("destination-dialog","keydown",{key:"Enter",keyCode:229}).prevented,false);
    assert.deepEqual(env.sent,[]);
    env.fire("destination-dialog","keydown",{key:"Enter",keyCode:13});env.fire("destination-dialog","keydown",{key:"Enter",keyCode:13});
    assert.deepEqual(env.sent,[{kind:"destination_submit",request:1,stamp:{generation:7,revision:4,input_epoch:2},value:"Stella",trusted:true}]);
    assert.equal(env.node("destination-name").disabled,true);
});

test("cancel revokes one request, late replies cannot close a newer draft, and errors retain input",()=>{
    const env=environment();env.fire("destination-open","click");env.node("destination-name").value="Stella";env.fire("destination-submit","click");
    env.fire("destination-cancel","click");
    assert.deepEqual(env.sent.at(-1),{kind:"destination_cancel",operation:{generation:7,revision:4,input_epoch:2},trusted:true});
    env.fire("destination-open","click");env.node("destination-name").value="Other";env.fire("destination-submit","click");
    env.ui.receive({request:1,feedback:{status:"started"}});assert.equal(env.node("destination-dialog").open,true);
    env.ui.receive({request:2,feedback:{status:"rejected",notice:"unsupported"}});
    assert.equal(env.node("destination-note").textContent,"unsupported");assert.equal(env.node("destination-name").value,"Other");assert.equal(env.node("destination-name").disabled,false);
    env.fire("destination-submit","click");env.ui.receive({request:3,feedback:{status:"started"}});
    assert.equal(env.node("destination-dialog").open,false);assert.equal(env.node("destination-name").value,"");assert.equal(env.focus.at(-1),"restore");
});

test("active cancellation uses its original operation, and session changes release local waits",()=>{
    const env=environment();const operation={generation:7,revision:4,input_epoch:2};
    env.update({input_epoch:9,destination:{state:"active",serial:1,target:{kind:"board",board:"Stella"},operation}});
    assert.equal(env.node("destination-stop").hidden,false);env.fire("destination-stop","click");
    assert.deepEqual(env.sent.at(-1),{kind:"destination_cancel",operation,trusted:true});
    env.update({destination:{state:"idle"}});env.fire("destination-open","click");env.node("destination-name").value="Other";env.fire("destination-submit","click");
    env.update({generation:8,connected:false});assert.equal(env.node("destination-name").disabled,false);
    env.ui.receive({request:1,feedback:{status:"started"}});assert.equal(env.node("destination-dialog").open,true);
    const before=env.sent.length;env.fire("destination-submit","click");assert.equal(env.sent.length,before);assert.match(env.node("destination-note").textContent,/Connect/);
});

test("revoke clears drafts and makes late packets and bound actions inert",()=>{
    const env=environment();env.fire("destination-open","click");env.node("destination-name").value="Stella";env.fire("destination-submit","click");
    const before=env.sent.length;env.ui.revoke();env.ui.receive({request:1,feedback:{status:"started"}});env.ui.destroy();
    env.fire("destination-open","click");env.fire("destination-submit","click");env.fire("destination-stop","click");
    assert.equal(env.sent.length,before);assert.equal(env.ui.clean(),true);
});


test("recent choices only fill a draft; clearing is trusted and sends no navigation",()=>{
    const env=environment();env.ui.receiveRecent({boards:["Other","Stella"],notice:"saved"});
    assert.equal(env.node("board-recent-empty").hidden,true);
    assert.equal(env.node("board-recent-note").textContent,"saved");
    env.fire("destination-open","click");
    const button=env.node("board-recent").children[0].children[0];
    assert.equal(button.textContent,"Other");
    env.fire("destination-dialog","keydown",{key:"Enter",target:button});
    assert.deepEqual(env.sent,[]);
    env.fire("board-recent","click",{target:button,isTrusted:false});assert.equal(env.node("destination-name").value,"");
    env.fire("board-recent","click",{target:button});assert.equal(env.node("destination-name").value,"Other");
    assert.deepEqual(env.sent,[]);
    env.fire("board-recent-clear","click",{isTrusted:false});assert.deepEqual(env.sent,[]);
    env.fire("board-recent-clear","click");assert.deepEqual(env.sent,[{kind:"recent_boards_clear",trusted:true}]);
    env.ui.receiveRecent({boards:[],notice:"cleared"});assert.equal(env.node("board-recent").children.length,0);
    assert.equal(env.node("board-recent-empty").hidden,false);assert.equal(env.node("destination-name").value,"Other");
    env.fire("destination-submit","click");assert.equal(env.sent.at(-1).kind,"destination_submit");assert.equal(env.sent.at(-1).value,"Other");
    env.ui.destroy();env.ui.receiveRecent({boards:["Late"],notice:"late"});
    env.fire("board-recent-clear","click");assert.equal(env.sent.length,2);assert.equal(env.node("board-recent").children.length,0);
});

test("Enter on cancellation and recent controls does not submit a stale draft",()=>{
    const env=environment();env.fire("destination-open","click");env.node("destination-name").value="Stella";
    for(const id of ["destination-cancel","board-recent-clear"]){
        const event=env.fire("destination-dialog","keydown",{key:"Enter",target:env.node(id)});assert.equal(event.prevented,false);
    }
    assert.deepEqual(env.sent,[]);
});


test("opening during a typed destination job displays its board and optional AID, never an object",()=>{
    for(const target of [{kind:"board",board:"Stella"},{kind:"article",board:"Stella",aid:"00000101"}]){
        const env=environment(),operation={generation:7,revision:4,input_epoch:2};
        env.update({destination:{state:"active",serial:1,target,operation}});env.fire("destination-open","click");
        assert.equal(env.node("destination-name").value,target.kind==="article"?"#00000101@Stella":"Stella");assert.equal(env.node("destination-name").disabled,true);
        assert.match(env.node("destination-note").textContent,/Stella/);assert.doesNotMatch(env.node("destination-note").textContent,/object Object/);
        if(target.kind==="article")assert.match(env.node("destination-note").textContent,/#00000101/);
        assert.deepEqual(env.sent,[]);env.fire("destination-cancel","click");
        assert.deepEqual(env.sent,[{kind:"destination_cancel",operation,trusted:true}]);
    }
});


test("article URLs and AIDs remain local drafts until one explicit confirmation",()=>{
    for(const value of ["#00000101@Stella","https://www.ptt.cc/bbs/Stella/M.1.A.001.html","Stella"+" ".repeat(251)]){
        const env=environment();env.fire("destination-open","click");env.node("destination-name").value=value;
        assert.deepEqual(env.sent,[]);env.fire("destination-submit","click");env.fire("destination-submit","click");
        assert.equal(env.sent.length,1);assert.equal(env.sent[0].value,value);
        env.ui.receive({request:1,feedback:{status:"rejected",notice:"invalid target"}});
        assert.equal(env.node("destination-name").value,value);assert.equal(env.node("destination-name").disabled,false);
    }
});

test("a bookmark target opens the same local draft without sending, and active navigation refuses it",()=>{
    const env=environment();const event={isTrusted:true};
    assert.equal(env.ui.openDraft(event,"#00000101@Stella"),true);
    assert.equal(env.node("destination-name").value,"#00000101@Stella");assert.deepEqual(env.sent,[]);
    env.fire("destination-dialog","cancel");
    env.update({destination:{state:"active",serial:1,target:{kind:"article",board:"Stella",aid:"00000102"},operation:{generation:7,revision:4,input_epoch:2}}});
    assert.equal(env.ui.openDraft(event,"#00000101@Stella"),false);assert.equal(env.node("destination-dialog").open,false);
    env.ui.destroy();assert.equal(env.ui.openDraft(event,"#00000101@Stella"),false);
});

test("PTT navigation shortcuts stay closed on another station",()=>{
    const env=environment();env.update({ptt_features:false});
    env.fire("destination-open","click");env.fire("document","keydown",{key:"k",code:"KeyK",metaKey:true});
    assert.equal(env.node("destination-dialog").open,false);assert.deepEqual(env.sent,[]);
});
