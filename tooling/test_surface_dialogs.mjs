// Unit tests with explicit DOM/clock doubles; these are not native input proof.
import test from "node:test";
import assert from "node:assert/strict";
import {readFileSync} from "node:fs";
import vm from "node:vm";

function environment(file,factory){
    const nodes=new Map(),focus=[],sent=[],renders=[],timers=new Map();let nextTimer=0;
    function element(id){return {id,value:"",checked:false,disabled:false,open:false,dataset:{},children:[],textContent:"",
        showModal(){this.open=true;},close(){this.open=false;},focus(){focus.push(id);},
        replaceChildren(){this.children=[];},append(value){this.children.push(value);}};}
    const document={getElementById(id){if(!nodes.has(id))nodes.set(id,element(id));return nodes.get(id);},createElement:()=>element("option")};
    const context={document,setTimeout:fn=>{const id=++nextTimer;timers.set(id,fn);return id;},clearTimeout:id=>timers.delete(id)};
    const create=vm.runInNewContext(readFileSync(new URL("../assets/"+file,import.meta.url),"utf8")+"\n"+factory,context);
    let ui;
    return {nodes,focus,sent,renders,timers,node:document.getElementById,
        create(extra={}){ui=create({send:value=>sent.push(value),beforeRequest(){},beforeShow(){},restore(){},changed(){},
            loadFonts:async()=>"",fontLoadNote:()=>"loaded",render:(...args)=>renders.push(args),
            prepare(){},stamp:()=>({generation:7,revision:9,input_epoch:0}),focus(){},current:()=>null,invalidate(){},reset(){},...extra});return ui;},
        fire(id,event="click"){const binding=ui.bindings.find(([target,type])=>target.id===id&&type===event);assert.ok(binding,id+" "+event);binding[2]({target:document.getElementById(id),isTrusted:true,preventDefault(){}});},
        tick(){const work=[...timers.values()];timers.clear();for(const fn of work)fn();},
    };
}
const display={ascii_font:"ASCII",cjk_font:"中文",font_size:20,baseline:20};
const openData=()=>({profile:{fonts:{kind:"platform_default"}},values:{...display},defaults:{...display},presets:{},sample:{settings:{...display}},warning:null});
function deferred(){let resolve;const promise=new Promise(done=>{resolve=done;});return {promise,resolve};}

test("closing a settings dialog while fonts load cannot repaint or steal focus",async()=>{
    const env=environment("surface-settings.js","createSettingsDialog"),load=deferred();
    const ui=env.create({loadFonts:()=>load.promise});
    const opening=ui.open(openData());assert.equal(env.node("font-dialog").open,true);
    env.fire("font-cancel");load.resolve("");await opening;
    assert.equal(env.renders.length,0);assert.equal(env.focus.length,0);
    ui.destroy();assert.equal(ui.clean(),true);
});

test("an old preview finishing after a newer draft cannot replace it",async()=>{
    const env=environment("surface-settings.js","createSettingsDialog");let slow=null;
    const ui=env.create({loadFonts:()=>slow?slow.promise:Promise.resolve("")});
    await ui.open(openData());env.renders.length=0;
    env.node("font-size").value="18";env.fire("font-size","input");env.tick();
    const first=env.sent.at(-1);slow=deferred();
    const receiving=ui.preview({request:first.request,packet:{settings:{...display,font_size:18}}});
    env.node("font-size").value="22";env.fire("font-size","input");env.tick();
    const second=env.sent.at(-1);assert.equal(second.request,first.request+1);
    slow.resolve("");await receiving;assert.equal(env.renders.length,0);
    slow=null;await ui.preview({request:second.request,packet:{settings:{...display,font_size:22}}});
    assert.equal(env.renders.length,1);assert.equal(env.renders[0][1].settings.font_size,22);
    env.fire("font-size","input");assert.equal(env.timers.size,1);
    ui.revoke();assert.equal(env.timers.size,0);ui.destroy();assert.equal(ui.clean(),true);
});

