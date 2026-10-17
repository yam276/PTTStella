// Draft fields, preview requests and timers belong to this dialog only.
function createSettingsDialog({send,beforeRequest,beforeShow,restore,changed,tr,loadFonts,fontLoadNote,invalidateFonts,render}) {
    const fontButton=document.getElementById("font-settings"),fontDialog=document.getElementById("font-dialog");
    let disposed=false,epoch=0,fontDraft=null,fontTimer=0,fontRequest=0,fontPreviewBusy=false,fontPreviewNext=null,fontSaving=false;
    const fontPreview=document.getElementById("font-preview"),fontNote=document.getElementById("font-note");
    const fontAvailability=document.getElementById("font-availability");
    const fontPreset=document.getElementById("font-preset"),fontSave=document.getElementById("font-save"),fontCancel=document.getElementById("font-cancel");
    const fontDefaults=document.getElementById("font-defaults");
    const profileImport=document.getElementById("profile-import"),profileExport=document.getElementById("profile-export"),profileName=document.getElementById("font-profile-name");
    const language=document.getElementById("interface-language");
    const localChoices={ascii_font:document.getElementById("font-local-ascii"),cjk_font:document.getElementById("font-local-cjk")};
    const localStatus=document.getElementById("font-local-status"),fontRefresh=document.getElementById("font-refresh");
    const sourceButtons=[...document.querySelectorAll("[data-font-source]")];
    const sources=document.getElementById("font-sources");
    let catalogRequest=0;
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
        fontFields[name].value=e.target.value;fontPreset.value="custom";syncChoices();previewFonts();
    }
    function catalog(data){
        if(disposed||!fontDialog.open||data.request!==catalogRequest)return;
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
    function profileAction(action,e){if(profileBusy||fontSaving)return;profileBusy=true;profileImport.disabled=true;profileExport.disabled=true;
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
    function editFont(e){if(e.target===fontFields.ascii_font||e.target===fontFields.cjk_font)fontPreset.value="custom";previewFonts();}
    function chooseFontPreset(){const values=fontPreset.value==="platform"?fontDraft?.defaults:fontDraft?.presets[fontPreset.value];
        if(values) {const size=fontFields.font_size.value,baseline=fontFields.baseline.value;fillFonts(values);fontFields.font_size.value=size;fontFields.baseline.value=baseline;previewFonts();}}
    function resetFonts(){if(fontDraft){fillFonts(fontDraft.defaults);fontPreset.value="platform";previewFonts();}}
    function closeFonts(e) {
        e?.preventDefault();if(fontSaving||profileBusy) return;
        clearTimeout(fontTimer);fontTimer=0;fontRequest++;fontPreviewNext=null;
        fontDialog.close();epoch++;catalogRequest++;fontRefresh.disabled=false;restore();
        changed();
    }
    function saveFonts(e) {
        if(fontSaving||profileBusy) return;
        fontSaving=true;fontSave.disabled=true;fontCancel.disabled=true;
        send({kind:"settings_save",values:readFonts(),language:language.value,trusted:e.isTrusted});
    }

    async function preview(settings,packet,valid){
        const warning=await loadFonts(settings);
        if(disposed||!valid())return;
        fontAvailability.textContent=fontLoadNote(settings,warning);
        if(warning)sources.open=true;
        render(fontPreview,packet,{cursor:false});
    }
    async function open(data){
        if(disposed)return;
        const opening=++epoch;
        fontDraft=data;language.value=data.language;fillFonts(data.values);fontPreset.value=profileChoice(data.profile);fontNote.textContent=data.warning||"";
        beforeShow();if(!fontDialog.open)fontDialog.showModal();changed();requestCatalog();
        await preview(data.values,data.sample,()=>fontDialog.open&&opening===epoch);
        if(!disposed&&fontDialog.open&&opening===epoch)fontFields.ascii_font.focus();
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
            await preview(imported.values,imported.sample,()=>fontDialog.open&&opening===epoch);
        }
        if(!disposed&&fontDialog.open&&opening===epoch){if(data.notice)fontNote.textContent=data.notice;profileName.focus();}
    }
    function saved(data){
        if(disposed)return;
        fontSaving=false;fontSave.disabled=false;fontCancel.disabled=false;
        if(data.status==="saved")closeFonts();
    }
    function revoke(){disposed=true;epoch++;catalogRequest++;clearTimeout(fontTimer);fontTimer=0;fontPreviewNext=null;}
    function destroy(){revoke();fontDraft=null;fontPreviewBusy=false;profileBusy=false;fontSaving=false;if(fontDialog.open)fontDialog.close();}
    return {open,preview:receivePreview,catalog,profile,saved,revoke,destroy,
        notice(message){if(!fontDialog.open)return false;fontNote.textContent=message;return true;},
        clean:()=>disposed&&fontTimer===0&&fontPreviewNext===null&&!fontDialog.open,
        bindings:[
            [fontButton,"click",openFonts],[fontPreset,"change",chooseFontPreset],
            [fontRefresh,"click",refreshFonts],...Object.values(localChoices).map(field=>[field,"change",chooseLocal]),
            ...sourceButtons.map(button=>[button,"click",e=>openSource(button,e)]),
            [profileImport,"click",importProfile],[profileExport,"click",exportProfile],
            ...Object.values(fontFields).map(field=>[field,"input",editFont]),
            [fontDefaults,"click",resetFonts],[fontCancel,"click",closeFonts],[fontDialog,"cancel",closeFonts],[fontSave,"click",saveFonts],
        ]};
}
