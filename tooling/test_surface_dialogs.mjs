// Unit tests with explicit DOM/clock doubles; these are not native input proof.
import test from "node:test";
import assert from "node:assert/strict";
import {readFileSync} from "node:fs";
import vm from "node:vm";
import {translator} from "./i18n_test_support.mjs";

function environment(file,factory){
    const nodes=new Map(),focus=[],sent=[],renders=[],timers=new Map();let nextTimer=0;
    function element(id){return {id,value:"",checked:false,disabled:false,open:false,dataset:{},children:[],textContent:"",
        showModal(){this.open=true;},close(){this.open=false;},focus(){focus.push(id);},
        replaceChildren(...values){this.children=values;},append(value){this.children.push(value);}};}
    const document={getElementById(id){if(!nodes.has(id))nodes.set(id,element(id));return nodes.get(id);},querySelectorAll:()=>[document.getElementById("font-source-cascadia"),document.getElementById("font-source-microsoft_jhenghei")],createElement:()=>element("option")};
    document.getElementById("font-source-cascadia").dataset={fontSource:"cascadia",fontFamilies:"Cascadia Mono"};
    document.getElementById("font-source-microsoft_jhenghei").dataset={fontSource:"microsoft_jhenghei",fontFamilies:"Microsoft JhengHei"};
    const context={document,setTimeout:fn=>{const id=++nextTimer;timers.set(id,fn);return id;},clearTimeout:id=>timers.delete(id)};
    const create=vm.runInNewContext(readFileSync(new URL("../assets/"+file,import.meta.url),"utf8")+"\n"+factory,context);
    let ui;
    return {nodes,focus,sent,renders,timers,node:document.getElementById,
        create(extra={}){ui=create({tr:translator("zh-TW"),send:value=>sent.push(value),beforeRequest(){},beforeShow(){},restore(){},changed(){},
            loadFonts:async()=>"",fontLoadNote:()=>"loaded",invalidateFonts(){},render:(...args)=>renders.push(args),
            prepare(){},stamp:()=>({generation:7,revision:9,input_epoch:0}),focus(){},current:()=>null,invalidate(){},reset(){},...extra});return ui;},
        fire(id,event="click",trusted=true){const binding=ui.bindings.find(([target,type])=>target.id===id&&type===event);assert.ok(binding,id+" "+event);binding[2]({target:document.getElementById(id),isTrusted:trusted,preventDefault(){}});},
        tick(){const work=[...timers.values()];timers.clear();for(const fn of work)fn();},
    };
}
const display={ascii_font:"ASCII",cjk_font:"中文",font_size:20,baseline:20};
test("the production clearSelection sends a scoped revocation before dropping its cache",()=>{
    const packetSource=readFileSync(new URL("../assets/surface-packets.js",import.meta.url),"utf8");
    const revoke=vm.runInNewContext(packetSource+"\nrevokeSurfaceSelection");
    const source=readFileSync(new URL("../assets/surface.js",import.meta.url),"utf8");
    const start=source.indexOf("function clearSelection()"),end=source.indexOf("function requestSelection(",start);
    assert.ok(start>=0&&end>start);
    const expected=JSON.parse(readFileSync(new URL("../tests/fixtures/selection-cleared.json",import.meta.url),"utf8"));
    const events=[],context={stopped:false,selectionActive:true,selectionFingerprint:"selected",selectionNext:null,
        selectionCache:{...expected,kind:undefined,value:{text:"synthetic"}},selectionBusy:null,
        selectionLayer:{replaceChildren(){}},surfaceNote:{textContent:""},surface:{dataset:{}},
        refreshExportControls(){},revokeSurfaceSelection:revoke,send:packet=>events.push(packet)};
    vm.runInNewContext(source.slice(start,end)+"\nclearSelection();",context);
    assert.deepEqual(JSON.parse(JSON.stringify(events)),[expected]);
    assert.equal(context.selectionCache,null);assert.equal(context.selectionActive,false);
    context.selectionBusy={...expected,request:12};events.length=0;
    vm.runInNewContext(source.slice(start,end)+"\nclearSelection();",context);
    assert.equal(events[0].request,12);
    context.stopped=true;events.length=0;
    vm.runInNewContext(source.slice(start,end)+"\nclearSelection();",context);
    assert.equal(events.length,0);
});
const openData=()=>({language:"zh-TW",profile:{fonts:{kind:"platform_default"}},values:{...display},defaults:{...display},presets:{},sample:{settings:{...display}},warning:null});
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
        "media_reset","settings_open","font_catalog","font_preview","profile_result","settings_saved","selection","paste_result","input_rejected","navigation_result","link_result","export_result"];
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


