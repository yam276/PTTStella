const localization=await dioxus.recv();
const tr=createSurfaceI18n(localization,fluent);
document.documentElement.lang=localization.payload.language;
const canvas=document.getElementById("screen"), keyboard=document.getElementById("keyboard");
const surface=document.getElementById("surface"), viewport=document.querySelector(".workspace");
const chrome=document.getElementById("chrome"), toolbar=chrome.querySelector("header");
const minus=document.getElementById("zoom-out"), plus=document.getElementById("zoom-in");
const fit=document.getElementById("zoom-fit"), level=document.getElementById("zoom-level");
const selectionLayer=document.getElementById("selection"), selectAll=document.getElementById("select-all");
const surfaceNote=document.getElementById("surface-note");
const pasteDialog=document.getElementById("paste-dialog"), pasteSend=document.getElementById("paste-send"), pasteCancel=document.getElementById("paste-cancel");
const exportButton=document.getElementById("export-png");
const exportRows=document.getElementById("export-rows");
const keepAlive=document.getElementById("keep-alive");
const stationMouseButton=document.getElementById("station-mouse"),stationMouseStatus=document.getElementById("station-mouse-status");
const hoverPosition=document.getElementById("hover-position");
let hoverPreference="top_right",hoverCandidate=null,hoverTimer=0,hoverSequence=0,pointerClient=null;
let pasteConfirmProbe=false;
function cancelHover(forget=false) {
    clearTimeout(hoverTimer);hoverTimer=0;
    const candidate=hoverCandidate;hoverCandidate=null;
    if(forget)pointerClient=null;
    media.hideHover();
    if(candidate?.started&&!stopped)send({kind:"hover",presentation:candidate.presentation,sequence:candidate.sequence,stamp:candidate.stamp,index:null,trusted:true});
}
function previewHover(index) {
    if(filterUi.blocked()||!pointerClient||hoverPreference==="off"||blocked()||dragging||hasSelection()||!latest?.connected||!latest.image_links.includes(index)) {cancelHover();return;}
    const identity=JSON.stringify(latest.links[index]);
    if(hoverCandidate?.identity===identity) {media.locate(pointerClient,hoverPreference);return;}
    cancelHover();
    const candidate={identity,index,sequence:++hoverSequence,stamp:stamp(),presentation:latest.presentation,started:false,trusted:pointerClient.trusted};hoverCandidate=candidate;
    hoverTimer=setTimeout(()=>{hoverTimer=0;
        if(stopped||hoverCandidate!==candidate||blocked()||dragging||!document.hasFocus()||!sameStamp(candidate.stamp,stamp())||candidate.presentation!==latest?.presentation)return;
        candidate.started=true;send({kind:"hover",presentation:candidate.presentation,sequence:candidate.sequence,stamp:candidate.stamp,index:candidate.index,trusted:candidate.trusted});
    },350);
}
function changeHoverPreference(e){send({kind:"hover_preference",position:hoverPosition.value,trusted:e.isTrusted});}
const vimToggle=document.getElementById("vim-toggle"),vimOnce=document.getElementById("vim-once"),vimStatus=document.getElementById("vim-status");
let keyboardState={enabled:false,mode:"input",pending:false,prefix:""};
function policy(action,e,code="") {send({kind:"keyboard_state",action,code,generation,trusted:e?.isTrusted??true});}
function toggleVim(e){policy("toggle",e);restoreEditingFocus();}
function armNative(e){policy("once",e);restoreEditingFocus();}
function keyup(e){if(!e.metaKey)policy("release",e,e.code);}
function receiveKeyboard(state){keyboardState=state;vimToggle.textContent=state.enabled?tr("vim-on"):tr("vim-off");
    keyboard.readOnly=state.enabled&&!["input","once"].includes(state.mode);
    vimToggle.setAttribute("aria-pressed",String(state.enabled));vimOnce.hidden=!state.enabled||state.mode==="input";
    vimOnce.setAttribute("aria-pressed",String(state.mode==="once"));
    const reason={unsupported_grid:tr("reason-grid"),incomplete:tr("reason-incomplete"),unmapped:tr("reason-unmapped"),unrecognized:tr("reason-unrecognized"),invalid_cursor:tr("reason-cursor"),board_header:tr("reason-board-header"),board_rows:tr("reason-board-rows")}[state.suspended_reason]||tr("reason-unknown");
    vimStatus.textContent=!state.enabled?"":({normal:"VIM NORMAL",input:"PTT INPUT",once:tr("vim-once"),suspended:tr("vim-suspended",{reason})}[state.mode])+" "+state.prefix;
    surface.dataset.keyboardMode=state.mode;surface.dataset.vim=String(state.enabled);refreshKeepAlive();}
const navigationMarker=document.getElementById("navigation-marker"),browseControls=document.getElementById("browse-controls");
const browseBack=document.getElementById("browse-back"),browsePrevious=document.getElementById("browse-previous"),browseNext=document.getElementById("browse-next");
const browseHint=document.getElementById("browse-hint");
function invalidatePresentation(){stationMouse.revoke();cancelHover(true);cancelComposition();cancelDrag();clearSelection();released=null;setHover(null);}
const ctx=canvas.getContext("2d",{willReadFrequently:true});
const palette=["#000000","#800000","#008000","#808000","#000080","#800080","#008080","#c0c0c0",
    "#808080","#ff0000","#00ff00","#ffff00","#0000ff","#ff00ff","#00ffff","#ffffff"];
const {family:fontFamily,description:fontDescription,load:loadFonts,note:fontLoadNote,invalidate:invalidateFonts}=
    createSurfaceFonts({document,FontFace,tr});
