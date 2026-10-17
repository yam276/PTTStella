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
    const create=vm.runInNewContext(readFileSync(new URL("../assets/surface-boards.js",import.meta.url),"utf8")+"\ncreateBoardDialog",{document});
    let packet={connected:true,generation:7,input_epoch:2,frame:{revision:4},board_navigation:{state:"idle"}};
    const ui=create({tr:translator("en-US"),send:value=>sent.push(JSON.parse(JSON.stringify(value))),stamp:()=>({generation:packet.generation,revision:packet.frame.revision,input_epoch:packet.input_epoch}),
        prepare:()=>focus.push("prepare"),restore:()=>focus.push("restore"),changed(){},current:()=>packet,isEditing:()=>editing});
    function fire(id,event,extra={}){
        const e={target:id==="board-dialog"?node("board-name"):node(id),isTrusted:true,prevented:false,stopped:false,preventDefault(){this.prevented=true;},stopPropagation(){this.stopped=true;},...extra};
        const entry=ui.bindings.find(([target,type])=>target===(id==="document"?document:node(id))&&type===event);assert.ok(entry,`${id} ${event}`);entry[2](e);return e;
    }
    return {node,ui,sent,focus,fire,edit:value=>editing=value,foreign:value=>foreign=value,update:value=>{packet={...packet,...value};ui.update(packet);}};
}

test("Cmd/Ctrl K is local, composition-aware, and cancel before confirm sends nothing",()=>{
    const env=environment();
    for(const modifier of ["metaKey","ctrlKey"]){
        const event=env.fire("document","keydown",{key:"k",code:"KeyK",[modifier]:true});
        assert.ok(event.prevented&&event.stopped&&env.node("board-dialog").open);
        env.node("board-name").value="draft";env.fire("board-dialog","cancel");
        assert.equal(env.node("board-dialog").open,false);assert.equal(env.node("board-name").value,"");
    }
    for(const extra of [{isTrusted:false},{repeat:true},{isComposing:true},{altKey:true}]){
        env.fire("document","keydown",{key:"k",code:"KeyK",metaKey:true,...extra});assert.equal(env.node("board-dialog").open,false);
    }
    env.edit(true);env.fire("document","keydown",{key:"k",metaKey:true});assert.equal(env.node("board-dialog").open,false);
    env.edit(false);env.foreign(true);env.fire("document","keydown",{key:"k",metaKey:true});assert.equal(env.node("board-dialog").open,false);
    assert.deepEqual(env.sent,[]);
});

test("IME confirmation and legacy 229 do not submit, but the next ordinary Enter does once",()=>{
    const env=environment();env.fire("board-open","click");env.node("board-name").value="Stella";
    env.fire("board-name","compositionstart");
    assert.equal(env.fire("board-dialog","keydown",{key:"Enter",keyCode:13}).prevented,false);
    env.fire("board-name","compositionend");
    assert.equal(env.fire("board-dialog","keydown",{key:"Enter",keyCode:229}).prevented,false);
    assert.deepEqual(env.sent,[]);
    env.fire("board-dialog","keydown",{key:"Enter",keyCode:13});env.fire("board-dialog","keydown",{key:"Enter",keyCode:13});
    assert.deepEqual(env.sent,[{kind:"board_submit",request:1,stamp:{generation:7,revision:4,input_epoch:2},name:"Stella",trusted:true}]);
    assert.equal(env.node("board-name").disabled,true);
});

test("cancel revokes one request, late replies cannot close a newer draft, and errors retain input",()=>{
    const env=environment();env.fire("board-open","click");env.node("board-name").value="Stella";env.fire("board-submit","click");
    env.fire("board-cancel","click");
    assert.deepEqual(env.sent.at(-1),{kind:"board_cancel",operation:{generation:7,revision:4,input_epoch:2},trusted:true});
    env.fire("board-open","click");env.node("board-name").value="Other";env.fire("board-submit","click");
    env.ui.receive({request:1,feedback:{status:"started"}});assert.equal(env.node("board-dialog").open,true);
    env.ui.receive({request:2,feedback:{status:"rejected",notice:"unsupported"}});
    assert.equal(env.node("board-note").textContent,"unsupported");assert.equal(env.node("board-name").value,"Other");assert.equal(env.node("board-name").disabled,false);
    env.fire("board-submit","click");env.ui.receive({request:3,feedback:{status:"started"}});
    assert.equal(env.node("board-dialog").open,false);assert.equal(env.node("board-name").value,"");assert.equal(env.focus.at(-1),"restore");
});

