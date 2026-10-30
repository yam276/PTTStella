// Unit tests with explicit DOM/clock doubles; these are not native input proof.
import test from "node:test";
import assert from "node:assert/strict";
import {readFileSync} from "node:fs";
import vm from "node:vm";
import {translator} from "./i18n_test_support.mjs";

function environment(file,factory){
    const nodes=new Map(),focus=[],sent=[],renders=[],timers=new Map();let nextTimer=0;
    function element(id){return {id,value:"",checked:false,disabled:false,open:false,dataset:{},attrs:{},children:[],textContent:"",
        setAttribute(key,value){this.attrs[key]=value;},get options(){return this.children;},
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
        fire(id,event="click",trusted=true,extra={}){const binding=ui.bindings.find(([target,type])=>target.id===id&&type===event);assert.ok(binding,id+" "+event);binding[2]({target:document.getElementById(id),isTrusted:trusted,preventDefault(){},stopPropagation(){},...extra});},
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
const openData=()=>({language:"zh-TW",tools_opacity:100,tools_access:{right_click_menu:true,show_floating_button:true},profile:{fonts:{kind:"platform_default"}},values:{...display},defaults:{...display},presets:{},sample:{settings:{...display}},warning:null});
test("trusted tool edits autosave, coalesce while writing, and retain failures for retry",async()=>{
    const env=environment("surface-settings.js","createSettingsDialog"),ui=env.create();
    await ui.open(openData());env.node("tools-show-ball").checked=false;
    env.fire("tools-show-ball","change",false);env.tick();assert.equal(env.sent.filter(x=>x.kind==="settings_save").length,0);
    env.fire("tools-show-ball","change");env.tick();assert.equal(env.sent.at(-1).tools_access.show_floating_button,false);
    env.node("tools-right-click").checked=false;env.fire("tools-right-click","change");env.tick();
    assert.equal(env.sent.filter(x=>x.kind==="settings_save").length,1);
    ui.saved({status:"saved"});assert.equal(env.sent.filter(x=>x.kind==="settings_save").length,2);
    assert.equal(env.sent.at(-1).tools_access.right_click_menu,false);
    ui.saved({status:"rejected",notice:"disk full"});assert.equal(env.node("font-note").textContent,"disk full");assert.equal(env.node("font-save").hidden,false);
    env.fire("font-save");ui.saved({status:"saved"});assert.equal(env.node("font-dialog").open,true);
    env.fire("font-cancel");assert.equal(env.node("font-dialog").open,false);ui.destroy();
});
function deferred(){let resolve;const promise=new Promise(done=>{resolve=done;});return {promise,resolve};}

test("finishing embedded login closes settings without requiring a DOM event",async()=>{
    const env=environment("surface-settings.js","createSettingsDialog");let closed=0;
    const ui=env.create({onClose(){closed++;}});
    await ui.open(openData());
    assert.doesNotThrow(()=>ui.close({isTrusted:true}));
    assert.equal(env.node("font-dialog").open,false);assert.equal(closed,1);
    await ui.open(openData());env.node("tools-show-ball").checked=false;
    env.fire("tools-show-ball","change");
    ui.close({isTrusted:true});assert.equal(env.node("font-dialog").open,true);
    assert.equal(env.sent.at(-1).kind,"settings_save");
    ui.saved({status:"saved"});assert.equal(env.node("font-dialog").open,false);assert.equal(closed,2);
    ui.destroy();
});

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
    const first=env.sent.filter(x=>x.kind==="settings_preview").at(-1);slow=deferred();
    const receiving=ui.preview({request:first.request,packet:{settings:{...display,font_size:18}}});
    env.node("font-size").value="22";env.fire("font-size","input");env.tick();
    const second=env.sent.filter(x=>x.kind==="settings_preview").at(-1);assert.equal(second.request,first.request+1);
    slow.resolve("");await receiving;assert.equal(env.renders.length,0);
    slow=null;await ui.preview({request:second.request,packet:{settings:{...display,font_size:22}}});
    assert.equal(env.renders.length,1);assert.equal(env.renders[0][1].settings.font_size,22);
    env.fire("font-size","input");assert.equal(env.timers.size,2);
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

test("each account policy survives selection and is sent explicitly on save",()=>{
    for(const [policy,label] of [["mask","黑名單（遮內容、保留 ID）"],["mask_row","黑名單（整行塗黑）"],["allow","白名單（ID 標色）"]]){
        const env=environment("surface-filters.js","createFiltersDialog"),ui=env.create();
        env.fire("filters-open");ui.open({entries:[{id:"Alice",note:"",policy}]});
        if(policy==="allow")env.fire("filters-tab-allow");
        assert.equal(env.node("filters-list").children.length,1);
        assert.ok(env.node("filters-list").children[0].textContent.startsWith("Alice · "));
        assert.ok(env.node("filters-list").children[0].textContent.includes(policy==="allow"?"使用共用顏色":label));
        env.node("filters-list").value="Alice";env.fire("filters-list","change");
        assert.equal(env.node("filters-policy").value,policy);
        env.fire("filters-save");assert.equal(env.sent.at(-1).policy,policy);
        ui.receive({state:"saved",entries:[{id:"Alice",note:"",policy}],message:"saved"});
        assert.equal(env.node("filters-list").children[0].value,"Alice");
        ui.destroy();
    }
});

test("the tagged router delivers independent wire examples once and rejects legacy/unknown envelopes",()=>{
    const create=vm.runInNewContext(readFileSync(new URL("../assets/surface-packets.js",import.meta.url),"utf8")+"\ncreateSurfaceRouter");
    const kinds=["localization","diagnostics","update_status","article_updates","station_mouse_rejected","paint","notice","surface_probe","filters_open","filters_result","login_result","keyboard_state",
        "hover_settings","tools_opacity","tools_access","sites_open","sites_result","media_hover","hover_reset","hover_rejected","media_open","media_image","media_error","media_external",
        "media_reset","reading_move","inline_open","inline_image","inline_error","inline_reset","settings_visibility","settings_close","settings_open","font_catalog","font_preview","profile_result","settings_saved","selection","paste_result","input_rejected","navigation_result","destination_result","recent_boards","bookmarks_open","bookmarks_result","archive","bookmark_capture","link_result","export_result"];
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


test("language autosaves without closing, and closing flushes the final edit",async()=>{
    const env=environment("surface-settings.js","createSettingsDialog"),ui=env.create();
    await ui.open(openData());env.node("interface-language").value="en-US";
    env.fire("interface-language","change");env.tick();assert.equal(env.sent.at(-1).language,"en-US");
    ui.saved({status:"saved"});assert.equal(env.node("font-dialog").open,true);
    env.node("tools-opacity").value="30";env.fire("tools-opacity","input");env.fire("font-cancel");
    assert.equal(env.sent.at(-1).tools_opacity,30);assert.equal(env.node("font-dialog").open,true);
    ui.saved({status:"saved"});assert.equal(env.node("font-dialog").open,false);ui.destroy();
});

test("local font selection autosaves, ignores stale catalogs and refresh does not save again",async()=>{
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
    assert.equal(env.sent.filter(e=>e.kind==="settings_save").length,1);ui.saved({status:"saved"});
    env.fire("font-refresh","click",false);assert.equal(refreshed,0);
    env.fire("font-refresh");assert.equal(refreshed,1);const next=env.sent.at(-1).request;
    ui.catalog({request,families:["Stale"],notice:null});assert.notEqual(env.node("font-local-ascii").children[1].textContent,"Stale");
    ui.catalog({request:next,families:[],notice:"scan failed"});assert.equal(env.node("font-local-ascii").disabled,true);
    assert.equal(env.node("font-local-status").textContent,"scan failed");
    assert.equal(env.node("font-ascii").value,"Menlo");assert.equal(env.sent.filter(e=>e.kind==="settings_save").length,1);
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


test("traditional preset autosaves preserve family intent and current size",async()=>{
    const env=environment("surface-settings.js","createSettingsDialog"),ui=env.create(),data=openData();
    data.presets={windows_classic:{ascii_font:"MingLiU",cjk_font:"MingLiU",font_size:20,baseline:20},macos_bbs:{ascii_font:"Songti TC",cjk_font:"Songti TC",font_size:20,baseline:20}};
    await ui.open(data);env.node("font-size").value="22";env.node("font-baseline").value="19";
    for(const [preset,family] of [["windows_classic","MingLiU"],["macos_bbs","Songti TC"]]){
        env.node("font-preset").value=preset;env.fire("font-preset","change");env.tick();
        assert.equal(env.node("font-ascii").value,family);assert.equal(env.node("font-cjk").value,family);
        assert.equal(env.node("font-size").value,"22");assert.equal(env.node("font-baseline").value,"19");
        const next=env.sent.filter(x=>x.kind==="settings_preview").at(-1);assert.equal(next.kind,"settings_preview");assert.equal(next.values.fonts.preset,preset);
        await ui.preview({request:next.request,packet:{settings:{...display,ascii_font:family,cjk_font:family,font_size:22,baseline:19}}});
    }
    ui.saved({status:"saved"});
    assert.equal(env.sent.filter(x=>x.kind==="settings_save").at(-1).values.fonts.preset,"macos_bbs");ui.destroy();
});

test("opacity changes autosave, and restored defaults are persisted without resetting language",async()=>{
    const env=environment("surface-settings.js","createSettingsDialog"),ui=env.create({page:()=>"input"});
    await ui.open({...openData(),language:"en-US",tools_opacity:65});
    env.node("tools-opacity").value="35";env.fire("tools-opacity","input");env.tick();
    assert.equal(env.sent.at(-1).tools_opacity,35);ui.saved({status:"saved"});
    env.fire("font-defaults");env.tick();assert.equal(env.sent.at(-1).tools_opacity,100);assert.equal(env.sent.at(-1).language,"en-US");ui.destroy();
});

test("PTT login stays disabled on another station and on an unsupported platform",()=>{
    const env=environment("surface-login.js","createLoginDialog"),ui=env.create();
    ui.update({ptt_features:false,login_pending:false});assert.equal(env.node("login-open").disabled,true);
    ui.update({ptt_features:true,login_pending:false});assert.equal(env.node("login-open").disabled,false);
    env.node("login-open").dataset.platformSupported="false";ui.update({ptt_features:true,login_pending:false});assert.equal(env.node("login-open").disabled,true);ui.destroy();
});


test("account policy is explicit, restored on selection, and discarded with unsaved edits",()=>{
    const env=environment("surface-filters.js","createFiltersDialog"),ui=env.create();
    env.fire("filters-open");ui.open({entries:[{id:"Alice",note:"exception",policy:"allow"}],warning:null});
    env.fire("filters-tab-allow");
    env.node("filters-list").value="Alice";env.fire("filters-list","change");
    assert.equal(env.node("filters-policy").value,"allow");
    env.fire("filters-save","click",false);assert.equal(env.sent.filter(e=>e.kind==="filters_edit").length,0);
    env.fire("filters-save");assert.equal(env.sent.at(-1).policy,"allow");
    ui.receive({state:"rejected",message:"failed"});assert.equal(env.node("filters-policy").value,"allow");
    env.fire("filters-close");assert.equal(env.node("filters-policy").value,"allow");
    assert.equal(env.node("filters-id").value,"");
    ui.destroy();const count=env.sent.length;env.fire("filters-save");assert.equal(env.sent.length,count);
});


test("import preview requires explicit apply and cancelled late results cannot replace a new draft",()=>{
    const env=environment("surface-filters.js","createFiltersDialog"),ui=env.create();
    env.fire("filters-open");ui.open({entries:[{id:"Local",note:"",policy:"allow"}]});
    env.fire("filters-import","click",false);assert.equal(env.sent.at(-1).kind,"filters_open");
    env.fire("filters-import");assert.equal(env.sent.at(-1).kind,"filters_import");const first=env.sent.at(-1).request;
    env.fire("filters-import");assert.equal(env.sent.filter(x=>x.kind==="filters_import").length,1);
    env.fire("filters-close");assert.equal(env.sent.at(-1).kind,"filters_import_cancel");
    env.fire("filters-open");ui.open({entries:[{id:"Local",note:"",policy:"allow"}]});env.fire("filters-import");const second=env.sent.at(-1).request;
    ui.receive({state:"preview",request:first,entries:[{id:"Stale",note:"",policy:"mask"}],message:"stale"});
    assert.equal(env.node("filters-import-apply").hidden,true);
    ui.receive({state:"preview",request:second,entries:[{id:"Local",note:"",policy:"allow"},{id:"Imported",note:"",policy:"mask"}],message:"preview"});
    assert.equal(env.node("filters-import-apply").hidden,false);assert.equal(env.node("filters-save").disabled,true);
    assert.equal(env.sent.filter(x=>x.kind==="filters_import_apply").length,0);
    env.fire("filters-import-apply","click",false);assert.equal(env.sent.filter(x=>x.kind==="filters_import_apply").length,0);
    env.fire("filters-import-apply");env.fire("filters-import-apply");assert.equal(env.sent.filter(x=>x.kind==="filters_import_apply").length,1);
    assert.equal(ui.blocked(),true);
    ui.receive({state:"rejected",message:"save failed"});assert.equal(ui.blocked(),false);
    assert.equal(env.node("filters-list").children.length,0);
    env.fire("filters-tab-allow");assert.equal(env.node("filters-list").children.length,1);
    assert.equal(env.node("filters-list").children[0].value,"Local");assert.equal(env.node("filters-import").disabled,false);
    ui.destroy();assert.equal(ui.clean(),true);
});

test("invalid fields and composition cannot persist partial edits, and shutdown retains the latest complete snapshot",async()=>{
 const env=environment("surface-settings.js","createSettingsDialog"),ui=env.create();await ui.open(openData());
 env.node("font-size").checkValidity=()=>false;env.node("font-size").value="";env.fire("font-size","input");env.tick();
 assert.equal(env.sent.filter(x=>x.kind==="settings_save").length,0);assert.throws(()=>ui.snapshot(),/尚未完整/);
 env.node("font-size").checkValidity=()=>true;env.node("font-size").value="18";env.fire("font-size","compositionstart");env.fire("font-size","input");env.tick();
 assert.equal(env.sent.filter(x=>x.kind==="settings_save").length,0);
 env.fire("font-size","compositionend");assert.equal(ui.snapshot().values.font_size,18);
 env.tick();assert.equal(env.sent.filter(x=>x.kind==="settings_save").length,1);
 env.node("font-size").value="22";env.fire("font-size","input");assert.equal(ui.snapshot().values.font_size,22);
 ui.destroy();assert.equal(env.timers.size,0);
});

test("an accepted profile is retained for autosave and shutdown while its fonts are still loading",async()=>{
 const env=environment("surface-settings.js","createSettingsDialog");let slow=null;
 const ui=env.create({loadFonts:()=>slow?slow.promise:Promise.resolve("")});await ui.open(openData());
 slow=deferred();const values={...display,font_size:18,baseline:19};
 const receiving=ui.profile({status:"imported",profile:{name:"Synthetic",profile:{fonts:{kind:"platform_default"}},values,sample:{settings:values}}});
 assert.equal(ui.snapshot()?.values.font_size,18,'accepted import must survive closing before preview resolves');
 env.tick();assert.equal(env.sent.at(-1).kind,"settings_save");assert.equal(env.sent.at(-1).values.font_size,18);
 ui.destroy();const renders=env.renders.length;slow.resolve("");await receiving;
 assert.equal(env.renders.length,renders);assert.equal(ui.clean(),true);
});

test("live locale refresh relabels cached font choices without a second catalog request or losing the draft",async()=>{
 const env=environment('surface-settings.js','createSettingsDialog');let prefix='中文';
 const ui=env.create({tr:(key,args)=>prefix+key+(args?.count??''),fontLoadNote:()=>prefix+'loaded'});await ui.open(openData());
 const request=env.sent.find(x=>x.kind==='settings_fonts').request;ui.catalog({request,families:['ASCII','中文']});
 env.node('font-ascii').value='draft font';const count=env.sent.length;prefix='English';ui.relocalize();
 assert.equal(env.sent.length,count);assert.equal(env.node('font-local-ascii').children[0].textContent,'Englishfont-local-choose');assert.equal(env.node('font-ascii').value,'draft font');assert.equal(env.node('font-availability').textContent,'Englishloaded');
});

test("article mode saves, retries, survives close snapshots, and resets only in media",async()=>{
    const env=environment("surface-settings.js","createSettingsDialog");let page="appearance";
    const ui=env.create({page:()=>page});await ui.open({...openData(),article_mode:"terminal"});
    assert.equal(env.node("article-mode").value,"terminal");
    env.node("article-mode").value="web";env.fire("article-mode","change");env.tick();
    assert.equal(env.sent.at(-1).article_mode,"web");ui.saved({status:"rejected"});
    assert.equal(ui.snapshot().article_mode,"web");env.fire("font-save");ui.saved({status:"saved"});
    env.node("article-mode").value="terminal";env.fire("article-mode","change");env.tick();ui.saved({status:"saved"});
    env.fire("font-defaults");env.tick();assert.equal(env.sent.at(-1).article_mode,"terminal");ui.saved({status:"saved"});
    page="media";env.fire("font-defaults");env.tick();assert.equal(env.sent.at(-1).article_mode,"web");
    ui.saved({status:"saved"});ui.destroy();
});


test("allowlist shared and individual colors require explicit trusted saves and recover failures",()=>{
    const env=environment("surface-filters.js","createFiltersDialog"),ui=env.create();
    env.fire("filters-open");ui.open({allow_color:10,entries:[{id:"Alice",policy:"allow",note:"",color:13},{id:"Bob",policy:"allow",note:""}]});
    assert.equal(env.node("filters-default-color").value,"10");
    env.fire("filters-tab-allow");
    env.node("filters-list").value="Alice";env.fire("filters-list","change");
    assert.equal(env.node("filters-color").value,"13");assert.equal(env.node("filters-color").disabled,false);
    env.node("filters-color").value="";env.fire("filters-save");assert.equal(env.sent.at(-1).color,null);
    ui.receive({state:"saved",allow_color:10,entries:[{id:"Alice",policy:"allow",note:""}],message:"saved"});
    env.node("filters-default-color").value="12";env.fire("filters-default-color-save","click",false);
    assert.equal(env.sent.filter(e=>e.kind==="filters_default_color").length,0);
    env.fire("filters-default-color-save");assert.equal(env.sent.at(-1).color,12);
    assert.equal(env.node("filters-save").disabled,true);
    ui.receive({state:"rejected",message:"disk failed"});assert.equal(env.node("filters-default-color").value,"10");
    env.fire("filters-tab-allow");
    env.node("filters-default-color").value="12";env.fire("filters-default-color-save");
    ui.receive({state:"saved",allow_color:12,entries:[],message:"saved"});assert.equal(env.node("filters-default-color").value,"12");
    env.fire("filters-tab-block");assert.equal(env.node("filters-color").disabled,true);
    assert.equal(env.node("filters-color-field").hidden,true);
    env.fire("filters-close");assert.equal(env.node("filters-color").value,"");ui.destroy();
});

test('embedded login clears secret drafts on category exit and ignores late vault replies',()=>{
 const env=environment('surface-login.js','createLoginDialog');let presentations=0;
 const ui=env.create({manage:()=>true,present:dialog=>{presentations++;dialog.open=true;},visible:()=>false});
 ui.request({isTrusted:true});assert.equal(presentations,1);assert.equal(env.sent.at(-1).kind,'login_open');
 env.node('login-password').value='temporary draft';env.node('login-remember').checked=true;
 ui.close({isTrusted:true,preventDefault(){}});
 assert.equal(env.node('login-password').value,'');assert.equal(env.node('login-remember').checked,false);
 ui.receive({state:'opened',account:'Alice',saved:true,message:'late'});
 assert.equal(env.node('login-dialog').open,false);assert.equal(env.focus.length,0);
 assert.ok(env.sent.every(packet=>packet.kind!=='login_submit'));
 ui.request({isTrusted:true});ui.receive({state:'opened',account:'Alice',saved:true,message:'ready'});
 assert.equal(env.node('login-account').value,'Alice');assert.equal(env.focus.length,0);ui.destroy();
});
test('embedded filters use the host without opening when the host is still loading',()=>{
 const env=environment('surface-filters.js','createFiltersDialog');let managed=false;
 const ui=env.create({manage:()=>managed,present:dialog=>{dialog.open=true;},visible:()=>false});
 ui.request({isTrusted:true});assert.equal(env.sent.length,0);assert.equal(env.node('filters-dialog').open,false);
 managed=true;ui.request({isTrusted:true});assert.equal(env.sent.at(-1).kind,'filters_open');
 ui.open({entries:[],allow_color:14});assert.equal(env.focus.length,0);
 ui.close({isTrusted:true,preventDefault(){}});ui.open({entries:[{id:'late',policy:'allow'}]});
 assert.equal(env.node('filters-dialog').open,false);ui.destroy();
});

test('leaving an embedded settings panel preserves search focus until the host closes',()=>{
 const source=readFileSync(new URL('../assets/surface.js',import.meta.url),'utf8');
 const start=source.indexOf('function restoreSettingsPanelFocus()'),end=source.indexOf('\n',start);
 const dialog={open:true};let restored=0;
 const restore=vm.runInNewContext(source.slice(start,end)+'\nrestoreSettingsPanelFocus',{document:{getElementById:()=>dialog},restoreEditingFocus:()=>restored++});
 restore();assert.equal(restored,0);dialog.open=false;restore();assert.equal(restored,1);
});

function filterFixture(entries,{allowColor=14}={}){
    const env=environment("surface-filters.js","createFiltersDialog");let language="zh-TW";
    for(const [value,key] of [[10,"filters-color-bright-green"],[13,"filters-color-bright-magenta"],[14,"filters-color-bright-cyan"]])
        env.node("filters-color").append({value:String(value),dataset:{colorKey:key}});
    const ui=env.create({tr:(key,args)=>translator(language)(key,args)});
    env.fire("filters-open");ui.open({entries,allow_color:allowColor});
    return {env,ui,ids:()=>env.node("filters-list").children.map(option=>option.value),language:value=>language=value};
}
test("account tabs contain only real IDs and sort within each category without editing stored rules",()=>{
    const entries=[{id:"Zulu",policy:"mask",note:"z note"},{id:"alpha",policy:"mask_row",note:"a note"},
        {id:"Zwhite",policy:"allow",color:10,note:"green"},{id:"Awhite",policy:"allow",note:"shared"},{id:"Bwhite",policy:"allow",color:13,note:"pink"}];
    const f=filterFixture(entries),{env,ui}=f,snapshot=JSON.stringify(entries),count=env.sent.length;
    assert.deepEqual(f.ids(),["alpha","Zulu"]);assert.equal(env.node("filters-list").selectedIndex,-1);
    assert.ok(env.node("filters-list").children[0].textContent.startsWith("alpha · 黑名單（整行塗黑）"));
    env.node("filters-list").value="alpha";env.fire("filters-list","change");env.node("filters-note").value="unsaved note";
    env.fire("filters-sort","click",false);assert.deepEqual(f.ids(),["alpha","Zulu"]);
    env.fire("filters-sort");assert.deepEqual(f.ids(),["Zulu","alpha"]);assert.equal(env.node("filters-list").selectedIndex,1);
    assert.equal(env.node("filters-note").value,"unsaved note");assert.equal(env.node("filters-sort").attrs["aria-pressed"],"true");
    env.fire("filters-tab-allow");assert.deepEqual(f.ids(),["Zwhite","Bwhite","Awhite"]);
    assert.equal(env.node("filters-id").value,"");assert.equal(env.node("filters-policy").value,"allow");
    assert.equal(env.node("filters-policy-field").hidden,true);assert.equal(env.node("filters-color-field").hidden,false);
    assert.ok(env.node("filters-list").children[0].textContent.startsWith("Zwhite · 亮綠"));
    env.fire("filters-sort");assert.deepEqual(f.ids(),["Awhite","Bwhite","Zwhite"]);
    assert.equal(env.node("filters-tab-allow").attrs["aria-selected"],"true");
    assert.equal(env.sent.length,count);assert.equal(JSON.stringify(entries),snapshot);
    env.fire("filters-tab-block","keydown",true,{key:"ArrowLeft"});assert.equal(env.focus.at(-1),"filters-tab-block");
    assert.deepEqual(f.ids(),["alpha","Zulu"]);ui.destroy();
});
test("new account is a separate action, tab choice constrains policy, and empty lists have no placeholder row",()=>{
    const f=filterFixture([{id:"Alice",policy:"mask",note:"saved"}]),{env,ui}=f;
    env.node("filters-list").value="Alice";env.fire("filters-list","change");const count=env.sent.length;
    env.fire("filters-new");assert.equal(env.node("filters-id").value,"");assert.equal(env.node("filters-note").value,"");
    assert.equal(env.focus.at(-1),"filters-id");assert.equal(env.sent.length,count);
    env.fire("filters-tab-allow");assert.deepEqual(f.ids(),[]);assert.equal(env.node("filters-empty").hidden,false);
    env.node("filters-id").value="Bob";env.node("filters-policy").value="mask_row";env.fire("filters-save");
    assert.equal(env.sent.at(-1).policy,"allow");assert.equal(env.sent.at(-1).id,"Bob");
    ui.receive({state:"saved",entries:[{id:"Alice",policy:"mask",note:"saved"},{id:"Bob",policy:"allow",note:""}]});
    assert.deepEqual(f.ids(),["Bob"]);env.fire("filters-tab-block");
    env.node("filters-id").value="Charlie";env.node("filters-policy").value="allow";env.fire("filters-save");
    assert.equal(env.sent.at(-1).policy,"mask");ui.destroy();
});
test("shared color changes regroup allowlist rows, and live translation preserves the selected draft",()=>{
    const entries=[{id:"Alice",policy:"allow",note:"saved"},{id:"Bob",policy:"allow",color:13,note:""}];
    const f=filterFixture(entries),{env,ui}=f;env.fire("filters-tab-allow");env.fire("filters-sort");assert.deepEqual(f.ids(),["Bob","Alice"]);
    env.node("filters-default-color").value="10";env.fire("filters-default-color-save");
    ui.receive({state:"saved",allow_color:10,entries});assert.deepEqual(f.ids(),["Alice","Bob"]);
    env.node("filters-list").value="Bob";env.fire("filters-list","change");env.node("filters-note").value="unsaved";
    const count=env.sent.length;f.language("en-US");ui.relocalize();
    assert.ok(env.node("filters-list").children[0].textContent.startsWith("Alice · Bright green"));
    assert.equal(env.node("filters-sort-label").textContent,"Sort: category");assert.equal(env.node("filters-note").value,"unsaved");
    assert.equal(env.node("filters-color").value,"13");assert.equal(env.node("filters-list").selectedIndex,1);assert.equal(env.sent.length,count);
    env.node("filters-default-color").value="14";env.fire("filters-default-color-save");ui.receive({state:"rejected",message:"failed"});
    assert.deepEqual(f.ids(),["Alice","Bob"]);assert.equal(env.node("filters-default-color").value,"10");ui.destroy();
});
test("imported marker colours are readable in preview and cancellation restores local colours",()=>{
    const local=[{id:"Alice",policy:"allow",note:"local",color:13}];
    const f=filterFixture(local),{env,ui}=f;
    env.fire("filters-import");const request=env.sent.at(-1).request;
    ui.receive({state:"preview",request,entries:[{id:"Alice",policy:"allow",note:"imported",color:10}],message:"preview"});
    env.fire("filters-tab-allow");
    assert.ok(env.node("filters-list").children[0].textContent.startsWith("Alice · 亮綠"));
    env.node("filters-list").value="Alice";env.fire("filters-list","change");
    assert.equal(env.node("filters-color").value,"10");
    assert.equal(env.sent.filter(packet=>packet.kind==="filters_import_apply").length,0);
    env.fire("filters-import-cancel");
    assert.ok(env.node("filters-list").children[0].textContent.startsWith("Alice · 亮紫"));
    env.fire("filters-import");const next=env.sent.at(-1).request;
    ui.receive({state:"preview",request:next,entries:[{id:"Alice",policy:"allow",note:"imported",color:10}],message:"preview"});
    env.fire("filters-import-apply");
    assert.equal(env.sent.at(-1).kind,"filters_import_apply");
    assert.equal(env.sent.at(-1).request,next);
    ui.receive({state:"saved",allow_color:14,entries:[{id:"Alice",policy:"allow",note:"imported",color:10}],message:"saved"});
    assert.ok(env.node("filters-list").children[0].textContent.startsWith("Alice · 亮綠"));ui.destroy();
});

test("import preview can be inspected across tabs and sort modes before cancellation restores saved entries",()=>{
    const f=filterFixture([{id:"Local",policy:"allow",note:""}]),{env,ui}=f;
    env.fire("filters-import");const request=env.sent.at(-1).request;
    ui.receive({state:"preview",request,entries:[{id:"Imported",policy:"mask",note:""},{id:"Local",policy:"allow",note:""}]});
    assert.deepEqual(f.ids(),["Imported"]);assert.equal(env.node("filters-new").disabled,true);
    const count=env.sent.length;env.fire("filters-tab-allow");env.fire("filters-sort");assert.deepEqual(f.ids(),["Local"]);
    env.fire("filters-new");assert.equal(env.sent.length,count);assert.equal(env.node("filters-save").disabled,true);
    env.fire("filters-import-cancel");env.fire("filters-tab-block");assert.deepEqual(f.ids(),[]);
    assert.equal(env.node("filters-empty").hidden,false);env.fire("filters-tab-allow");assert.deepEqual(f.ids(),["Local"]);
    assert.equal(env.sent.filter(packet=>packet.kind==="filters_import_apply"||packet.kind==="filters_edit").length,0);ui.destroy();
});

test('article boundary setting defaults off, saves, retries and resets with its own page',async()=>{
 let page='media';const env=environment('surface-settings.js','createSettingsDialog'),ui=env.create({page:()=>page});
 await ui.open(openData());assert.equal(env.node('article-boundary-navigation').checked,false);
 env.node('article-boundary-navigation').checked=true;env.fire('article-boundary-navigation','change',false);env.tick();
 assert.equal(env.sent.filter(p=>p.kind==='settings_save').length,0);
 env.fire('article-boundary-navigation','change');env.tick();assert.equal(env.sent.at(-1).article_boundary_navigation,true);
 ui.saved({status:'rejected'});assert.equal(ui.snapshot().article_boundary_navigation,true);
 env.fire('font-save');ui.saved({status:'saved'});
 page='appearance';env.fire('font-defaults');env.tick();assert.equal(env.sent.at(-1).article_boundary_navigation,true);ui.saved({status:'saved'});
 page='media';env.fire('font-defaults');env.tick();assert.equal(env.sent.at(-1).article_boundary_navigation,false);ui.saved({status:'saved'});
 ui.destroy();
});

test('settings Escape owns the gesture, waits for autosave and preserves composition',async()=>{
 const env=environment('surface-settings.js','createSettingsDialog'),ui=env.create();await ui.open(openData());
 const escape=ui.bindings.find(b=>b[0]===env.node('font-dialog'));
 const handler=ui.bindings.find(b=>b[1]==='keydown'&&b[3]===true)[2];
 let prevented=0,stopped=0;const event={key:'Escape',isTrusted:true,preventDefault(){prevented++;},stopImmediatePropagation(){stopped++;}};
 handler({...event,isComposing:true});handler({...event,isTrusted:false});assert.equal(prevented,0);
 env.node('font-size').value='18';env.fire('font-size','input');handler(event);
 assert.equal(prevented,1);assert.equal(stopped,1);assert.equal(env.node('font-dialog').open,true);
 ui.saved({status:'rejected'});assert.equal(env.node('font-dialog').open,true);
 handler(event);ui.saved({status:'saved'});assert.equal(env.node('font-dialog').open,false);
 ui.destroy();
});

test('native settings Escape does not discard an active IME composition',async()=>{
 const env=environment('surface-settings.js','createSettingsDialog'),ui=env.create();await ui.open(openData());
 env.fire('font-dialog','compositionstart');ui.escape();assert.equal(env.node('font-dialog').open,true);
 env.fire('font-dialog','compositionend');ui.escape();assert.equal(env.node('font-dialog').open,false);ui.destroy();
});

test("update checks and download actions require an open trusted settings dialog",async()=>{
    const env=environment("surface-settings.js","createSettingsDialog"),ui=env.create();
    env.fire("update-check");assert.equal(env.sent.length,0);
    await ui.open(openData());env.sent.length=0;
    env.fire("update-check","click",false);assert.equal(env.sent.length,0);
    env.fire("update-check");assert.equal(env.sent.at(-1).kind,"update_check");
    ui.updateStatus({checking:true,available:null,up_to_date:false});
    assert.equal(env.node("update-check").disabled,true);
    const before=env.sent.length;env.fire("update-check");assert.equal(env.sent.length,before);
    ui.updateStatus({checking:false,available:null,up_to_date:false});
    assert.equal(env.node("update-status").textContent,"");
    assert.equal(env.node("update-open").hidden,true);
    assert.equal(env.node("update-check").disabled,false);
    env.fire("update-open");assert.equal(env.sent.length,before);
    ui.updateStatus({checking:false,available:{version:"0.3.0",url:"https://github.com/yam276/PTTStella/releases/tag/v0.3.0"},up_to_date:false});
    assert.equal(env.node("update-status").textContent,"有新版本：0.3.0");
    assert.equal(env.node("update-open").hidden,false);
    env.fire("update-open","click",false);assert.equal(env.sent.length,before);
    env.fire("update-open");assert.equal(JSON.stringify(env.sent.at(-1)),JSON.stringify({kind:"update_open",trusted:true}));
    ui.relocalize();assert.equal(env.node("update-status").textContent,"有新版本：0.3.0");
    ui.updateStatus({checking:false,available:null,up_to_date:true});
    assert.match(env.node("update-status").textContent,/最新正式版本/);
    ui.destroy();const message=env.node("update-status").textContent;
    ui.updateStatus({checking:true,available:null,up_to_date:false});
    assert.equal(env.node("update-status").textContent,message);
    const final=env.sent.length;env.fire("update-check");env.fire("update-open");assert.equal(env.sent.length,final);
});