test("language is a settings draft: cancel sends no save, failure retains the draft, success commits once",async()=>{
    const env=environment("surface-settings.js","createSettingsDialog"),ui=env.create();
    await ui.open(openData());env.sent.length=0;env.node("interface-language").value="en-US";
    env.fire("font-cancel");assert.equal(env.sent.length,0);
    await ui.open(openData());env.sent.length=0;assert.equal(env.node("interface-language").value,"zh-TW");
    env.node("interface-language").value="en-US";env.fire("font-save");env.fire("font-save");
    assert.equal(env.sent.length,1);assert.equal(env.sent[0].language,"en-US");
    ui.saved({status:"rejected"});assert.equal(env.node("font-dialog").open,true);
    assert.equal(env.node("interface-language").value,"en-US");
    env.fire("font-save");assert.equal(env.sent.length,2);
    ui.saved({status:"saved"});assert.equal(env.node("font-dialog").open,false);
    ui.destroy();assert.equal(ui.clean(),true);
});


test("local font selection changes only a draft, ignores stale catalogs and refresh does not save",async()=>{
    const env=environment("surface-settings.js","createSettingsDialog");let refreshed=0;
    const ui=env.create({invalidateFonts(){refreshed++;}});await ui.open(openData());
    const request=env.sent.at(-1).request;
    ui.catalog({request,families:["Menlo","Songti TC",'<img src=x onerror=alert(1)>'],notice:null});
    assert.equal(env.node("font-local-ascii").disabled,false);
    assert.equal(env.node("font-local-ascii").value,"");
    assert.equal(env.node("font-local-ascii").children[3].textContent,'<img src=x onerror=alert(1)>');
    env.node("font-local-ascii").value="Menlo";env.fire("font-local-ascii","change");
    assert.equal(env.node("font-ascii").value,"Menlo");assert.equal(env.node("font-preset").value,"custom");
    env.node("font-local-cjk").value="Songti TC";env.fire("font-local-cjk","change");env.tick();
    assert.equal(env.sent.at(-1).values.fonts.cjk_font,"Songti TC");
    assert.equal(env.sent.at(-1).values.fonts.ascii_font,"Menlo");
    env.fire("font-refresh","click",false);assert.equal(refreshed,0);
    env.fire("font-refresh");assert.equal(refreshed,1);const next=env.sent.at(-1).request;
    ui.catalog({request,families:["Stale"],notice:null});assert.notEqual(env.node("font-local-ascii").children[1].textContent,"Stale");
    ui.catalog({request:next,families:[],notice:"scan failed"});assert.equal(env.node("font-local-ascii").disabled,true);
    assert.equal(env.node("font-local-status").textContent,"scan failed");
    assert.equal(env.node("font-ascii").value,"Menlo");assert.equal(env.sent.some(e=>e.kind==="settings_save"),false);
    env.fire("font-cancel");ui.catalog({request:next,families:["Late"],notice:null});
    assert.equal(env.node("font-dialog").open,false);ui.destroy();
});

test("official source buttons match selected families and require an explicit trusted click",async()=>{
    const env=environment("surface-settings.js","createSettingsDialog"),ui=env.create();
    const data=openData();Object.assign(data.values,{ascii_font:"Cascadia Mono",cjk_font:"Microsoft JhengHei"});
    await ui.open(data);assert.equal(env.node("font-source-microsoft_jhenghei").hidden,false);
    assert.equal(env.sent.some(e=>e.kind==="font_source"),false);
    env.fire("font-source-microsoft_jhenghei","click",false);assert.equal(env.sent.some(e=>e.kind==="font_source"),false);
    env.fire("font-source-microsoft_jhenghei");assert.equal(env.sent.at(-1).source,"microsoft_jhenghei");
    assert.equal(Object.hasOwn(env.sent.at(-1),"url"),false);
    env.node("font-cjk").value="Unknown";env.fire("font-cjk","input");
    assert.equal(env.node("font-source-microsoft_jhenghei").hidden,true);
    const count=env.sent.length;env.fire("font-source-microsoft_jhenghei");assert.equal(env.sent.length,count);
    ui.destroy();env.fire("font-source-cascadia");assert.equal(env.sent.length,count);
});


test("traditional preset choices preserve family intent and current size until explicit save",async()=>{
    const env=environment("surface-settings.js","createSettingsDialog"),ui=env.create(),data=openData();
    data.presets={windows_classic:{ascii_font:"MingLiU",cjk_font:"MingLiU",font_size:20,baseline:20},macos_bbs:{ascii_font:"Songti TC",cjk_font:"Songti TC",font_size:20,baseline:20}};
    await ui.open(data);env.node("font-size").value="22";env.node("font-baseline").value="19";
    for(const [preset,family] of [["windows_classic","MingLiU"],["macos_bbs","Songti TC"]]){
        env.node("font-preset").value=preset;env.fire("font-preset","change");env.tick();
        assert.equal(env.node("font-ascii").value,family);assert.equal(env.node("font-cjk").value,family);
        assert.equal(env.node("font-size").value,"22");assert.equal(env.node("font-baseline").value,"19");
        const next=env.sent.at(-1);assert.equal(next.kind,"settings_preview");assert.equal(next.values.fonts.preset,preset);
        await ui.preview({request:next.request,packet:{settings:{...display,ascii_font:family,cjk_font:family,font_size:22,baseline:19}}});
    }
    assert.equal(env.sent.some(e=>e.kind==="settings_save"),false);env.fire("font-save");
    assert.equal(env.sent.at(-1).values.fonts.preset,"macos_bbs");ui.destroy();
});
