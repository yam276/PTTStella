// One bounded autosave: coalesce edits, retain failures, never overwrite newer input.
function createSettingsDialog({send,beforeRequest,beforeShow,restore,changed,tr,loadFonts,fontLoadNote,invalidateFonts,render,onClose=()=>{},page=()=>"appearance"}) {
    const fontButton=document.getElementById("font-settings"),fontDialog=document.getElementById("font-dialog");
    let disposed=false,epoch=0,fontDraft=null,fontTimer=0,fontRequest=0,fontPreviewBusy=false,fontPreviewNext=null,fontSaving=false;
    const fontPreview=document.getElementById("font-preview"),fontNote=document.getElementById("font-note");
    const fontAvailability=document.getElementById("font-availability");
    const fontPreset=document.getElementById("font-preset"),fontSave=document.getElementById("font-save"),fontCancel=document.getElementById("font-cancel");
    const fontDefaults=document.getElementById("font-defaults");
    const profileImport=document.getElementById("profile-import"),profileExport=document.getElementById("profile-export"),profileName=document.getElementById("font-profile-name");
    const hoverPosition=document.getElementById("settings-hover");
    const articleMode=document.getElementById("article-mode");
    const articleBoundary=document.getElementById("article-boundary-navigation");
    const language=document.getElementById("interface-language");
    const toolsOpacity=document.getElementById("tools-opacity"),opacityValue=document.getElementById("tools-opacity-value");
    const rightClickMenu=document.getElementById("tools-right-click"),showFloatingButton=document.getElementById("tools-show-ball");
    function updateOpacity(){opacityValue.textContent=tr("settings-opacity-value",{percent:Number(toolsOpacity.value)});}
    const localChoices={ascii_font:document.getElementById("font-local-ascii"),cjk_font:document.getElementById("font-local-cjk")};
    const localStatus=document.getElementById("font-local-status"),fontRefresh=document.getElementById("font-refresh");
    const sourceButtons=[...document.querySelectorAll("[data-font-source]")];
    const sources=document.getElementById("font-sources");
    let hoverTouched=false,hoverVersion=0,savedHoverVersion=0;
    let lastCatalog=null,lastPreview=null;
    let catalogRequest=0,saveTimer=0,dirty=false,closing=false,composing=false,pendingSave=null;
    function state(message){fontNote.textContent=message;fontSave.hidden=!dirty||fontSaving;fontSave.disabled=fontSaving;}
    function scheduleSave(e){
        if(disposed||!fontDialog.open||!e?.isTrusted)return;
        dirty=true;closing=false;fontDialog.dataset.saveState="pending";state(tr("settings-saving"));clearTimeout(saveTimer);
        if(!composing)saveTimer=setTimeout(()=>{saveTimer=0;saveFonts(e);},300);
    }
    function validFields(){
        return Object.values(fontFields).every(field=>!field.checkValidity||field.checkValidity())&&
            [fontFields.ascii_font,fontFields.cjk_font].every(field=>field.value.trim().length>0);
    }
    function syncChoices(){
        for(const [name,field] of Object.entries(localChoices)){
            const selected=fontFields[name].value;
            field.value=[...field.children].some(option=>option.value===selected)?selected:"";
        }
        const names=[fontFields.ascii_font.value,fontFields.cjk_font.value];
        for(const button of sourceButtons)button.hidden=!button.dataset.fontFamilies.split("\n").some(name=>names.includes(name));
    }
    function requestCatalog(){
        catalogRequest++;fontRefresh.disabled=true;localStatus.textContent=tr("font-local-loading");
        for(const field of Object.values(localChoices))field.disabled=true;
        send({kind:"settings_fonts",request:catalogRequest});
    }
    function refreshFonts(e){if(!e.isTrusted||disposed||!fontDialog.open)return;invalidateFonts();requestCatalog();previewFonts();}
    function chooseLocal(e){
        if(!e.isTrusted||disposed||!fontDialog.open||!e.target.value)return;
        const name=Object.keys(localChoices).find(name=>localChoices[name]===e.target);
        if(!name)return;
        fontFields[name].value=e.target.value;fontPreset.value="custom";syncChoices();previewFonts();scheduleSave(e);
    }
    function catalog(data){
        if(disposed||!fontDialog.open||data.request!==catalogRequest)return;
        lastCatalog=data;
        fontRefresh.disabled=false;
        for(const field of Object.values(localChoices)){
            const placeholder=document.createElement("option");placeholder.value="";placeholder.textContent=tr("font-local-choose");
            field.replaceChildren(placeholder);
            for(const name of data.families){const option=document.createElement("option");option.value=name;option.textContent=name;field.append(option);}
            field.disabled=data.families.length===0;
        }
        syncChoices();
        localStatus.textContent=data.notice??tr("font-local-count",{count:data.families.length});
    }
    function openSource(button,e){
        if(disposed||!fontDialog.open||button.hidden||!e.isTrusted)return;
        send({kind:"font_source",source:button.dataset.fontSource,trusted:e.isTrusted});
    }
    let profileBusy=false;
    function profileAction(action,e){if(!e.isTrusted||disposed||profileBusy||fontSaving)return;if(dirty){saveFonts(e);return;}profileBusy=true;profileImport.disabled=true;profileExport.disabled=true;
        send({kind:"settings_profile",action,name:profileName.value,trusted:e.isTrusted});}
    const importProfile=e=>profileAction("import",e),exportProfile=e=>profileAction("export",e);
    function profileChoice(profile){return profile.fonts.kind==="platform_default"?"platform":profile.fonts.kind==="preset"?profile.fonts.preset:"custom";}
    const fontFields={ascii_font:document.getElementById("font-ascii"),cjk_font:document.getElementById("font-cjk"),
        font_size:document.getElementById("font-size"),baseline:document.getElementById("font-baseline")};
    function openFonts(e){beforeRequest(e);send({kind:"settings_open",trusted:e.isTrusted});}
    function fillFonts(values) {
        for(const [name,field] of Object.entries(fontFields)) field.value=values[name];
        syncChoices();
    }
    function readFonts() {
        const selected=fontPreset.value,fonts=selected==="custom"?{kind:"custom",ascii_font:fontFields.ascii_font.value,cjk_font:fontFields.cjk_font.value}:
            selected==="platform"?{kind:"platform_default"}:{kind:"preset",preset:selected};
        return {fonts,font_size:Number(fontFields.font_size.value),baseline:Number(fontFields.baseline.value)};
    }
    function flushFontPreview() {
        if(fontPreviewBusy||!fontPreviewNext||!fontDialog.open||disposed) return;
        fontPreviewBusy=true;send({kind:"settings_preview",...fontPreviewNext});fontPreviewNext=null;
    }
    function previewFonts() {
        clearTimeout(fontTimer);fontRequest++;fontNote.textContent="";syncChoices();
        fontTimer=setTimeout(()=>{fontTimer=0;fontPreviewNext={request:fontRequest,values:readFonts()};flushFontPreview();},150);
    }
    function editFont(e){if(!e.isTrusted||disposed)return;if(e.target===fontFields.ascii_font||e.target===fontFields.cjk_font)fontPreset.value="custom";previewFonts();scheduleSave(e);}
    function chooseFontPreset(e){if(!e.isTrusted||disposed)return;const values=fontPreset.value==="platform"?fontDraft?.defaults:fontDraft?.presets[fontPreset.value];
        if(values) {const size=fontFields.font_size.value,baseline=fontFields.baseline.value;fillFonts(values);fontFields.font_size.value=size;fontFields.baseline.value=baseline;previewFonts();scheduleSave(e);}}
    function resetFonts(e){
        if(!e.isTrusted||!fontDraft||disposed)return;
        if(page()==="general")language.value="zh-TW";
        else if(page()==="input"){toolsOpacity.value="100";rightClickMenu.checked=true;showFloatingButton.checked=true;updateOpacity();}
        else if(page()==="media"){articleMode.value="web";articleBoundary.checked=false;hoverPosition.value="top_right";hoverTouched=true;hoverVersion++;}
        else if(page()==="appearance"){fillFonts(fontDraft.defaults);fontPreset.value="platform";previewFonts();}
        else return;
        scheduleSave(e);
    }
    function finishClose(){
        clearTimeout(fontTimer);fontTimer=0;fontRequest++;fontPreviewNext=null;
        fontDialog.close();epoch++;catalogRequest++;fontRefresh.disabled=false;restore();changed();onClose();
    }
    function escapeSettings(e) {
        if(disposed||!fontDialog.open||!e.isTrusted||e.key!=="Escape"||e.isComposing||e.keyCode===229||composing)return;
        e.preventDefault();e.stopImmediatePropagation();
        if(!e.repeat)closeFonts();
    }
    function repository(e){if(!disposed&&fontDialog.open&&e.isTrusted)send({kind:"about_repository",trusted:true});}
    function closeFonts(e) {
        e?.preventDefault?.();if(disposed||profileBusy)return;
        if(dirty||fontSaving){closing=true;clearTimeout(saveTimer);saveTimer=0;if(!fontSaving)saveFonts({isTrusted:true});return;}
        finishClose();
    }
    function saveFonts(e) {
        if(!e?.isTrusted||disposed||!fontDialog.open||fontSaving||profileBusy||composing)return;
        clearTimeout(saveTimer);saveTimer=0;
        if(!validFields()){fontDialog.dataset.saveState="invalid";dirty=true;closing=false;state(tr("settings-autosave-invalid"));return;}
        pendingSave={kind:"settings_save",values:readFonts(),language:language.value,article_mode:articleMode.value,article_boundary_navigation:articleBoundary.checked,hover:hoverTouched?hoverPosition.value:null,tools_opacity:Number(toolsOpacity.value),
            tools_access:{right_click_menu:rightClickMenu.checked,show_floating_button:showFloatingButton.checked},trusted:true};
        dirty=false;fontSaving=true;fontDialog.dataset.saveState="writing";savedHoverVersion=hoverVersion;state(tr("settings-saving"));send(pendingSave);
    }

    async function preview(settings,packet,valid){
        const warning=await loadFonts(settings);
        if(disposed||!valid())return;
        lastPreview={settings,warning};
        fontAvailability.textContent=fontLoadNote(settings,warning);
        if(warning)sources.open=true;
        render(fontPreview,packet,{cursor:false});
    }
    async function open(data){
        if(disposed)return;
        if(fontDialog.open)return;
        const opening=++epoch;
        dirty=false;closing=false;fontSaving=false;pendingSave=null;hoverTouched=false;fontDialog.dataset.saveState="saved";fontSave.hidden=true;
        fontDraft=data;articleMode.value=data.article_mode??"web";articleBoundary.checked=data.article_boundary_navigation??false;hoverPosition.value=data.hover??"top_right";language.value=data.language;toolsOpacity.value=String(data.tools_opacity);updateOpacity();fillFonts(data.values);fontPreset.value=profileChoice(data.profile);fontNote.textContent=data.warning||"";
        rightClickMenu.checked=data.tools_access.right_click_menu;showFloatingButton.checked=data.tools_access.show_floating_button;
        beforeShow();if(!fontDialog.open)fontDialog.showModal();changed();requestCatalog();
        await preview(data.values,data.sample,()=>fontDialog.open&&opening===epoch);
    }
    async function receivePreview(data){
        if(disposed)return;
        fontPreviewBusy=false;
        const request=data.request,opening=epoch;
        if(fontDialog.open&&request===fontRequest){
            if(data.packet){
                await preview(data.packet.settings,data.packet,()=>fontDialog.open&&request===fontRequest&&opening===epoch);
                if(!disposed&&fontDialog.open&&request===fontRequest&&opening===epoch){
                    fontPreview.dataset.request=String(request);fontPreview.dataset.fontSize=String(data.packet.settings.font_size);fontPreview.dataset.baseline=String(data.packet.settings.baseline);
                }
            }
            if(data.error)fontNote.textContent=data.error;
        }
        flushFontPreview();
    }
    async function profile(data){
        if(disposed)return;
        profileBusy=false;profileImport.disabled=false;profileExport.disabled=false;fontDialog.dataset.profileResult=data.status;
        const opening=epoch;
        if(data.status==="imported"&&fontDialog.open){
            const imported=data.profile;clearTimeout(fontTimer);fontTimer=0;fontRequest++;fontPreviewNext=null;
            fillFonts(imported.values);fontPreset.value=profileChoice(imported.profile);profileName.value=imported.name;
            scheduleSave({isTrusted:true});
            await preview(imported.values,imported.sample,()=>fontDialog.open&&opening===epoch);
        }
        if(!disposed&&fontDialog.open&&opening===epoch){if(data.notice)fontNote.textContent=data.notice;if(dirty&&data.status!=="imported")saveFonts({isTrusted:true});profileName.focus();}
    }
    function saved(data){
        if(disposed||!fontSaving)return;
        fontSaving=false;fontDialog.dataset.saveState=data.status;
        if(data.status!=="saved"){dirty=true;closing=false;state(data.notice||tr("settings-save-failed"));return;}
        pendingSave=null;if(hoverVersion===savedHoverVersion)hoverTouched=false;
        if(dirty){saveFonts({isTrusted:true});return;}
        state(data.notice||tr("settings-saved"));
        if(closing)finishClose();
    }
    function revoke(){disposed=true;clearTimeout(saveTimer);saveTimer=0;epoch++;catalogRequest++;clearTimeout(fontTimer);fontTimer=0;fontPreviewNext=null;}
    function destroy(){revoke();fontDraft=null;lastCatalog=null;lastPreview=null;fontPreviewBusy=false;profileBusy=false;fontSaving=false;if(fontDialog.open)fontDialog.close();}
    return {request:openFonts,close:closeFonts,escape(){if(!composing)closeFonts();},
        snapshot(){if(!dirty&&!fontSaving)return null;if(!validFields())throw new Error(tr("settings-autosave-invalid"));return {kind:"settings_save",values:readFonts(),language:language.value,article_mode:articleMode.value,article_boundary_navigation:articleBoundary.checked,hover:hoverTouched?hoverPosition.value:null,tools_opacity:Number(toolsOpacity.value),tools_access:{right_click_menu:rightClickMenu.checked,show_floating_button:showFloatingButton.checked},trusted:true};},
        open,preview:receivePreview,catalog,profile,saved,revoke,destroy,
        relocalize(){updateOpacity();if(fontDialog.open){if(lastCatalog)catalog(lastCatalog);if(lastPreview)fontAvailability.textContent=fontLoadNote(lastPreview.settings,lastPreview.warning);}},
        notice(message){if(!fontDialog.open)return false;fontNote.textContent=message;return true;},
        clean:()=>disposed&&fontTimer===0&&saveTimer===0&&fontPreviewNext===null&&!fontDialog.open,
        bindings:[
            [document,"keydown",escapeSettings,true],
            [fontDialog,"compositionstart",()=>{composing=true;clearTimeout(saveTimer);saveTimer=0;}],
            [fontDialog,"compositionend",e=>{composing=false;if(dirty)scheduleSave(e);}],
            [document.getElementById("about-repository"),"click",repository],
            [toolsOpacity,"input",e=>{updateOpacity();scheduleSave(e);}],
            [articleMode,"change",scheduleSave],[articleBoundary,"change",scheduleSave],[language,"change",scheduleSave],[hoverPosition,"change",e=>{if(e.isTrusted){hoverTouched=true;hoverVersion++;}scheduleSave(e);}],[rightClickMenu,"change",scheduleSave],[showFloatingButton,"change",scheduleSave],
            ...Object.values(fontFields).flatMap(field=>[[field,"compositionstart",()=>{composing=true;clearTimeout(saveTimer);saveTimer=0;}],[field,"compositionend",e=>{composing=false;scheduleSave(e);}]]),
            ...(fontButton?[[fontButton,"click",openFonts]]:[]),[fontPreset,"change",chooseFontPreset],
            [fontRefresh,"click",refreshFonts],...Object.values(localChoices).map(field=>[field,"change",chooseLocal]),
            ...sourceButtons.map(button=>[button,"click",e=>openSource(button,e)]),
            [profileImport,"click",importProfile],[profileExport,"click",exportProfile],
            ...Object.values(fontFields).map(field=>[field,"input",editFont]),
            [fontDefaults,"click",resetFonts],[fontCancel,"click",closeFonts],[fontDialog,"cancel",closeFonts],[fontSave,"click",saveFonts],
        ]};
}