let generation=0, revision=-1, composing=false, composition=null, beforeEdit=null, sent=0;
let inputKey=false;
let latest=null, animation=0, stopped=false, draws=0, zoom=1;
let grid=null, dragging=null, selectionNext=null, selectionBusy=null, selectionCache=null, selectionSequence=0;
let selectionActive=false, selectionFingerprint=null;
let pastePending=false;
let hoveredLink=-1;
let hoveredArticle=null,released=null,clickSequenceAction=false,navigationLocked=false,navigationRequest=false;
let exportBusy=false, exportTask=null;
let keepAliveGate=null;
const send=value=>{ sent++; dioxus.send(value); };
const stamp=()=>({generation,revision,input_epoch:latest?.input_epoch??0});
const sameStamp=(a,b)=>a.generation===b.generation&&a.revision===b.revision&&a.input_epoch===b.input_epoch;
const blocked=()=>composing||pastePending||!!document.querySelector("dialog[open]");
const stationMouse=createStationMouse({current:()=>latest,point:cellAt,viewport,send,blocked,
    prepare:()=>{cancelHover(true);policy("reset",{isTrusted:true});released=null;cancelDrag();clearSelection();focus();},
    changed:()=>{surface.dataset.stationMouseActive=String(stationMouse.active());refreshKeepAlive();}});