test("revoking an open settings operation retains its await but prevents late effects",async()=>{
    const env=environment("surface-settings.js","createSettingsDialog"),load=deferred();
    const ui=env.create({loadFonts:()=>load.promise});const opening=ui.open(openData());
    ui.revoke();ui.destroy();load.resolve("");await opening;
    assert.equal(env.renders.length,0);assert.equal(env.focus.length,0);assert.equal(ui.clean(),true);
});

test("login submit clears the draft immediately and destruction ignores late results",()=>{
    const env=environment("surface-login.js","createLoginDialog"),ui=env.create();
    env.fire("login-open");ui.receive({state:"opened",account:"Synthetic",saved:true,message:"loaded"});
    env.node("login-password").value="synthetic-only";env.node("login-remember").checked=true;
    env.fire("login-submit");assert.equal(env.node("login-password").value,"");
    assert.equal(env.sent.at(-1).password,"synthetic-only");
    env.fire("login-cancel");assert.equal(env.node("login-remember").checked,false);
    ui.destroy();ui.receive({state:"opened",account:"Late",saved:true,message:"late"});
    assert.equal(env.node("login-account").value,"");assert.equal(ui.clean(),true);
});

test("filter edit admission, failure recovery and presentation blocking share one owner",()=>{
    const env=environment("surface-filters.js","createFiltersDialog");
    const ui=env.create({current:()=>({presentation:3,generation:7,frame:{revision:9},revealed:false})});
    env.fire("filters-open");ui.open({entries:[],warning:null});
    env.node("filters-id").value="Alice";env.node("filters-note").value="note";
    env.fire("filters-save");env.fire("filters-save");
    assert.equal(env.sent.filter(e=>e.kind==="filters_edit").length,1);assert.equal(ui.blocked(),true);
    ui.receive({state:"rejected",message:"write failed"});assert.equal(ui.blocked(),false);
    env.fire("filters-save");assert.equal(env.sent.filter(e=>e.kind==="filters_edit").length,2);
    ui.paint({masked_pushes:2,revealed:false,filter_reason:"Alice"});assert.equal(ui.blocked(),false);
    env.fire("filters-reveal");assert.equal(env.sent.at(-1).presentation,3);assert.equal(ui.blocked(),true);
    ui.destroy();assert.equal(ui.clean(),true);
});

test("the tagged router delivers independent wire examples once and rejects legacy/unknown envelopes",()=>{
    const create=vm.runInNewContext(readFileSync(new URL("../assets/surface-packets.js",import.meta.url),"utf8")+"\ncreateSurfaceRouter");
    const kinds=["paint","notice","surface_probe","filters_open","filters_result","login_result","keyboard_state",
        "hover_settings","media_hover","hover_reset","hover_rejected","media_open","media_image","media_error","media_external",
        "media_reset","settings_open","font_preview","profile_result","settings_saved","selection","paste_result","input_rejected","navigation_result","link_result","export_result"];
    const calls=[],handlers=Object.fromEntries(kinds.map(kind=>[kind,payload=>calls.push({kind,payload})]));
    const route=create(handlers),examples=JSON.parse(readFileSync(new URL("../tests/fixtures/ui-packets.json",import.meta.url),"utf8"));
    for(const {packet} of examples){
        if(packet.kind==="stop")continue;
        const before=calls.length;route(packet);assert.equal(calls.length,before+1);
        assert.equal(calls.at(-1).kind,packet.kind);assert.deepEqual(calls.at(-1).payload,packet.payload);
    }
    for(const packet of [null,[],{}, {notice:"legacy"},{kind:"paint"},{kind:"unknown",payload:{}},{kind:"notice",payload:"x",frame:{}}]){
        const before=calls.length;assert.throws(()=>route(packet));assert.equal(calls.length,before);
    }
    assert.throws(()=>create({}));
});
