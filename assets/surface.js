const canvas=document.getElementById("screen"), keyboard=document.getElementById("keyboard");
const surface=document.getElementById("surface"), viewport=document.querySelector(".workspace");
const chrome=document.getElementById("chrome"), toolbar=chrome.querySelector("header");
const minus=document.getElementById("zoom-out"), plus=document.getElementById("zoom-in");
const fit=document.getElementById("zoom-fit"), level=document.getElementById("zoom-level");
const selectionLayer=document.getElementById("selection"), selectAll=document.getElementById("select-all");
const surfaceNote=document.getElementById("surface-note");
const pasteDialog=document.getElementById("paste-dialog"), pasteSend=document.getElementById("paste-send"), pasteCancel=document.getElementById("paste-cancel");
const exportButton=document.getElementById("export-png");
const fontButton=document.getElementById("font-settings"),fontDialog=document.getElementById("font-dialog");
const fontPreview=document.getElementById("font-preview"),fontNote=document.getElementById("font-note");
const fontPreset=document.getElementById("font-preset"),fontSave=document.getElementById("font-save"),fontCancel=document.getElementById("font-cancel");
const fontDefaults=document.getElementById("font-defaults");
const fontFields={ascii_font:document.getElementById("font-ascii"),cjk_font:document.getElementById("font-cjk"),
    font_size:document.getElementById("font-size"),baseline:document.getElementById("font-baseline")};
const ctx=canvas.getContext("2d",{willReadFrequently:true});
const palette=["#000000","#800000","#008000","#808000","#000080","#800080","#008080","#c0c0c0",
    "#808080","#ff0000","#00ff00","#ffff00","#0000ff","#ff00ff","#00ffff","#ffffff"];
const fontFamily=name=>JSON.stringify(name)+", monospace";
const fontDescription=(settings,cjk=false)=>settings.font_size+"px "+fontFamily(cjk?settings.cjk_font:settings.ascii_font);
let loadedFonts=null,fontDraft=null,fontTimer=0,fontRequest=0,fontPreviewBusy=false,fontPreviewNext=null,fontSaving=false;
async function loadFonts(settings) {
    const key=JSON.stringify([settings.ascii_font,settings.cjk_font,settings.font_size]);
    if(loadedFonts===key) return;
    await Promise.all([document.fonts.load(fontDescription(settings),"M"),document.fonts.load(fontDescription(settings,true),"中")]);
    await document.fonts.ready;loadedFonts=key;
}
let generation=0, revision=-1, composing=false, composition=null, beforeEdit=null, sent=0;
let latest=null, animation=0, stopped=false, draws=0, zoom=1;
let grid=null, dragging=null, selectionNext=null, selectionBusy=null, selectionCache=null, selectionSequence=0;
let selectionActive=false, selectionFingerprint=null;
let pastePending=false;
let hoveredLink=-1;
let exportBusy=false, exportTask=null;
const send=value=>{ sent++; dioxus.send(value); };
const keys={Enter:[13],Backspace:[8],Tab:[9],Escape:[27],ArrowUp:[27,91,65],ArrowDown:[27,91,66],
    ArrowRight:[27,91,67],ArrowLeft:[27,91,68],Home:[27,91,72],End:[27,91,70],
    PageUp:[27,91,53,126],PageDown:[27,91,54,126],Delete:[27,91,51,126]};