function toggleStationMouse(e){
    if(!e.isTrusted||stopped||!latest)return;
    stationMouse.revoke();
    send({kind:"station_mouse_enabled",generation,enabled:!latest.mouse.enabled,trusted:true});
    focus();
}
function refreshStationMouse(){
    const state=latest?.mouse;
    stationMouseButton.textContent=tr(state?.enabled===false?"station-mouse-off":"station-mouse-on");
    stationMouseButton.setAttribute("aria-pressed",String(state?.enabled!==false));
    stationMouseStatus.textContent=!latest?.connected||!state?.enabled?"":tr(state.route==="station"?"station-mouse-active":state.route==="blocked"?"station-mouse-blocked":"station-mouse-local");
    surface.dataset.stationMouseRoute=state?.route??"local";
}
const canNavigate=()=>!stopped&&!!latest?.connected&&['article_list','article_reading'].includes(latest.navigation.kind)&&!latest.input_pending&&!navigationLocked&&!navigationRequest&&!blocked();
function invalidateNavigation(){navigationLocked=true;released=null;cancelDrag();setHover(null);refreshNavigation();}
function sendInput(value){invalidateNavigation();send(value);}
// Only classify editing vs command events here. Rust owns key translation and
// the context-dependent conversion from browsing intents to PTT bytes.
const commandKeys=new Set(["Enter","Backspace","Tab","Escape","ArrowUp","ArrowDown","ArrowRight","ArrowLeft","Home","End","PageUp","PageDown","Delete"]);
function keydown(e) {
    if(e.isTrusted)stationMouse.revoke();
    cancelHover(true);
    if(pastePending) {e.preventDefault();return;}
    // Older WebKit can end composition before its confirming keydown. That
    // event still carries 229 even though isComposing is already false.
    // Leave the native IME action intact; never forward it as a BBS command.
    const browsing=keyboardState.enabled&&!["input","once"].includes(keyboardState.mode);
    const physical=browsing&&/^(Key[A-Z]|Digit[0-9])$/.test(e.code);
    if(e.isComposing||composing||((e.keyCode===229||e.key==="Process")&&!physical)||e.metaKey) return;
    // Let the native copy action fire; copy data is already prepared by Rust.
    if(e.ctrlKey&&e.key.toLowerCase()==="c"&&hasSelection()) return;
    if(e.ctrlKey&&e.key.toLowerCase()==="v") return;
    if(e.key==="Escape"&&hasSelection()) { e.preventDefault(); cancelDrag();clearSelection(); return; }
    if(e.ctrlKey&&["+","=","-","0"].includes(e.key)) {
        e.preventDefault(); zoomTo(e.key==="0"?1:zoom+(e.key==="-"?-.25:.25)); return;
    }
    const command=commandKeys.has(e.key)||(e.ctrlKey&&/^[a-z]$/i.test(e.key));
    if(command||physical||(browsing&&e.key.length===1)) {clearSelection();e.preventDefault();}
    if(command||physical||e.key.length===1||e.key==="Dead") {
        inputKey=!command;
        send({kind:"key",key:{key:e.key,code:e.code,ctrl:e.ctrlKey,alt:e.altKey,shift:e.shiftKey,repeat:e.repeat},generation,trusted:e.isTrusted});
    }
}
function input(e) {
    if(e.isTrusted)stationMouse.revoke();
    cancelHover(true);
    if(pastePending) {keyboard.value="";return;}
    if(composing||e.isComposing) {
        if(composition) {composition.editTrusted=e.isTrusted;composition.editText=keyboard.value;}
        positionKeyboard();
        return;
    }
    const text=e.data||keyboard.value; keyboard.value="";
    // Enter bytes have one owner: keydown. A textarea edit caused by an IME's
    // confirming Return must not become a second text/newline submission.
    if(lineBreak(e)) {beforeEdit=null;return;}
    // A trailing input for an already committed composition belongs to that edit,
    // while a new beforeinput identifies a new edit even when its text is equal.
    if(beforeEdit?.composition) {beforeEdit=null;return;}
    if(text) { clearSelection(); sendInput({kind:"text",text,generation,trusted:e.isTrusted}); }
}
const lineBreak=e=>e.inputType==="insertLineBreak"||e.inputType==="insertParagraph";
function beforeinput(e) {
    if(!inputKey&&!composing&&!e.isComposing&&!lineBreak(e)) policy("independent_edit",e);
    inputKey=false;
    beforeEdit={composition:composing||e.isComposing||/Composition/.test(e.inputType||"")};
    if(lineBreak(e)&&!composing&&!e.isComposing) e.preventDefault();
}
function compositionstart(e) {
    stationMouse.revoke();
    cancelHover(true);
    inputKey=false;
    policy("composition_start",e);
    clearSelection();composing=true;
    composition={generation,startTrusted:e.isTrusted,editTrusted:false,editText:null};
    keyboard.classList.add("composing");positionKeyboard();
    cancelDrag();released=null;setHover(null);refreshNavigation();
}
function compositionend(e) {
    const finished=composition;
    composing=false;composition=null;keyboard.classList.remove("composing");keyboard.value="";positionKeyboard();
    if(finished&&e.data) sendInput({kind:"text",text:e.data,generation:finished.generation,trusted:e.isTrusted,composed:true,
        composition_input:finished.startTrusted&&finished.editTrusted&&finished.editText===e.data});
    policy("composition_end",e);
    refreshNavigation();
}
function cancelComposition() {
    // Late composition input is ignored until a fresh beforeinput starts an edit.
    composition=null;composing=false;beforeEdit={composition:true};keyboard.classList.remove("composing");keyboard.value="";positionKeyboard();
    refreshNavigation();
}
function positionKeyboard() {
    if(!grid||!latest) return;
    const [col,row]=latest.frame.cursor, left=grid.x(col)/grid.ratio, top=grid.y(row)/grid.ratio;
    const height=(grid.y(row+1)-grid.y(row))/grid.ratio;
    const fontSize=height*latest.settings.font_size/24;
    const font=fontFamily(latest.settings.cjk_font,latest.settings.ascii_font);
    Object.assign(keyboard.style,{left:left+"px",top:top+"px",height:height+"px",lineHeight:height+"px",
        width:"1px",fontSize:fontSize+"px",fontFamily:font});
    if(!composing) return;
    ctx.save();ctx.font=fontSize+"px "+font;
    const textWidth=ctx.measureText(keyboard.value).width;ctx.restore();
    const cellWidth=(grid.x(col+1)-grid.x(col))/grid.ratio;
    const glyph=latest.frame.glyphs.find(g=>g.row===row&&g.col<=col&&col<g.col+g.width);
    const [color,background]=glyph?colours(col===glyph.col?glyph.left:glyph.right):[palette[7],palette[0]];
    Object.assign(keyboard.style,{width:Math.max(1,Math.min(textWidth+cellWidth,surface.clientWidth-left))+"px",
        color,background});
}
function focus() { if(document.activeElement!==keyboard) keyboard.focus({preventScroll:true}); }
function restoreEditingFocus(){keyboard.blur();focus();}
function pointerdown(e) {
    if(stationMouse.down(e))return;
    if(e.button!==0||!e.isPrimary||dragging) return;
    released=null;
    if(blocked()||latest?.output_pending) return;
    policy("reset",e);
    const bounds=viewport.getBoundingClientRect();
    if(e.clientX>=bounds.left+viewport.clientWidth || e.clientY>=bounds.top+viewport.clientHeight) return;
    // Prevent the scroll container's default focus step from overriding ours.
    e.preventDefault(); focus(); clearSelection();
    const point=cellAt(e,false);
    const hit=hitAt(point);
    if(hit?.kind!=="link"||hoverCandidate?.index!==hit.index)cancelHover();
    else if(!hoverCandidate.started)cancelHover();
    if(point&&!composing) {
        dragging={anchor:point,x:e.clientX,y:e.clientY,moved:false,pointer:e.pointerId,hit:hitAt(point),stamp:stamp()};
        viewport.setPointerCapture(e.pointerId);
        surface.dataset.pointerId=String(e.pointerId);surface.dataset.pointerActive="true";
        refreshKeepAlive();
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
    pointerClient={x:e.clientX,y:e.clientY,clientX:e.clientX,clientY:e.clientY,trusted:e.isTrusted};media.locate(pointerClient,hoverPreference);
    if(dragging&&e.pointerId!==dragging.pointer) return;
    if(!dragging) {if(stationMouse.local())setHover(cellAt(e,false));else {cancelHover();setHover(null);}return;}
    dragging.moved ||= Math.hypot(e.clientX-dragging.x,e.clientY-dragging.y)>3;
    if(dragging.moved)cancelHover();
    if(dragging.moved) requestSelection({mode:"range",anchor:dragging.anchor,focus:cellAt(e,true)});
}
function pointerup(e) {
    if(!dragging&&stationMouse.up(e))return;
    if(!dragging||e.pointerId!==dragging.pointer) return;
    pointermove(e);
    const hit=hitAt(cellAt(e,false));
    if(!dragging.moved&&!e.shiftKey&&stationMouse.local()&&!blocked()&&sameStamp(dragging.stamp,stamp())&&JSON.stringify(dragging.hit)===JSON.stringify(hit)) {
        released={hit,stamp:dragging.stamp};
    }
    dragging=null;
    surface.dataset.pointerActive="false";
    if(viewport.hasPointerCapture(e.pointerId)) viewport.releasePointerCapture(e.pointerId);
    refreshKeepAlive();
}
function click(e) {
    if(!stationMouse.local()){released=null;return;}
    if(e.button!==0||e.detail>1) {released=null;refreshKeepAlive();return;}
    const action=released;released=null;clickSequenceAction=!!action?.hit;
    if(!action?.hit||blocked()||!sameStamp(action.stamp,stamp())) {refreshKeepAlive();return;}
    if(action.hit.kind==="link") send({kind:"open_link",presentation:latest.presentation,generation,revision,index:action.hit.index,trusted:e.isTrusted});
    else navigate({action:"open",row:action.hit.row},e);
    refreshKeepAlive();
}
function cancelDrag() {
    if(dragging&&viewport.hasPointerCapture(dragging.pointer)) viewport.releasePointerCapture(dragging.pointer);
    dragging=null;
    surface.dataset.pointerActive="false";
    refreshKeepAlive();
}
function pointercancel(){stationMouse.cancel();cancelHover(true);released=null;cancelDrag();clearSelection();}
function deactivate(e){stationMouse.revoke();cancelHover(true);policy("reset",e);released=null;cancelDrag();setHover(null);cancelComposition();}
function lostcapture(e){stationMouse.lost(e);if(dragging){cancelHover();dragging=null;released=null;surface.dataset.pointerActive="false";refreshKeepAlive();}}
function hasSelection(){return selectionActive;}
function clearSelection() {
    if(!stopped)revokeSurfaceSelection(send,selectionCache,selectionBusy);
    selectionActive=false;selectionFingerprint=null;selectionNext=null;selectionCache=null;selectionLayer.replaceChildren();
    surfaceNote.textContent="";
    delete surface.dataset.selectionReady;
    refreshExportControls();
}
function requestSelection(selection) {
    if(latest?.output_pending)return;
    cancelHover(true);
    setHover(null);
    const fingerprint=JSON.stringify({generation,revision,presentation:latest?.presentation,selection});
    if(fingerprint===selectionFingerprint) return;
    if(!stopped)revokeSurfaceSelection(send,selectionCache,selectionBusy);
    selectionFingerprint=fingerprint;selectionActive=true;
    selectionNext={presentation:latest.presentation,generation,revision,selection}; selectionCache=null;
    delete surface.dataset.selectionReady;flushSelection();
    refreshExportControls();
}
function flushSelection() {
    if(selectionBusy||!selectionNext) return;
    selectionBusy={...selectionNext,request:++selectionSequence};selectionNext=null;
    send({kind:"select",...selectionBusy});
}
function receiveSelection(reply) {
    if(!selectionBusy||selectionBusy.request!==reply.request) return;
    selectionBusy=null;
    if(selectionActive&&!latest?.output_pending&&!selectionNext&&reply.generation===generation&&reply.revision===revision&&reply.presentation===latest?.presentation&&!filterUi.blocked()&&reply.value) {
        selectionCache=reply;surfaceNote.textContent="";surface.dataset.selectionReady=String(reply.request);drawSelection();
    }
    refreshExportControls();
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
    if(!selectionCache||latest?.output_pending||filterUi.blocked()||selectionCache.presentation!==latest?.presentation) {surfaceNote.textContent=tr("copy-not-ready");return;}
    if(!e.clipboardData) {surfaceNote.textContent=tr("copy-event-unavailable");return;}
    try {
        e.clipboardData.setData("text/plain",selectionCache.value.text);
        const {presentation,generation,revision,request}=selectionCache;
        send({kind:"copied",presentation,generation,revision,request,trusted:e.isTrusted});
    } catch(_) { surfaceNote.textContent=tr("copy-failed"); }
}
function doubleclick(e) {
    if(clickSequenceAction||blocked()||!stationMouse.local()&&!e.shiftKey) return;
    const point=cellAt(e,false);if(point) requestSelection({mode:"line",row:point.row});
}
function selectPage(){stationMouse.revoke();if(latest&&!composing){requestSelection({mode:"all"});focus();}}
function linkAt(point) {
    if(!point||!latest) return -1;
    return latest.links.findIndex(link=>link.row===point.row&&link.start<=point.col&&point.col<link.end);
}
function hitAt(point) {
    if(!point||blocked()||dragging?.moved||!stationMouse.local()) return null;
    const index=linkAt(point);if(index>=0) return {kind:"link",index};
    if(!canNavigate()||latest.navigation.kind!=="article_list") return null;
    const article=latest.navigation.articles.find(a=>a.row===point.row&&a.start<=point.col&&point.col<a.end);
    return article?{kind:"article",row:article.row}:null;
}
function setHover(point,redraw=true,preview=true) {
    const hit=hitAt(point),index=hit?.kind==="link"?hit.index:-1;
    const changed=index!==hoveredLink;hoveredLink=index;
    hoveredArticle=hit?.kind==="article"?latest.navigation.articles.find(a=>a.row===hit.row):null;
    canvas.style.cursor=hit?"pointer":"default";
    canvas.title=index>=0?tr("link-hover-hint",{url:latest.links[index].url}):hoveredArticle?tr("article-open-hint"):"";
    if(preview)previewHover(index);
    if(changed&&redraw&&latest) draw(latest);else drawNavigationMarker();
}
function drawNavigationMarker() {
    navigationMarker.hidden=!hoveredArticle||!grid||!canNavigate();
    if(navigationMarker.hidden) {delete navigationMarker.dataset.row;return;}
    const {row,start,end}=hoveredArticle;navigationMarker.dataset.row=String(row);
    Object.assign(navigationMarker.style,{left:grid.x(start)/grid.ratio+"px",top:grid.y(row)/grid.ratio+"px",
        width:(grid.x(end)-grid.x(start))/grid.ratio+"px",height:(grid.y(row+1)-grid.y(row))/grid.ratio+"px"});
}
function refreshNavigation() {
    refreshExportControls();
    const context=latest?.navigation,ready=canNavigate();
    surface.dataset.navigation=context?.kind||"unknown";surface.dataset.navigationReady=String(ready);
    surface.dataset.readingFirst=String(context?.first_line??0);
    browseControls.hidden=context?.kind!=="article_reading";
    browseBack.disabled=!ready||context?.kind!=="article_reading";
    browsePrevious.disabled=browseBack.disabled||context.first_line===1;
    browseNext.disabled=browseBack.disabled||context.percent===100;
    browseHint.textContent=!latest?.connected?"":latest.input_pending||navigationLocked||navigationRequest?tr("navigation-wait"):
        context?.kind==="article_list"?tr("article-list-hint"):context?.kind==="article_reading"?tr("article-reading-hint"):"";
    drawNavigationMarker();
}
function navigate(intent,e,source="pointer") {
    if(source==="pointer"&&!stationMouse.local())return;
    if(!canNavigate()||dragging?.moved) return;
    stationMouse.revoke();policy("reset",e);
    if(intent.action==="previous_page"&&latest.navigation.first_line===1||intent.action==="next_page"&&latest.navigation.percent===100) return;
    clearSelection();navigationRequest=true;navigationLocked=true;setHover(null);refreshNavigation();
    surface.dataset.navigationResult="pending";send({kind:"navigate",stamp:stamp(),intent,source,trusted:e.isTrusted});focus();
}
const browseReturn=e=>navigate({action:"back"},e,"button"),browseUp=e=>navigate({action:"previous_page"},e,"button"),browseDown=e=>navigate({action:"next_page"},e,"button");
const leaveLink=()=>{if(!dragging){cancelHover(true);setHover(null);}};
function selectLink(e) {
    if(!stationMouse.local()&&!e.shiftKey){e.preventDefault();return;}
    const index=linkAt(cellAt(e,false));
    if(index<0||composing) return;
    e.preventDefault();cancelDrag();focus();
    requestSelection({mode:"link",index});
}
function activate() { if(!document.querySelector("dialog[open]")&&!chrome.contains(document.activeElement)) focus(); }
function paste(e) {
    if(!e.isTrusted||stopped)return;
    stationMouse.revoke();
    cancelHover(true);
    e.preventDefault();
    if(pastePending||pasteDialog.open) return;
    if(keyboardState.enabled&&keyboardState.mode!=="input") {policy("reset",e);surfaceNote.textContent=tr("paste-native-first");return;}
    const text=e.clipboardData?.getData("text/plain");
    cancelComposition();cancelDrag();clearSelection();
    if(text===undefined) {surfaceNote.textContent=tr("paste-unavailable");return;}
    if(text.length>16384) {surfaceNote.textContent=tr("paste-too-long");return;}
    pastePending=true;surface.dataset.pasteResult="pending";sendInput({kind:"paste",text,generation,trusted:e.isTrusted});
}
function confirmPaste(e){if(e.isTrusted&&!stopped)sendInput({kind:"paste_confirm",trusted:true});}
function cancelPaste(e){if(!e.isTrusted||stopped)return;e.preventDefault();send({kind:"paste_cancel",trusted:true});}
const pasteObserver=new MutationObserver(()=>{
    if(pasteDialog.dataset.active==="true") {
        cancelComposition();cancelDrag();clearSelection();
        if(!pasteDialog.open) pasteDialog.showModal();
        if(pasteConfirmProbe){
            pasteConfirmProbe=false;
            const before=sent;pasteSend.click();
            const clean=sent===before&&pasteDialog.open&&pastePending;
            send({kind:"paste_confirm",trusted:false});
            send({kind:"surface_probe",clean});
        }
        pasteCancel.focus();
        refreshNavigation();
    } else if(pasteDialog.open) {
        pasteDialog.close();pastePending=false;
        // Restore the editing host, not just the DOM activeElement, after a modal.
        restoreEditingFocus();refreshNavigation();
    }
});
pasteObserver.observe(pasteDialog,{attributes:true,attributeFilter:["data-active"]});
function refreshExportControls() {
    refreshKeepAlive();
    const selection=selectionCache;
    exportButton.disabled=exportBusy||!latest||latest.output_pending||filterUi.blocked();
    exportRows.disabled=exportBusy||latest?.output_pending||filterUi.blocked()||blocked()||!selection||selection.generation!==generation||selection.revision!==revision||selection.presentation!==latest?.presentation;
    exportRows.title=selection?tr("export-lines-hint",{start:selection.value.rows.start+1,end:selection.value.rows.end}):tr("export-rows-hint");
}
function refreshKeepAlive() {
    const ready=!!latest?.keep_alive&&canNavigate()&&!stationMouse.active()&&!dragging&&!released&&!hasSelection()&&!exportBusy&&!keyboardState.pending;
    const current=stamp(),key=JSON.stringify([current,ready,latest?.keep_alive_ready,latest?.keep_alive]);
    keepAlive.textContent=latest?.keep_alive?tr("keepalive-on"):tr("keepalive-off");
    keepAlive.setAttribute("aria-pressed",String(!!latest?.keep_alive));
    keepAlive.title=latest?.keep_alive_failure||(!latest?.keep_alive?tr("keepalive-disabled-help"):
        ready?tr("keepalive-ready-help"):tr("keepalive-wait-help"));
    if(!stopped&&latest&&key!==keepAliveGate) {keepAliveGate=key;send({kind:"keep_alive_gate",stamp:current,ready});}
}
function toggleKeepAlive(e){if(latest&&!stopped)send({kind:"keep_alive",enabled:!latest.keep_alive,trusted:e.isTrusted});}
function exportScreen(e, selected=false) {
    if(exportBusy||!latest||stopped||filterUi.blocked()) return;
    const selection=selected?selectionCache:null;
    if(selected&&exportRows.disabled) return;
    exportBusy=true;refreshExportControls();surface.dataset.exportResult="pending";
    const packet=latest;
    exportTask=(async()=>{
        const full=document.createElement("canvas");full.width=960;full.height=576;
        const target=selection?document.createElement("canvas"):full;
        try {
            render(full,packet,{cursor:false,hovered:-1});
            if(selection) {
                const {start,end}=selection.value.rows;
                target.width=960;target.height=(end-start)*24;
                target.getContext("2d").drawImage(full,0,start*24,960,target.height,0,0,960,target.height);
            }
            const blob=await new Promise((resolve,reject)=>target.toBlob(value=>value?resolve(value):reject(new Error(tr("export-encode-failed"))),"image/png"));
            if(blob.size>3*1024*1024) throw new Error(tr("export-too-large"));
            const png=await new Promise((resolve,reject)=>{
                const reader=new FileReader();reader.onload=()=>resolve(reader.result);reader.onerror=()=>reject(new Error(tr("export-prepare-failed")));reader.readAsDataURL(blob);
            });
            if(!stopped) send({kind:"export_png",presentation:packet.presentation,generation:packet.generation,revision:packet.frame.revision,
                selection_request:selection?.request??null,png,trusted:e.isTrusted});
        } catch(error) {
            exportBusy=false;refreshExportControls();
            if(!stopped) {surface.dataset.exportResult="failed";surfaceNote.textContent=String(error);restoreEditingFocus();}
        } finally {target.width=0;target.height=0;full.width=0;full.height=0;}
    })();
}
const exportSelected=e=>exportScreen(e,true);
function showChrome() { chrome.classList.add("open"); }
function hideChrome() { chrome.classList.remove("open"); }
function zoomTo(value) {
    if(stopped||!latest) return;
    stationMouse.revoke();
    zoom=Math.max(.25,Math.min(4,value)); draw(latest);
    viewport.scrollLeft=Math.max(0,(surface.offsetWidth-viewport.clientWidth)/2);
    viewport.scrollTop=Math.max(0,(surface.offsetHeight-viewport.clientHeight)/2); focus();
}
function wheel(e) {
    cancelHover(true);
    if(!e.deltaY||e.altKey||e.metaKey||e.shiftKey) return;
    if(e.ctrlKey) {e.preventDefault();zoomTo(zoom+(e.deltaY<0?.25:-.25));return;}
    if(!stationMouse.local()||blocked()||dragging||hasSelection()||viewport.scrollWidth>viewport.clientWidth+1||viewport.scrollHeight>viewport.clientHeight+1) return;
    const kind=latest?.navigation.kind;
    if(!latest?.connected||!['article_list','article_reading'].includes(kind)) return;
    e.preventDefault();
    const action=kind==="article_reading"?(e.deltaY<0?"previous_page":"next_page"):(e.deltaY<0?"previous_item":"next_item");
    navigate({action},e);
}
const zoomIn=()=>zoomTo(zoom+.25), zoomOut=()=>zoomTo(zoom-.25), zoomFit=()=>zoomTo(1);
const media=installMedia({tr,send,prepare:()=>{stationMouse.revoke();cancelHover(true);cancelComposition();cancelDrag();clearSelection();released=null;setHover(null);},changed:refreshNavigation,restore:restoreEditingFocus});
function refreshControls(){refreshNavigation();refreshExportControls();}
const settingsUi=createSettingsDialog({send,
    beforeRequest:e=>{stationMouse.revoke();cancelHover(true);policy("reset",e);},beforeShow:invalidatePresentation,
    restore:restoreEditingFocus,changed:refreshControls,tr,loadFonts,fontLoadNote,invalidateFonts,render});
const loginUi=createLoginDialog({tr,send,stamp,
    prepare:e=>{stationMouse.revoke();cancelHover(true);policy("reset",e);cancelComposition();clearSelection();},
    restore:restoreEditingFocus,changed:refreshControls,focus});
const destinationUi=createNavigationDialog({tr,send,stamp,prepare:e=>{policy("reset",e);invalidatePresentation();},restore:restoreEditingFocus,changed:refreshControls,current:()=>latest,isEditing:()=>composing||pastePending});
const bookmarksUi=createBookmarksDialog({tr,send,prepare:e=>{policy("reset",e);invalidatePresentation();},restore:restoreEditingFocus,changed:refreshControls,openDestination:destinationUi.openDraft,current:()=>latest});
const filterUi=createFiltersDialog({tr,send,current:()=>latest,invalidate:invalidatePresentation,
    reset:e=>policy("reset",e),restore:restoreEditingFocus,changed:refreshControls});
const bindings=[...settingsUi.bindings,...loginUi.bindings,...filterUi.bindings,...destinationUi.bindings,...bookmarksUi.bindings,
    [keyboard,"keydown",keydown],[keyboard,"keyup",keyup],[keyboard,"input",input],
    [vimToggle,"click",toggleVim],[vimOnce,"click",armNative],
    [keyboard,"beforeinput",beforeinput],
    [keyboard,"blur",cancelComposition],
    [keyboard,"compositionstart",compositionstart],[keyboard,"compositionend",compositionend],
    [keyboard,"paste",paste],[viewport,"pointerdown",pointerdown],
    [viewport,"pointermove",pointermove],[viewport,"pointerup",pointerup],
    [viewport,"click",click],[viewport,"dblclick",doubleclick],
    [viewport,"pointercancel",pointercancel],[viewport,"lostpointercapture",lostcapture],
    [selectAll,"click",selectPage],[document,"copy",copy],
    [canvas,"contextmenu",selectLink],[canvas,"pointerleave",leaveLink],
    [pasteSend,"click",confirmPaste],[pasteCancel,"click",cancelPaste],[pasteDialog,"cancel",cancelPaste],
    [exportButton,"click",exportScreen],[exportRows,"click",exportSelected],
    [keepAlive,"click",toggleKeepAlive],[stationMouseButton,"click",toggleStationMouse],
    [hoverPosition,"change",changeHoverPreference],
    [browseBack,"click",browseReturn],[browsePrevious,"click",browseUp],[browseNext,"click",browseDown],
    [chrome,"pointerenter",showChrome],[chrome,"pointerleave",hideChrome],
    [plus,"click",zoomIn],[minus,"click",zoomOut],[fit,"click",zoomFit],
    [viewport,"wheel",wheel,{passive:false}],[window,"resize",repaint],[window,"focus",activate],[window,"blur",deactivate],
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
    positionKeyboard();drawSelection();drawNavigationMarker();draws++;return grid;
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
        ctx.font=fontDescription(packet.settings,glyph.width===2,scale);
        const left=x(glyph.col),top=y(glyph.row),span=x(glyph.col+glyph.width)-left,h=y(glyph.row+1)-top;
        const advance=Math.max(ctx.measureText(glyph.text).width,1);
        // Text keeps the font's aspect ratio; narrow symbols are centred instead
        // of stretched into ovals. Connected box/shade glyphs retain grid fitting.
        const code=glyph.text.codePointAt(0),connected=[...glyph.text].length===1&&code>=0x2500&&code<=0x2593;
        const fitting=connected?span/advance:Math.min(1,span/advance);
        const origin=connected?left:left+(span-advance*fitting)/2;
        for(let half=0;half<glyph.width;half++) {
            const [fg]=colours(half===0?glyph.left:glyph.right),col=glyph.col+half;
            ctx.save(); ctx.beginPath(); ctx.rect(x(col),top,x(col+1)-x(col),h); ctx.clip(); ctx.fillStyle=fg;
            if(!graphic(ctx,glyph.text,left,top,span,h)) {
                ctx.translate(origin,top+packet.settings.baseline*scale); ctx.scale(fitting,connected?1:fitting); ctx.fillText(glyph.text,0,0);
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
    animation=requestAnimationFrame(()=>{animation=0;if(!stopped) {draw(latest);if(pointerClient)setHover(cellAt(pointerClient,false),false);}});
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
    if(stopped)return;
    const presentationChanged=packet.presentation!==latest?.presentation;
    const changed=presentationChanged||!sameStamp(stamp(),{generation:packet.generation,revision:packet.frame.revision,input_epoch:packet.input_epoch});
    if(changed||packet.output_pending||latest?.input_pending&&!packet.input_pending) {released=null;clearSelection();cancelDrag();setHover(null,false,false);navigationLocked=false;}
    if(packet.output_pending)cancelHover();
    if(packet.mouse?.modes.epoch!==latest?.mouse?.modes.epoch||packet.mouse?.enabled!==latest?.mouse?.enabled){released=null;cancelDrag();cancelHover();setHover(null,false,false);}
    if(packet.generation!==generation){
        pastePending=false;cancelComposition();focus();
    }
    const previousFailure=latest?.keep_alive_failure;
    const failureChanged=packet.keep_alive_failure!==previousFailure;
    stationMouse.update(packet);
    generation=packet.generation;revision=packet.frame.revision;latest=packet;filterUi.paint(packet);refreshStationMouse();
    surface.dataset.presentation=String(packet.presentation);surface.dataset.maskedPushes=String(packet.masked_pushes);surface.dataset.revealed=String(packet.revealed);
    if(changed&&hoverCandidate) {
        const lease=packet.hover,candidate=hoverCandidate;
        if(!presentationChanged&&candidate.started&&lease?.sequence===candidate.sequence&&sameStamp(lease.stamp,stamp())&&JSON.stringify(lease.link)===candidate.identity&&linkAt(cellAt(pointerClient,false))===lease.index) {
            candidate.stamp=lease.stamp;candidate.index=lease.index;
        } else cancelHover();
    }
    if(pointerClient)setHover(cellAt(pointerClient,false),false);
    if(failureChanged) {
        if(packet.keep_alive_failure) surfaceNote.textContent=packet.keep_alive_failure;
        else if(surfaceNote.textContent===previousFailure) surfaceNote.textContent="";
    }
    refreshNavigation();
    const geometry=draw(packet);send({kind:"painted",presentation:packet.presentation,revision,generation});if(packet.probe) probe(packet,geometry);
    surface.dataset.revision=String(revision);
    surface.dataset.fontSize=String(packet.settings.font_size);surface.dataset.baseline=String(packet.settings.baseline);
    surface.dataset.cursorRow=String(packet.frame.cursor[1]);surface.dataset.inputEpoch=String(packet.input_epoch);
    surface.dataset.outputPending=String(!!packet.output_pending);
    surface.dataset.keepAlive=String(packet.keep_alive);surface.dataset.keepAliveReady=String(packet.keep_alive_ready);
    surface.dataset.keepAliveSent=String(packet.keep_alive_sent);
    loginUi.update(packet);destinationUi.update(packet);bookmarksUi.update();
}
focus();
function revokeSurface(){
    stopped=true;stationMouse.destroy();settingsUi.revoke();loginUi.revoke();filterUi.revoke();destinationUi.revoke();bookmarksUi.revoke();
    for(const [target,event,callback,options] of bindings)target.removeEventListener(event,callback,options);
}
const lifetime=createSurfaceLifetime(dioxus,window,revokeSurface);
let cleanupClean=false;
function runSurfaceProbe(mode){
            if(mode==="decode")send({kind:"synthetic_invalid_event"});
            else if(mode==="painted")send({kind:"painted",presentation:latest.presentation,revision:revision+100,generation});
            else if(mode==="renderer_error")throw new Error("synthetic renderer failure");
            else if(mode==="slow")surfaceNote.textContent="慢命令探針：結果等待 60 秒；可開啟字型設定或立即關閉視窗";
            else if(mode==="files")surfaceNote.textContent="原生選檔探針（本機合成資料）：可匯出 PNG 或匯入／匯出設定檔";
            else if(mode==="files_slow")surfaceNote.textContent="慢檔案探針：等待本機 runner 釋放；可開啟設定或關閉視窗";
            else if(mode==="fonts")surfaceNote.textContent="Font selection probe: local synthetic data and external-page sink";
            else if(mode==="recent")surfaceNote.textContent="Recent-board probe: inspect the restored list, clear it, wait for saved feedback, then close";
            else if(mode==="bookmarks")surfaceNote.textContent="Bookmark probe: add two entries, rename the first, remove the second, reject a bare board, then open #00000101@Stella through Go to";
            else if(mode.startsWith("bookmarks_"))surfaceNote.textContent="Bookmark restart probe: inspect restored/empty/corrupt data, remove restored entry explicitly, then close without station input";
            else if(mode==="articles"||mode==="articles_close")surfaceNote.textContent="Article probe: enable Vim, open URL 00000101 and AID 00000102, fail 00000103 and recover manually, reject foreign URL and long draft, then cancel/close during 00000104";
            else if(mode==="boards_close")surfaceNote.textContent="Board close probe: visit Stella and Other, start Beta, then close while waiting";
            else if(mode==="boards")surfaceNote.textContent="Board probe: cancel a draft, visit Stella and Other, reject an invalid name, then start Beta and stop before the delayed board list";
            else if(mode==="osc8")surfaceNote.textContent="OSC 8 probe: both wrapped fragments share a complete destination; select/copy, drag and open local media";
            else if(mode==="login_2fa")surfaceNote.textContent="Verification probe: explicit synthetic login, six digits, then eight-character recovery; Esc returns to Vim";
            else if(mode==="sgr66"||mode==="sgr66_legacy")surfaceNote.textContent="Dual-color probe: export PNG, then close; local synthetic data only";
            else if(mode==="dialogs")surfaceNote.textContent="對話框與媒體封包探針（僅本機合成資料）";
            else if(mode==="confirm"){pasteConfirmProbe=true;surfaceNote.textContent="貼上確認探針：請用原生貼上 A、換行、B";}
            else if(mode==="untrusted"){
                const before=sent,beforePending=pastePending,beforeDialog=pasteDialog.open;
                const clipboard=new DataTransfer();clipboard.setData("text/plain","synthetic");
                const event=new ClipboardEvent("paste",{clipboardData:clipboard,bubbles:true,cancelable:true});
                keyboard.dispatchEvent(event);pasteSend.click();pasteCancel.click();
                const clean=sent===before&&pastePending===beforePending&&pasteDialog.open===beforeDialog&&!event.defaultPrevented;
                // These are deliberately untrusted wire fixtures, not evidence
                // of native trusted input. The local peer must receive no bytes.
                send({kind:"paste",text:"synthetic",generation,trusted:false});
                send({kind:"paste",text:"A\nB",generation,trusted:false});
                send({kind:"paste_confirm",trusted:false});
                send({kind:"surface_probe",clean});
            }

}
function showNotice(message){if(!stopped&&message&&!settingsUi.notice(message))surfaceNote.textContent=message;}
const route=createSurfaceRouter({
    paint,
    station_mouse_rejected:data=>{stationMouse.reject(data.lease);showNotice(data.message);},
    surface_probe:runSurfaceProbe,
    filters_open:filterUi.open,filters_result:filterUi.receive,
    login_result:loginUi.receive,
    recent_boards:destinationUi.receiveRecent,destination_result:destinationUi.receive,keyboard_state:receiveKeyboard,
    bookmarks_open:bookmarksUi.receiveOpen,bookmarks_result:bookmarksUi.receive,
    hover_settings:position=>{hoverPreference=position;hoverPosition.value=position;surface.dataset.hoverPosition=position;
        if(position==="off")cancelHover(true);else if(pointerClient)media.locate(pointerClient,position);},
    media_hover:data=>{if(hoverCandidate?.sequence===data.sequence){media.hover(data);media.locate(pointerClient,hoverPreference);}},
    hover_reset:data=>{cancelHover();if(data.media)media.reset();},
    hover_rejected:sequence=>{if(hoverCandidate?.sequence===sequence)cancelHover();},
    media_open:data=>media.open(data),media_image:data=>media.receiveImage(data),media_error:data=>media.fail(data),
    media_external:data=>media.externalResult(data),media_reset:()=>media.reset(),
    settings_open:settingsUi.open,font_catalog:settingsUi.catalog,font_preview:settingsUi.preview,
    profile_result:async data=>{await settingsUi.profile(data);showNotice(data.notice);},
    settings_saved:data=>{settingsUi.saved(data);showNotice(data.notice);},
    selection:receiveSelection,
    paste_result:data=>{
        surface.dataset.pasteResult=data.status;pastePending=data.status==="preview";
        if(["cancelled","rejected"].includes(data.status))navigationLocked=false;
        refreshNavigation();showNotice(data.notice);
    },
    input_rejected:message=>{navigationLocked=false;refreshNavigation();showNotice(message);},
    navigation_result:data=>{
        navigationRequest=false;surface.dataset.navigationResult=data.status;
        if(data.status==="rejected")navigationLocked=false;
        refreshNavigation();showNotice(data.notice);
    },
    link_result:data=>{surface.dataset.linkResult=data.status;showNotice(data.notice);},
    export_result:data=>{surface.dataset.exportResult=data.status;exportBusy=false;refreshExportControls();restoreEditingFocus();showNotice(data.notice);},
    notice:showNotice,
});
try {
    while(true){
        const packet=await lifetime.receive();
        if(packet?.kind==="stop")break;
        await route(packet);
    }
} catch(e){send({kind:"error",message:String(e)});}
finally {
    lifetime.release();
    revokeSurface();
    cancelHover(true);
    stopped=true;latest=null;observer.disconnect();pasteObserver.disconnect();if(animation){cancelAnimationFrame(animation);animation=0;}
    const mediaClean=await media.destroy();
    settingsUi.destroy();loginUi.destroy();filterUi.destroy();destinationUi.destroy();bookmarksUi.destroy();
    if(exportTask)await exportTask;
    if(pasteDialog.open)pasteDialog.close();
    setHover(null);refreshNavigation();
    cancelComposition();keyboard.blur();hideChrome();cancelDrag();selectionBusy=null;clearSelection();
    const before=sent,beforeDraws=draws,beforeZoom=zoom;
    keyboard.dispatchEvent(new KeyboardEvent("keydown",{key:"ArrowUp",bubbles:true}));
    keyboard.dispatchEvent(new InputEvent("input",{data:"a",inputType:"insertText",bubbles:true}));
    surface.dispatchEvent(new PointerEvent("pointerdown",{bubbles:true}));window.dispatchEvent(new Event("resize"));plus.click();
    chrome.dispatchEvent(new PointerEvent("pointerenter"));
    document.getElementById("font-settings").click();document.getElementById("font-size").dispatchEvent(new InputEvent("input",{bubbles:true}));
    browseNext.click();exportRows.click();keepAlive.click();viewport.dispatchEvent(new WheelEvent("wheel",{deltaY:120,bubbles:true}));
    document.getElementById("media-close").click();document.getElementById("media-motion").click();
    cleanupClean=mediaClean&&stationMouse.clean()&&sent===before&&draws===beforeDraws&&zoom===beforeZoom&&animation===0
        &&settingsUi.clean()&&loginUi.clean()&&filterUi.clean()&&destinationUi.clean()&&bookmarksUi.clean()&&hoverTimer===0&&hoverCandidate===null&&!chrome.classList.contains("open")&&document.activeElement!==keyboard;
    try {send({kind:"stopped",clean:cleanupClean});} catch(_) { /* join still acknowledges cleanup */ }
}
return {clean:cleanupClean};
