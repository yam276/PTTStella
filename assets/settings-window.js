const locale=await dioxus.recv();const tr=createSurfaceI18n(locale,fluent);
const fonts=createSurfaceFonts({document,FontFace,tr});const fontDescription=fonts.description;
let stopped=false;
const send=value=>{if(!stopped)dioxus.send(value);};
const ui=createSettingsDialog({send,tr,beforeRequest(){},beforeShow(){},restore(){},changed(){},
    loadFonts:fonts.load,fontLoadNote:fonts.note,invalidateFonts:fonts.invalidate,render,
    page:()=>navigation.current(),onClose:()=>send({kind:'settings_hide'})});
const navigation=createSettingsNavigation({document,tr,launch:(feature,e)=>send({kind:'settings_launch',feature,trusted:e.isTrusted})});
const close=()=>{ui.close({preventDefault(){}});if(document.getElementById("font-dialog").open)send({kind:"settings_attention"});};
const context=e=>e.preventDefault();
const bindings=[...ui.bindings,...navigation.bindings,[window,'pttstella:settings-close',close],[document,'contextmenu',context]];
for(const [node,type,handler]of bindings)node.addEventListener(type,handler);
let snapshot={last:null,error:null};
try {
    while(!stopped){
        const packet=await dioxus.recv();
        if(packet.kind==='stop'){try{snapshot.last=ui.snapshot();}catch(error){snapshot.error=String(error);}stopped=true;break;}
        const data=packet.payload;
        if(packet.kind==='settings_close')close();
        else if(packet.kind==='settings_open')await ui.open(data);
        else if(packet.kind==='font_catalog')ui.catalog(data);
        else if(packet.kind==='font_preview')await ui.preview(data);
        else if(packet.kind==='settings_saved')ui.saved(data);
        else if(packet.kind==='profile_result')await ui.profile(data);
        else if(packet.kind==='notice')ui.notice(data);
        else throw new Error('Unknown settings packet: '+packet.kind);
    }
} finally {
    stopped=true;for(const [node,type,handler]of bindings)node.removeEventListener(type,handler);navigation.destroy();ui.destroy();
}
return snapshot;