function keydown(e) {
    if(pastePending) {e.preventDefault();return;}
    if(e.isComposing||composing||e.key==="Process"||e.metaKey) return;
    // Let the native copy action fire; copy data is already prepared by Rust.
    if(e.ctrlKey&&e.key.toLowerCase()==="c"&&hasSelection()) return;
    if(e.ctrlKey&&e.key.toLowerCase()==="v") return;
    if(e.key==="Escape"&&hasSelection()) { e.preventDefault(); cancelDrag();clearSelection(); return; }
    if(e.ctrlKey&&["+","=","-","0"].includes(e.key)) {
        e.preventDefault(); zoomTo(e.key==="0"?1:zoom+(e.key==="-"?-.25:.25)); return;
    }
    let bytes=keys[e.key];
    if(e.ctrlKey&&/^[a-z]$/i.test(e.key)) bytes=[e.key.toUpperCase().charCodeAt(0)&31];
    if(bytes) { clearSelection(); e.preventDefault(); send({kind:"key",bytes,generation,trusted:e.isTrusted}); }
}
function input(e) {
    if(pastePending) {keyboard.value="";return;}
    if(composing||e.isComposing) {
        if(composition) {composition.editTrusted=e.isTrusted;composition.editText=keyboard.value;}
        return;
    }
    const text=e.data||keyboard.value; keyboard.value="";
    // A trailing input for an already committed composition belongs to that edit,
    // while a new beforeinput identifies a new edit even when its text is equal.
    if(beforeEdit?.composition) {beforeEdit=null;return;}
    if(text) { clearSelection(); send({kind:"text",text,generation,trusted:e.isTrusted}); }
}
function beforeinput(e) {
    beforeEdit={composition:composing||e.isComposing||/Composition/.test(e.inputType||"")};
}
function compositionstart(e) {
    clearSelection();composing=true;
    composition={generation,startTrusted:e.isTrusted,editTrusted:false,editText:null};
    keyboard.classList.add("composing");positionKeyboard();
}
function compositionend(e) {
    const finished=composition;
    composing=false;composition=null;keyboard.classList.remove("composing");keyboard.value="";positionKeyboard();
    if(finished&&e.data) send({kind:"text",text:e.data,generation:finished.generation,trusted:e.isTrusted,
        composition_input:finished.startTrusted&&finished.editTrusted&&finished.editText===e.data});
}
function cancelComposition() {
    // Late composition input is ignored until a fresh beforeinput starts an edit.
    composition=null;composing=false;beforeEdit={composition:true};keyboard.classList.remove("composing");keyboard.value="";positionKeyboard();
}
function positionKeyboard() {
    if(!grid||!latest) return;
    const [col,row]=latest.frame.cursor, left=grid.x(col)/grid.ratio, top=grid.y(row)/grid.ratio;
    const height=(grid.y(row+1)-grid.y(row))/grid.ratio;
    Object.assign(keyboard.style,{left:left+"px",top:top+"px",height:height+"px",
        width:(composing?Math.max(1,Math.min(320,surface.clientWidth-left)):1)+"px",
        fontSize:height*latest.settings.font_size/24+"px",fontFamily:fontFamily(latest.settings.cjk_font)});
}
function focus() { if(document.activeElement!==keyboard) keyboard.focus({preventScroll:true}); }
function restoreEditingFocus(){keyboard.blur();focus();}
function pointerdown(e) {
    if(e.button!==0) return;
    const bounds=viewport.getBoundingClientRect();
    if(e.clientX>=bounds.left+viewport.clientWidth || e.clientY>=bounds.top+viewport.clientHeight) return;
    // Prevent the scroll container's default focus step from overriding ours.
    e.preventDefault(); focus(); clearSelection();
    const point=cellAt(e,false);
    if(point&&!composing) {
        dragging={anchor:point,x:e.clientX,y:e.clientY,moved:false,pointer:e.pointerId,link:linkAt(point)};
        viewport.setPointerCapture(e.pointerId);
    }
}
function cellAt(e,clamp) {
    if(!grid||!latest) return null;
    const bounds=canvas.getBoundingClientRect();
    if(!clamp&&(e.clientX<bounds.left||e.clientX>=bounds.right||e.clientY<bounds.top||e.clientY>=bounds.bottom)) return null;
    const px=(e.clientX-bounds.left)*canvas.width/bounds.width,py=(e.clientY-bounds.top)*canvas.height/bounds.height;
    const locate=(value,count,edge)=>{let at=0;while(at+1<count&&value>=edge(at+1))at++;return at;};
    return {col:locate(px,latest.frame.cols,grid.x),row:locate(py,latest.frame.rows,grid.y)};
}
function pointermove(e) {
    if(!dragging) {setLinkHover(linkAt(cellAt(e,false)));return;}
    dragging.moved ||= Math.hypot(e.clientX-dragging.x,e.clientY-dragging.y)>3;
    if(dragging.moved) requestSelection({mode:"range",anchor:dragging.anchor,focus:cellAt(e,true)});
}
function pointerup(e) {
    if(!dragging) return;
    pointermove(e);
    if(!dragging.moved&&dragging.link>=0&&dragging.link===linkAt(cellAt(e,false))) {
        send({kind:"open_link",generation,revision,index:dragging.link,trusted:e.isTrusted});
    }
    dragging=null;
    if(viewport.hasPointerCapture(e.pointerId)) viewport.releasePointerCapture(e.pointerId);
}
function cancelDrag() {
    if(dragging&&viewport.hasPointerCapture(dragging.pointer)) viewport.releasePointerCapture(dragging.pointer);
    dragging=null;
}
function pointercancel(){cancelDrag();clearSelection();}
function lostcapture(){dragging=null;}
function hasSelection(){return selectionActive;}
function clearSelection() {
    selectionActive=false;selectionFingerprint=null;selectionNext=null;selectionCache=null;selectionLayer.replaceChildren();
    surfaceNote.textContent="";
    delete surface.dataset.selectionReady;
}
function requestSelection(selection) {
    const fingerprint=JSON.stringify({generation,revision,selection});
    if(fingerprint===selectionFingerprint) return;
    selectionFingerprint=fingerprint;selectionActive=true;
    selectionNext={generation,revision,selection}; selectionCache=null;
    delete surface.dataset.selectionReady;flushSelection();
}
function flushSelection() {
    if(selectionBusy||!selectionNext) return;
    selectionBusy={...selectionNext,request:++selectionSequence};selectionNext=null;
    send({kind:"select",...selectionBusy});
}
function receiveSelection(reply) {
    if(!selectionBusy||selectionBusy.request!==reply.request) return;
    selectionBusy=null;
    if(selectionActive&&!selectionNext&&reply.generation===generation&&reply.revision===revision&&reply.value) {
        selectionCache=reply;surfaceNote.textContent="";surface.dataset.selectionReady=String(reply.request);drawSelection();
    }
    flushSelection();
}
function drawSelection() {
    selectionLayer.replaceChildren();
    if(!selectionCache||!grid) return;
    const nodes=document.createDocumentFragment();
    for(const span of selectionCache.value.spans) {
        const box=document.createElement("div");box.className="selection-row";
        Object.assign(box.style,{left:grid.x(span.start)/grid.ratio+"px",top:grid.y(span.row)/grid.ratio+"px",
            width:(grid.x(span.end)-grid.x(span.start))/grid.ratio+"px",height:(grid.y(span.row+1)-grid.y(span.row))/grid.ratio+"px"});
        nodes.append(box);
    }
    selectionLayer.append(nodes);
}
function copy(e) {
    if(!hasSelection()||document.activeElement!==keyboard) return;
    e.preventDefault();
    if(!selectionCache) {surfaceNote.textContent="選取尚未準備好，請再複製";return;}
    if(!e.clipboardData) {surfaceNote.textContent="此環境未提供剪貼簿事件資料";return;}
    try {
        e.clipboardData.setData("text/plain",selectionCache.value.text);
        const {generation,revision,request}=selectionCache;
        send({kind:"copied",generation,revision,request,trusted:e.isTrusted});
    } catch(_) { surfaceNote.textContent="無法寫入剪貼簿，請重試"; }
}
function doubleclick(e) {
    const point=cellAt(e,false);if(point&&!composing) requestSelection({mode:"line",row:point.row});
}
function selectPage(){if(latest&&!composing){requestSelection({mode:"all"});focus();}}
function linkAt(point) {
    if(!point||!latest) return -1;
    return latest.links.findIndex(link=>link.row===point.row&&link.start<=point.col&&point.col<link.end);
}
function setLinkHover(index,redraw=true) {
    if(index===hoveredLink) return;
    hoveredLink=index;canvas.style.cursor=index>=0?"pointer":"default";
    canvas.title=index>=0?latest.links[index].url+"\n點擊開啟；右鍵選取後可複製":"";
    if(redraw&&latest) draw(latest);
}
const leaveLink=()=>setLinkHover(-1);
function selectLink(e) {
    const index=linkAt(cellAt(e,false));
    if(index<0||composing) return;
    e.preventDefault();cancelDrag();focus();
    const link=latest.links[index];
    requestSelection({mode:"range",anchor:{col:link.start,row:link.row},focus:{col:link.end-1,row:link.row}});
}
function activate() { if(!document.querySelector("dialog[open]")&&!chrome.contains(document.activeElement)) focus(); }
function paste(e) {
    e.preventDefault();
    if(pastePending||pasteDialog.open) return;
    const text=e.clipboardData?.getData("text/plain");
    cancelComposition();cancelDrag();clearSelection();
    if(text===undefined) {surfaceNote.textContent="此環境無法讀取貼上的文字";return;}
    if(text.length>16384) {surfaceNote.textContent="貼上文字過長；整段未送出";return;}
    pastePending=true;surface.dataset.pasteResult="pending";send({kind:"paste",text,generation,trusted:e.isTrusted});
}
function confirmPaste(e){send({kind:"paste_confirm",trusted:e.isTrusted});}
function cancelPaste(e){e.preventDefault();send({kind:"paste_cancel",trusted:e.isTrusted});}
function openFonts(e){send({kind:"settings_open",trusted:e.isTrusted});}
function fillFonts(values) {
    for(const [name,field] of Object.entries(fontFields)) field.value=values[name];
}
function readFonts() {
    return {version:1,ascii_font:fontFields.ascii_font.value,cjk_font:fontFields.cjk_font.value,
        font_size:Number(fontFields.font_size.value),baseline:Number(fontFields.baseline.value)};
}
function flushFontPreview() {
    if(fontPreviewBusy||!fontPreviewNext||!fontDialog.open||stopped) return;
    fontPreviewBusy=true;send({kind:"settings_preview",...fontPreviewNext});fontPreviewNext=null;
}
function previewFonts() {
    clearTimeout(fontTimer);fontRequest++;fontNote.textContent="";
    fontTimer=setTimeout(()=>{fontTimer=0;fontPreviewNext={request:fontRequest,values:readFonts()};flushFontPreview();},150);
}
function editFont(){fontPreset.value="custom";previewFonts();}
function chooseFontPreset(){if(fontDraft?.presets[fontPreset.value]) {fillFonts(fontDraft.presets[fontPreset.value]);previewFonts();}}
function resetFonts(){if(fontDraft){fillFonts(fontDraft.defaults);fontPreset.value="custom";previewFonts();}}
function closeFonts(e) {
    e?.preventDefault();if(fontSaving) return;
    clearTimeout(fontTimer);fontTimer=0;fontRequest++;fontPreviewNext=null;
    fontDialog.close();restoreEditingFocus();
}
function saveFonts(e) {
    if(fontSaving) return;
    fontSaving=true;fontSave.disabled=true;fontCancel.disabled=true;
    send({kind:"settings_save",values:readFonts(),trusted:e.isTrusted});
}
const pasteObserver=new MutationObserver(()=>{
    if(pasteDialog.dataset.active==="true") {
        cancelComposition();cancelDrag();clearSelection();
        if(!pasteDialog.open) pasteDialog.showModal();
        pasteCancel.focus();
    } else if(pasteDialog.open) {
        pasteDialog.close();pastePending=false;
        // Restore the editing host, not just the DOM activeElement, after a modal.
        restoreEditingFocus();
    }
});
pasteObserver.observe(pasteDialog,{attributes:true,attributeFilter:["data-active"]});
function exportScreen(e) {
    if(exportBusy||!latest||stopped) return;
    exportBusy=true;exportButton.disabled=true;surface.dataset.exportResult="pending";
    const packet=latest;
    exportTask=(async()=>{
        const target=document.createElement("canvas");target.width=960;target.height=576;
        try {
            render(target,packet,{cursor:false,hovered:-1});
            const blob=await new Promise((resolve,reject)=>target.toBlob(value=>value?resolve(value):reject(new Error("無法編碼 PNG")),"image/png"));
            if(blob.size>3*1024*1024) throw new Error("PNG 超過匯出大小上限");
            const png=await new Promise((resolve,reject)=>{
                const reader=new FileReader();reader.onload=()=>resolve(reader.result);reader.onerror=()=>reject(new Error("無法準備 PNG"));reader.readAsDataURL(blob);
            });
            if(!stopped) send({kind:"export_png",generation:packet.generation,revision:packet.frame.revision,png,trusted:e.isTrusted});
        } catch(error) {
            exportBusy=false;exportButton.disabled=false;
            if(!stopped) {surface.dataset.exportResult="failed";surfaceNote.textContent=String(error);restoreEditingFocus();}
        } finally {target.width=0;target.height=0;}
    })();
}
function showChrome() { chrome.classList.add("open"); }
function hideChrome() { chrome.classList.remove("open"); }
function zoomTo(value) {
    if(stopped||!latest) return;
    zoom=Math.max(.25,Math.min(4,value)); draw(latest);
    viewport.scrollLeft=Math.max(0,(surface.offsetWidth-viewport.clientWidth)/2);
    viewport.scrollTop=Math.max(0,(surface.offsetHeight-viewport.clientHeight)/2); focus();
}
function wheel(e) { if(e.ctrlKey) { e.preventDefault(); zoomTo(zoom+(e.deltaY<0?.25:-.25)); } }
const zoomIn=()=>zoomTo(zoom+.25), zoomOut=()=>zoomTo(zoom-.25), zoomFit=()=>zoomTo(1);
const bindings=[
    [keyboard,"keydown",keydown],[keyboard,"input",input],
    [keyboard,"beforeinput",beforeinput],
    [keyboard,"blur",cancelComposition],
    [keyboard,"compositionstart",compositionstart],[keyboard,"compositionend",compositionend],
    [keyboard,"paste",paste],[viewport,"pointerdown",pointerdown],
    [viewport,"pointermove",pointermove],[viewport,"pointerup",pointerup],
    [viewport,"pointercancel",pointercancel],[viewport,"lostpointercapture",lostcapture],
    [canvas,"dblclick",doubleclick],[selectAll,"click",selectPage],[document,"copy",copy],
    [canvas,"contextmenu",selectLink],[canvas,"pointerleave",leaveLink],
    [pasteSend,"click",confirmPaste],[pasteCancel,"click",cancelPaste],[pasteDialog,"cancel",cancelPaste],
    [exportButton,"click",exportScreen],
    [fontButton,"click",openFonts],[fontPreset,"change",chooseFontPreset],
    ...Object.values(fontFields).map(field=>[field,"input",editFont]),
    [fontDefaults,"click",resetFonts],[fontCancel,"click",closeFonts],[fontDialog,"cancel",closeFonts],[fontSave,"click",saveFonts],
    [chrome,"pointerenter",showChrome],[chrome,"pointerleave",hideChrome],
    [plus,"click",zoomIn],[minus,"click",zoomOut],[fit,"click",zoomFit],
    [viewport,"wheel",wheel,{passive:false}],[window,"resize",repaint],[window,"focus",activate],
];
for(const [target,event,callback,options] of bindings) target.addEventListener(event,callback,options);
function colours(style) {
    const fg=style.fg+(style.bright&&style.fg<8?8:0), bg=style.bg;
    return style.inverse?[palette[bg],palette[fg]]:[palette[fg],palette[bg]];
}
// Filled BBS graphics follow the grid, independent of font metrics/fallback.
function graphic(ctx,text,x,y,w,h) {
    if([...text].length!==1) return false;
    const code=text.codePointAt(0);
    const rect=(l,t,r,b)=>{
        const left=Math.round(x+w*l),top=Math.round(y+h*t);
        ctx.fillRect(left,top,Math.round(x+w*r)-left,Math.round(y+h*b)-top);
    };
    if(code===0x2580) rect(0,0,1,.5);
    else if(code>=0x2581&&code<=0x2588) rect(0,1-(code-0x2580)/8,1,1);
    else if(code>=0x2589&&code<=0x258f) rect(0,0,(0x2590-code)/8,1);
    else if(code===0x2590) rect(.5,0,1,1);
    else if(code===0x2594) rect(0,0,1,.125);
    else if(code===0x2595) rect(.875,0,1,1);
    else if(code>=0x2596&&code<=0x259f) {
        const mask=[4,8,1,13,9,7,11,2,6,14][code-0x2596];
        for(let q=0;q<4;q++) if(mask&(1<<q)) rect((q%2)/2,Math.floor(q/2)/2,(q%2+1)/2,(Math.floor(q/2)+1)/2);
    } else if(code>=0x25e2&&code<=0x25e5) {
        const corners=[[[1,0],[1,1],[0,1]],[[0,0],[0,1],[1,1]],[[0,0],[1,0],[0,1]],[[0,0],[1,0],[1,1]]][code-0x25e2];
        ctx.beginPath(); corners.forEach(([u,v],i)=>i===0?ctx.moveTo(x+w*u,y+h*v):ctx.lineTo(x+w*u,y+h*v));
        ctx.closePath(); ctx.fill();
    } else return false;
    return true;
}
function draw(packet) {
    const frame=packet.frame,ratio=devicePixelRatio||1,width=frame.cols*12,height=frame.rows*24;
    const available=viewport.getBoundingClientRect();
    const fitting=Math.min(available.width/width,available.height/height);
    // At most 16M RGBA pixels (64 MiB), and 8192 pixels per dimension.
    const capacity=Math.min(Math.sqrt(16*1024*1024/(width*height)),8192/width,8192/height);
    const physical=Math.min(fitting*zoom*ratio,capacity);
    canvas.width=Math.max(1,Math.floor(width*physical)); canvas.height=Math.max(1,Math.floor(height*physical));
    surface.style.width=canvas.width/ratio+"px"; surface.style.height=canvas.height/ratio+"px";
    level.textContent=Math.round(physical/ratio/fitting*100)+"%";
    grid={...render(canvas,packet,{cursor:true,hovered:hoveredLink}),ratio};
    positionKeyboard();drawSelection();draws++;return grid;
}
function render(target,packet,{cursor=true,hovered=-1}={}) {
    const frame=packet.frame,width=frame.cols*12,height=frame.rows*24;
    const ctx=target.getContext("2d");
    const scale=Math.min(target.width/width,target.height/height);
    const x=col=>Math.round(col*12*scale),y=row=>Math.round(row*24*scale);
    ctx.setTransform(1,0,0,1,0,0); ctx.fillStyle="#000"; ctx.fillRect(0,0,target.width,target.height);
    // Opaque backgrounds first; both sides of every shared edge use one integer.
    for(const glyph of frame.glyphs) for(let half=0;half<glyph.width;half++) {
        const [,bg]=colours(half===0?glyph.left:glyph.right),col=glyph.col+half;
        ctx.fillStyle=bg; ctx.fillRect(x(col),y(glyph.row),x(col+1)-x(col),y(glyph.row+1)-y(glyph.row));
    }
    ctx.textBaseline="alphabetic";
    for(const glyph of frame.glyphs) {
        if(glyph.text===" ") continue;
        ctx.font=(packet.settings.font_size*scale)+"px "+fontFamily(glyph.width===2?packet.settings.cjk_font:packet.settings.ascii_font);
        const left=x(glyph.col),top=y(glyph.row),span=x(glyph.col+glyph.width)-left,h=y(glyph.row+1)-top;
        const advance=Math.max(ctx.measureText(glyph.text).width,1);
        for(let half=0;half<glyph.width;half++) {
            const [fg]=colours(half===0?glyph.left:glyph.right),col=glyph.col+half;
            ctx.save(); ctx.beginPath(); ctx.rect(x(col),top,x(col+1)-x(col),h); ctx.clip(); ctx.fillStyle=fg;
            if(!graphic(ctx,glyph.text,left,top,span,h)) {
                ctx.translate(left,top+packet.settings.baseline*scale); ctx.scale(span/advance,1); ctx.fillText(glyph.text,0,0);
            }
            ctx.restore();
        }
    }
    if(cursor&&frame.cursor_visible) {
        const [col,row]=frame.cursor,bottom=y(row+1),thickness=Math.max(1,Math.round(2*scale));
        ctx.fillStyle=palette[7]; ctx.fillRect(x(col),bottom-thickness,x(col+1)-x(col),thickness);
    }
    const link=packet.links[hovered];
    if(link) {ctx.fillStyle=palette[7];ctx.fillRect(x(link.start),y(link.row+1)-1,x(link.end)-x(link.start),1);}
    return {x,y};
}
function repaint() {
    if(stopped||!latest||animation) return;
    animation=requestAnimationFrame(()=>{animation=0;if(!stopped) draw(latest);});
}
const observer=new ResizeObserver(repaint); observer.observe(viewport);
function checkArt({x,y}) {
    const pixels=ctx.getImageData(x(0),y(19),x(16)-x(0),y(21)-y(19)).data;
    let seams=0;
    for(let i=0;i<pixels.length;i+=4) if(pixels[i]!==128||pixels[i+1]!==0||pixels[i+2]!==0) seams++;
    const sample=(col,u,v)=>Array.from(ctx.getImageData(Math.floor(x(col)+(x(col+2)-x(col))*u),
        Math.floor(y(21)+(y(22)-y(21))*v),1,1).data).slice(0,3);
    const cases=[[0,.5,.25,false],[0,.5,.75,true],[2,.25,.5,true],[2,.75,.5,false],
        [4,.75,.75,true],[4,.25,.25,false],[6,.25,.75,true],[6,.75,.25,false],
        [8,.25,.25,true],[8,.75,.75,false],[10,.75,.25,true],[10,.25,.75,false]];
    const geometry=cases.every(([col,u,v,ink])=>sample(col,u,v).join(",")===(ink?"128,0,0":"0,0,128"));
    return {seams,geometry};
}
function probe(packet,geometry) {
    let red=0,green=0;
    const {x,y,ratio}=geometry,area=ctx.getImageData(x(0),y(3),x(2)-x(0),y(4)-y(3)).data;
    for(let i=0;i<area.length;i+=4) {
        const [r,g,b]=area.slice(i,i+3);
        if(r>70&&r>g*1.5&&r>b*1.5) red++;
        if(g>70&&g>r*1.5&&g>b*1.5) green++;
    }
    const sample=col=>Array.from(ctx.getImageData(x(col)+1,y(3)+1,1,1).data);
    const left_bg=sample(0),right_bg=sample(1);
    // Probe solid geometry so font-specific antialiasing cannot redefine ANSI RGB.
    const ink=(col,row)=>Array.from(ctx.getImageData(x(col)+1,y(row)+1,1,1).data).slice(0,3);
    const colour_samples=Array.from({length:16},(_,i)=>ink(i*2,15));
    colour_samples.push(...Array.from({length:4},(_,i)=>ink(i*2,16)));
    const bounds=viewport.getBoundingClientRect(),near=(a,b)=>Math.abs(a-b)<1;
    const fills_body=near(bounds.left,0)&&near(bounds.top,0)&&near(bounds.right,innerWidth)&&near(bounds.bottom,innerHeight);
    chrome.dispatchEvent(new PointerEvent("pointerleave"));const hidden=getComputedStyle(toolbar).opacity==="0";
    chrome.dispatchEvent(new PointerEvent("pointerenter"));const shown=getComputedStyle(toolbar).opacity==="1";
    chrome.dispatchEvent(new PointerEvent("pointerleave"));
    const hover_controls=hidden&&shown&&getComputedStyle(toolbar).opacity==="0"&&near(viewport.getBoundingClientRect().top,0);
    const checks=[checkArt(geometry)],zoom_samples=[[canvas.width,canvas.height]];
    for(let i=0;i<2;i++){plus.click();checks.push(checkArt(draw(packet)));zoom_samples.push([canvas.width,canvas.height]);}
    const proportional_zoom=zoom_samples.every(([w,h])=>Math.abs(w*576-h*960)<=960)
        &&zoom_samples[1][0]>zoom_samples[0][0]&&zoom_samples[2][0]>zoom_samples[1][0];
    fit.click();
    surface.dataset.probeReady="true";
    send({kind:"probe",png:canvas.toDataURL("image/png"),red_pixels:red,green_pixels:green,left_bg,right_bg,
        fonts:document.fonts.status,dpr:ratio,pixel_size:[canvas.width,canvas.height],font_stack:fontDescription(packet.settings)+" / "+fontDescription(packet.settings,true),fills_body,
        colour_samples,art_seam_pixels:checks[0].seams,art_zoom_seams:checks.map(c=>c.seams),
        art_geometry:checks.every(c=>c.geometry),hover_controls,proportional_zoom,zoom_samples});
}
async function paint(packet) {
    if(packet.generation<generation||(packet.generation===generation&&packet.frame.revision<revision)) return;
    await loadFonts(packet.settings);
    if(packet.generation!==generation||packet.frame.revision!==revision) {clearSelection();cancelDrag();setLinkHover(-1,false);}
    if(packet.generation!==generation){
        pastePending=false;cancelComposition();focus();
    }
    generation=packet.generation;revision=packet.frame.revision;latest=packet;
    const geometry=draw(packet);send({kind:"painted",revision,generation});if(packet.probe) probe(packet,geometry);
    surface.dataset.revision=String(revision);
    surface.dataset.fontSize=String(packet.settings.font_size);surface.dataset.baseline=String(packet.settings.baseline);
}
focus();
try {
    while(true){const packet=await dioxus.recv();if(packet.stop) break;
        if(packet.settings_open) {
            fontDraft=packet.settings_open;fillFonts(fontDraft.values);fontPreset.value="custom";
            fontNote.textContent=fontDraft.warning||"";cancelComposition();cancelDrag();clearSelection();
            if(!fontDialog.open) fontDialog.showModal();
            await loadFonts(fontDraft.values);render(fontPreview,fontDraft.sample,{cursor:false});
            fontFields.ascii_font.focus();continue;
        }
        if(packet.font_preview) {
            const preview=packet.font_preview;fontPreviewBusy=false;
            if(fontDialog.open&&preview.request===fontRequest) {
                if(preview.packet){await loadFonts(preview.packet.settings);render(fontPreview,preview.packet,{cursor:false});fontPreview.dataset.request=String(preview.request);
                    fontPreview.dataset.fontSize=String(preview.packet.settings.font_size);fontPreview.dataset.baseline=String(preview.packet.settings.baseline);}
                if(preview.error) fontNote.textContent=preview.error;
            }
            flushFontPreview();continue;
        }
        if(typeof packet.settings_saved==="boolean") {
            fontSaving=false;fontSave.disabled=false;fontCancel.disabled=false;
            if(packet.settings_saved) closeFonts();
        }
        if(packet.selection){receiveSelection(packet.selection);continue;}
        if(packet.paste_result) {surface.dataset.pasteResult=packet.paste_result;pastePending=packet.paste_result==="preview";}
        if(packet.link_result) surface.dataset.linkResult=packet.link_result;
        if(packet.export_result) {surface.dataset.exportResult=packet.export_result;exportBusy=false;exportButton.disabled=false;restoreEditingFocus();}
        if(packet.notice) (fontDialog.open?fontNote:surfaceNote).textContent=packet.notice;
        if(packet.paste_result||packet.link_result||packet.export_result||packet.notice) continue;
        await paint(packet);
    }
} catch(e){send({kind:"error",message:String(e)});}
finally {
    stopped=true;latest=null;observer.disconnect();pasteObserver.disconnect();if(animation){cancelAnimationFrame(animation);animation=0;}
    clearTimeout(fontTimer);fontTimer=0;fontPreviewNext=null;
    for(const [target,event,callback,options] of bindings) target.removeEventListener(event,callback,options);
    if(exportTask) await exportTask;
    if(pasteDialog.open) pasteDialog.close();
    if(fontDialog.open) fontDialog.close();
    cancelComposition();keyboard.blur();hideChrome();cancelDrag();selectionBusy=null;clearSelection();
    const before=sent,beforeDraws=draws,beforeZoom=zoom;
    keyboard.dispatchEvent(new KeyboardEvent("keydown",{key:"ArrowUp",bubbles:true}));
    keyboard.dispatchEvent(new InputEvent("input",{data:"a",inputType:"insertText",bubbles:true}));
    surface.dispatchEvent(new PointerEvent("pointerdown",{bubbles:true}));window.dispatchEvent(new Event("resize"));plus.click();
    chrome.dispatchEvent(new PointerEvent("pointerenter"));
    fontButton.click();fontFields.font_size.dispatchEvent(new InputEvent("input",{bubbles:true}));
    send({kind:"stopped",clean:sent===before&&draws===beforeDraws&&zoom===beforeZoom&&animation===0
        &&fontTimer===0&&!fontDialog.open&&!chrome.classList.contains("open")&&document.activeElement!==keyboard});
}
