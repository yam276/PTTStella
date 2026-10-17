// One font policy for preview, screen, IME and PNG. Face checks do not prove glyph coverage.
function createSurfaceFonts({document,FontFace,tr}) {
    const bundledFonts=new Map([
        ["Cascadia Mono",{family:"PTTStella Cascadia Mono",file:"CascadiaMono.ttf"}],
        ["Noto Sans CJK TC",{family:"PTTStella Noto Sans CJK TC",file:"NotoSansCJKtc-Regular.otf"}]
    ]);
    const bundledLoads=new Map();
    // Preserve the chosen CJK face first. Missing faces/glyphs then use the
    // selected Latin face for symbols before the operating system's generic face.
    const fontFamily=(name,fallback)=>[...new Set([name,fallback].filter(Boolean))]
        .map(value=>JSON.stringify(bundledFonts.get(value)?.family??value)).join(", ")+", monospace";
    const fontDescription=(settings,cjk=false,scale=1)=>settings.font_size*scale+"px "+
        (cjk?fontFamily(settings.cjk_font,settings.ascii_font):fontFamily(settings.ascii_font));
    let loadedFonts=null;
    async function loadFonts(settings,{refresh=false}={}) {
        const key=JSON.stringify([settings.ascii_font,settings.cjk_font,settings.font_size]);
        if(!refresh&&loadedFonts?.key===key&&!loadedFonts.warning) return "";
        const missing=await Promise.all([[tr("font-role-ascii"),settings.ascii_font],[tr("font-role-cjk"),settings.cjk_font]].map(async([role,name])=>{
            const bundled=bundledFonts.get(name);
            try {
                if(bundled) {
                    if(!bundledLoads.has(name)) {
                        const url=new URL("/pttstella-fonts/"+bundled.file,document.baseURI);
                        bundledLoads.set(name,new FontFace(bundled.family,"url("+JSON.stringify(url.href)+")").load()
                            .then(face=>{document.fonts.add(face);return face;})
                            .catch(error=>{bundledLoads.delete(name);throw error;}));
                    }
                    await bundledLoads.get(name);
                } else {
                    // fonts.load/check can succeed via fallback; local() verifies
                    // the selected system face independently of bundled aliases.
                    await new FontFace("PTTStella font check","local("+JSON.stringify(name)+")").load();
                }
                return "";
            } catch {return tr(bundled?"font-face-bundled-failed":"font-face-local-failed",{role,name});}
        }));
        await Promise.all([document.fonts.load(fontDescription(settings),"M"),document.fonts.load(fontDescription(settings,true),"中")]);
        await document.fonts.ready;
        const warning=missing.filter(Boolean).join("；");
        loadedFonts={key,warning:warning?tr("font-fallback-warning",{warning}):""};
        return loadedFonts.warning;
    }
    function fontLoadNote(settings,warning) {
        if(warning)return warning;
        const names=[...new Set([settings.ascii_font,settings.cjk_font])].filter(name=>bundledFonts.has(name));
        return names.length?tr("font-bundled-loaded",{names:names.join(", ")}):tr("font-local-loaded");
    }
    return {family:fontFamily,description:fontDescription,load:loadFonts,note:fontLoadNote,invalidate:()=>{loadedFonts=null;}};
}