test("active cancellation uses its original operation, and session changes release local waits",()=>{
    const env=environment();const operation={generation:7,revision:4,input_epoch:2};
    env.update({input_epoch:9,board_navigation:{state:"active",serial:1,target:"Stella",operation}});
    assert.equal(env.node("board-stop").hidden,false);env.fire("board-stop","click");
    assert.deepEqual(env.sent.at(-1),{kind:"board_cancel",operation,trusted:true});
    env.update({board_navigation:{state:"idle"}});env.fire("board-open","click");env.node("board-name").value="Other";env.fire("board-submit","click");
    env.update({generation:8,connected:false});assert.equal(env.node("board-name").disabled,false);
    env.ui.receive({request:1,feedback:{status:"started"}});assert.equal(env.node("board-dialog").open,true);
    const before=env.sent.length;env.fire("board-submit","click");assert.equal(env.sent.length,before);assert.match(env.node("board-note").textContent,/Connect/);
});

test("revoke clears drafts and makes late packets and bound actions inert",()=>{
    const env=environment();env.fire("board-open","click");env.node("board-name").value="Stella";env.fire("board-submit","click");
    const before=env.sent.length;env.ui.revoke();env.ui.receive({request:1,feedback:{status:"started"}});env.ui.destroy();
    env.fire("board-open","click");env.fire("board-submit","click");env.fire("board-stop","click");
    assert.equal(env.sent.length,before);assert.equal(env.ui.clean(),true);
});


test("recent choices only fill a draft; clearing is trusted and sends no navigation",()=>{
    const env=environment();env.ui.receiveRecent({boards:["Other","Stella"],notice:"saved"});
    assert.equal(env.node("board-recent-empty").hidden,true);
    assert.equal(env.node("board-recent-note").textContent,"saved");
    env.fire("board-open","click");
    const button=env.node("board-recent").children[0].children[0];
    assert.equal(button.textContent,"Other");
    env.fire("board-dialog","keydown",{key:"Enter",target:button});
    assert.deepEqual(env.sent,[]);
    env.fire("board-recent","click",{target:button,isTrusted:false});assert.equal(env.node("board-name").value,"");
    env.fire("board-recent","click",{target:button});assert.equal(env.node("board-name").value,"Other");
    assert.deepEqual(env.sent,[]);
    env.fire("board-recent-clear","click",{isTrusted:false});assert.deepEqual(env.sent,[]);
    env.fire("board-recent-clear","click");assert.deepEqual(env.sent,[{kind:"recent_boards_clear",trusted:true}]);
    env.ui.receiveRecent({boards:[],notice:"cleared"});assert.equal(env.node("board-recent").children.length,0);
    assert.equal(env.node("board-recent-empty").hidden,false);assert.equal(env.node("board-name").value,"Other");
    env.fire("board-submit","click");assert.equal(env.sent.at(-1).kind,"board_submit");assert.equal(env.sent.at(-1).name,"Other");
    env.ui.destroy();env.ui.receiveRecent({boards:["Late"],notice:"late"});
    env.fire("board-recent-clear","click");assert.equal(env.sent.length,2);assert.equal(env.node("board-recent").children.length,0);
});

test("Enter on cancellation and recent controls does not submit a stale draft",()=>{
    const env=environment();env.fire("board-open","click");env.node("board-name").value="Stella";
    for(const id of ["board-cancel","board-recent-clear"]){
        const event=env.fire("board-dialog","keydown",{key:"Enter",target:env.node(id)});assert.equal(event.prevented,false);
    }
    assert.deepEqual(env.sent,[]);
});
