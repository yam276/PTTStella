// Draft fields, preview requests and timers belong to this dialog only.
function createSettingsDialog({send,beforeRequest,beforeShow,restore,changed,loadFonts,fontLoadNote,render}) {
    const fontButton=document.getElementById("font-settings"),fontDialog=document.getElementById("font-dialog");
    let disposed=false,epoch=0,fontDraft=null,fontTimer=0,fontRequest=0,fontPreviewBusy=false,fontPreviewNext=null,fontSaving=false;
    const fontPreview=document.getElementById("font-preview"),fontNote=document.getElementById("font-note");
    const fontAvailability=document.getElementById("font-availability");
    const fontPreset=document.getElementById("font-preset"),fontSave=document.getElementById("font-save"),fontCancel=document.getElementById("font-cancel");
    const fontDefaults=document.getElementById("font-defaults");
    const profileImport=document.getElementById("profile-import"),profileExport=document.getElementById("profile-export"),profileName=document.getElementById("font-profile-name");
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
        clearTimeout(fontTimer);fontRequest++;fontNote.textContent="";
        fontTimer=setTimeout(()=>{fontTimer=0;fontPreviewNext={request:fontRequest,values:readFonts()};flushFontPreview();},150);
    }
    function editFont(e){if(e.target===fontFields.ascii_font||e.target===fontFields.cjk_font)fontPreset.value="custom";previewFonts();}
    function chooseFontPreset(){const values=fontPreset.value==="platform"?fontDraft?.defaults:fontDraft?.presets[fontPreset.value];
        if(values) {const size=fontFields.font_size.value,baseline=fontFields.baseline.value;fillFonts(values);fontFields.font_size.value=size;fontFields.baseline.value=baseline;previewFonts();}}
    function resetFonts(){if(fontDraft){fillFonts(fontDraft.defaults);fontPreset.value="platform";previewFonts();}}
    function closeFonts(e) {
        e?.preventDefault();if(fontSaving||profileBusy) return;
        clearTimeout(fontTimer);fontTimer=0;fontRequest++;fontPreviewNext=null;
        fontDialog.close();epoch++;restore();
        changed();
    }
    function saveFonts(e) {
        if(fontSaving||profileBusy) return;
        fontSaving=true;fontSave.disabled=true;fontCancel.disabled=true;
        send({kind:"settings_save",values:readFonts(),trusted:e.isTrusted});
    }

    async function preview(settings,packet,valid){
        const warning=await loadFonts(settings);
        if(disposed||!valid())return;
        fontAvailability.textContent=fontLoadNote(settings,warning);
        render(fontPreview,packet,{cursor:false});
    }
    async function open(data){
        if(disposed)return;
        const opening=++epoch;
        fontDraft=data;fillFonts(data.values);fontPreset.value=profileChoice(data.profile);fontNote.textContent=data.warning||"";
        beforeShow();if(!fontDialog.open)fontDialog.showModal();changed();
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
    function revoke(){disposed=true;epoch++;clearTimeout(fontTimer);fontTimer=0;fontPreviewNext=null;}
    function destroy(){revoke();fontDraft=null;fontPreviewBusy=false;profileBusy=false;fontSaving=false;if(fontDialog.open)fontDialog.close();}
    return {open,preview:receivePreview,profile,saved,revoke,destroy,
        notice(message){if(!fontDialog.open)return false;fontNote.textContent=message;return true;},
        clean:()=>disposed&&fontTimer===0&&fontPreviewNext===null&&!fontDialog.open,
        bindings:[
            [fontButton,"click",openFonts],[fontPreset,"change",chooseFontPreset],
            [profileImport,"click",importProfile],[profileExport,"click",exportProfile],
            ...Object.values(fontFields).map(field=>[field,"input",editFont]),
            [fontDefaults,"click",resetFonts],[fontCancel,"click",closeFonts],[fontDialog,"cancel",closeFonts],[fontSave,"click",saveFonts],
        ]};
}
