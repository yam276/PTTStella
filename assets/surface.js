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
const hoverPosition=document.getElementById("hover-position");
let hoverPreference="top_right",hoverCandidate=null,hoverTimer=0,hoverSequence=0,pointerClient=null;
function cancelHover(forget=false) {
    clearTimeout(hoverTimer);hoverTimer=0;
    const candidate=hoverCandidate;hoverCandidate=null;
    if(forget)pointerClient=null;
    media.hideHover();
    if(candidate?.started&&!stopped)send({kind:"hover",sequence:candidate.sequence,stamp:candidate.stamp,index:null,trusted:true});
}
function previewHover(index) {
    if(!pointerClient||hoverPreference==="off"||blocked()||dragging||hasSelection()||!latest?.connected||!latest.image_links.includes(index)) {cancelHover();return;}
    const identity=JSON.stringify(latest.links[index]);
    if(hoverCandidate?.identity===identity) {media.locate(pointerClient,hoverPreference);return;}
    cancelHover();
    const candidate={identity,index,sequence:++hoverSequence,stamp:stamp(),started:false,trusted:pointerClient.trusted};hoverCandidate=candidate;
    hoverTimer=setTimeout(()=>{hoverTimer=0;
        if(stopped||hoverCandidate!==candidate||blocked()||dragging||!document.hasFocus()||!sameStamp(candidate.stamp,stamp()))return;
        candidate.started=true;send({kind:"hover",sequence:candidate.sequence,stamp:candidate.stamp,index:candidate.index,trusted:candidate.trusted});
    },350);
}
function changeHoverPreference(e){send({kind:"hover_preference",position:hoverPosition.value,trusted:e.isTrusted});}
const vimToggle=document.getElementById("vim-toggle"),vimOnce=document.getElementById("vim-once"),vimStatus=document.getElementById("vim-status");
let keyboardState={enabled:false,mode:"input",pending:false,prefix:""};
function policy(action,e,code="") {send({kind:"keyboard_state",action,code,generation,trusted:e?.isTrusted??true});}
function toggleVim(e){policy("toggle",e);restoreEditingFocus();}
function armNative(e){policy("once",e);restoreEditingFocus();}
function keyup(e){if(!e.metaKey)policy("release",e,e.code);}
function receiveKeyboard(state){keyboardState=state;vimToggle.textContent=state.enabled?"Vim：開":"Vim：關";
    keyboard.readOnly=state.enabled&&!["input","once"].includes(state.mode);
    vimToggle.setAttribute("aria-pressed",String(state.enabled));vimOnce.hidden=!state.enabled||state.mode==="input";
    vimOnce.setAttribute("aria-pressed",String(state.mode==="once"));
    const reason={unsupported_grid:"不支援的格線",incomplete:"畫面接收未完成",unmapped:"畫面有未映射字格",unrecognized:"未辨識的選單、列表或閱讀格式",invalid_cursor:"列表游標未確認",board_header:"看板標頭未確認",board_rows:"看板項目欄位未確認"}[state.suspended_reason]||"畫面未確認";
    vimStatus.textContent=!state.enabled?"":({normal:"VIM NORMAL",input:"PTT INPUT",once:"原生一次",suspended:"VIM 暫停 · "+reason+" · i 原始輸入"}[state.mode])+" "+state.prefix;
    surface.dataset.keyboardMode=state.mode;surface.dataset.vim=String(state.enabled);refreshKeepAlive();}
const navigationMarker=document.getElementById("navigation-marker"),browseControls=document.getElementById("browse-controls");
const browseBack=document.getElementById("browse-back"),browsePrevious=document.getElementById("browse-previous"),browseNext=document.getElementById("browse-next");
const browseHint=document.getElementById("browse-hint");
const fontButton=document.getElementById("font-settings"),fontDialog=document.getElementById("font-dialog");
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
const ctx=canvas.getContext("2d",{willReadFrequently:true});
const palette=["#000000","#800000","#008000","#808000","#000080","#800080","#008080","#c0c0c0",
    "#808080","#ff0000","#00ff00","#ffff00","#0000ff","#ff00ff","#00ffff","#ffffff"];
const bundledFonts=new Map([
    ["Cascadia Mono",{family:"PTTStella Cascadia Mono",file:"CascadiaMono.ttf"}],
    ["Noto Sans CJK TC",{family:"PTTStella Noto Sans CJK TC",file:"NotoSansCJKtc-Regular.otf"}]
]);
const bundledLoads=new Map();
const fontFamily=name=>JSON.stringify(bundledFonts.get(name)?.family??name)+", monospace";
const fontDescription=(settings,cjk=false)=>settings.font_size+"px "+fontFamily(cjk?settings.cjk_font:settings.ascii_font);
let loadedFonts=null,fontDraft=null,fontTimer=0,fontRequest=0,fontPreviewBusy=false,fontPreviewNext=null,fontSaving=false;
async function loadFonts(settings) {
    const key=JSON.stringify([settings.ascii_font,settings.cjk_font,settings.font_size]);
    if(loadedFonts?.key===key&&!loadedFonts.warning) return "";
    const missing=await Promise.all([["英數",settings.ascii_font],["中文",settings.cjk_font]].map(async([role,name])=>{
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
        } catch {return role+"字型「"+name+"」"+(bundled?"內建載入失敗":"無法由本機載入");}
    }));
    await Promise.all([document.fonts.load(fontDescription(settings),"M"),document.fonts.load(fontDescription(settings,true),"中")]);
    await document.fonts.ready;
    const warning=missing.filter(Boolean).join("；");
    loadedFonts={key,warning:warning?warning+"；目前使用系統替代字型。可改選「內建跨平台」，或安裝所選字型。":""};
    return loadedFonts.warning;
}
function fontLoadNote(settings,warning) {
    if(warning)return warning;
    const names=[...new Set([settings.ascii_font,settings.cjk_font])].filter(name=>bundledFonts.has(name));
    return names.length?"已載入 App 內建字型："+names.join("、")+"；無須另行安裝。":"本機字型已載入；請以預覽檢查字形。";
}
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
const canNavigate=()=>!stopped&&!!latest?.connected&&['article_list','article_reading'].includes(latest.navigation.kind)&&!latest.input_pending&&!navigationLocked&&!navigationRequest&&!blocked();
function invalidateNavigation(){navigationLocked=true;released=null;cancelDrag();setHover(null);refreshNavigation();}
function sendInput(value){invalidateNavigation();send(value);}
// Only classify editing vs command events here. Rust owns key translation and
// the context-dependent conversion from browsing intents to PTT bytes.
const commandKeys=new Set(["Enter","Backspace","Tab","Escape","ArrowUp","ArrowDown","ArrowRight","ArrowLeft","Home","End","PageUp","PageDown","Delete"]);
function keydown(e) {
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
    const font=fontFamily(latest.settings.cjk_font);
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
    if(e.button!==0||!e.isPrimary||dragging) return;
    released=null;
    if(blocked()) return;
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
    if(!dragging) {setHover(cellAt(e,false));return;}
    dragging.moved ||= Math.hypot(e.clientX-dragging.x,e.clientY-dragging.y)>3;
    if(dragging.moved)cancelHover();
    if(dragging.moved) requestSelection({mode:"range",anchor:dragging.anchor,focus:cellAt(e,true)});
}
function pointerup(e) {
    if(!dragging||e.pointerId!==dragging.pointer) return;
    pointermove(e);
    const hit=hitAt(cellAt(e,false));
    if(!dragging.moved&&!blocked()&&sameStamp(dragging.stamp,stamp())&&JSON.stringify(dragging.hit)===JSON.stringify(hit)) {
        released={hit,stamp:dragging.stamp};
    }
    dragging=null;
    surface.dataset.pointerActive="false";
    if(viewport.hasPointerCapture(e.pointerId)) viewport.releasePointerCapture(e.pointerId);
    refreshKeepAlive();
}
function click(e) {
    if(e.button!==0||e.detail>1) {released=null;refreshKeepAlive();return;}
    const action=released;released=null;clickSequenceAction=!!action?.hit;
    if(!action?.hit||blocked()||!sameStamp(action.stamp,stamp())) {refreshKeepAlive();return;}
    if(action.hit.kind==="link") send({kind:"open_link",generation,revision,index:action.hit.index,trusted:e.isTrusted});
    else navigate({action:"open",row:action.hit.row},e);
    refreshKeepAlive();
}
function cancelDrag() {
    if(dragging&&viewport.hasPointerCapture(dragging.pointer)) viewport.releasePointerCapture(dragging.pointer);
    dragging=null;
    surface.dataset.pointerActive="false";
    refreshKeepAlive();
}
function pointercancel(){cancelHover(true);released=null;cancelDrag();clearSelection();}
function deactivate(e){cancelHover(true);policy("reset",e);released=null;cancelDrag();setHover(null);cancelComposition();}
function lostcapture(){if(dragging){cancelHover();dragging=null;released=null;surface.dataset.pointerActive="false";refreshKeepAlive();}}
function hasSelection(){return selectionActive;}
function clearSelection() {
    selectionActive=false;selectionFingerprint=null;selectionNext=null;selectionCache=null;selectionLayer.replaceChildren();
    surfaceNote.textContent="";
    delete surface.dataset.selectionReady;
    refreshExportControls();
}
function requestSelection(selection) {
    cancelHover(true);
    setHover(null);
    const fingerprint=JSON.stringify({generation,revision,selection});
    if(fingerprint===selectionFingerprint) return;
    selectionFingerprint=fingerprint;selectionActive=true;
    selectionNext={generation,revision,selection}; selectionCache=null;
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
    if(selectionActive&&!selectionNext&&reply.generation===generation&&reply.revision===revision&&reply.value) {
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
    if(!selectionCache) {surfaceNote.textContent="選取尚未準備好，請再複製";return;}
    if(!e.clipboardData) {surfaceNote.textContent="此環境未提供剪貼簿事件資料";return;}
    try {
        e.clipboardData.setData("text/plain",selectionCache.value.text);
        const {generation,revision,request}=selectionCache;
        send({kind:"copied",generation,revision,request,trusted:e.isTrusted});
    } catch(_) { surfaceNote.textContent="無法寫入剪貼簿，請重試"; }
}
function doubleclick(e) {
    if(clickSequenceAction||blocked()) return;
    const point=cellAt(e,false);if(point) requestSelection({mode:"line",row:point.row});
}
function selectPage(){if(latest&&!composing){requestSelection({mode:"all"});focus();}}
function linkAt(point) {
    if(!point||!latest) return -1;
    return latest.links.findIndex(link=>link.row===point.row&&link.start<=point.col&&point.col<link.end);
}
function hitAt(point) {
    if(!point||blocked()||dragging?.moved) return null;
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
    canvas.title=index>=0?latest.links[index].url+"\n點擊開啟；右鍵選取後可複製":hoveredArticle?"點擊編號開啟文章；拖曳可選字":"";
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
    browseHint.textContent=!latest?.connected?"":latest.input_pending||navigationLocked||navigationRequest?"等待畫面更新":
        context?.kind==="article_list"?"點編號開啟 · 拖曳選字":context?.kind==="article_reading"?"滾輪翻頁 · Ctrl＋滾輪縮放":"";
    drawNavigationMarker();
}
function navigate(intent,e) {
    if(!canNavigate()||dragging?.moved) return;
    policy("reset",e);
    if(intent.action==="previous_page"&&latest.navigation.first_line===1||intent.action==="next_page"&&latest.navigation.percent===100) return;
    clearSelection();navigationRequest=true;navigationLocked=true;setHover(null);refreshNavigation();
    surface.dataset.navigationResult="pending";send({kind:"navigate",stamp:stamp(),intent,trusted:e.isTrusted});focus();
}
const browseReturn=e=>navigate({action:"back"},e),browseUp=e=>navigate({action:"previous_page"},e),browseDown=e=>navigate({action:"next_page"},e);
const leaveLink=()=>{if(!dragging){cancelHover(true);setHover(null);}};
function selectLink(e) {
    const index=linkAt(cellAt(e,false));
    if(index<0||composing) return;
    e.preventDefault();cancelDrag();focus();
    const link=latest.links[index];
    requestSelection({mode:"range",anchor:{col:link.start,row:link.row},focus:{col:link.end-1,row:link.row}});
}
function activate() { if(!document.querySelector("dialog[open]")&&!chrome.contains(document.activeElement)) focus(); }
function paste(e) {
    cancelHover(true);
    e.preventDefault();
    if(pastePending||pasteDialog.open) return;
    if(keyboardState.enabled&&keyboardState.mode!=="input") {policy("reset",e);surfaceNote.textContent="請先按 i 切換原始輸入再貼上";return;}
    const text=e.clipboardData?.getData("text/plain");
    cancelComposition();cancelDrag();clearSelection();
    if(text===undefined) {surfaceNote.textContent="此環境無法讀取貼上的文字";return;}
    if(text.length>16384) {surfaceNote.textContent="貼上文字過長；整段未送出";return;}
    pastePending=true;surface.dataset.pasteResult="pending";sendInput({kind:"paste",text,generation,trusted:e.isTrusted});
}
function confirmPaste(e){sendInput({kind:"paste_confirm",trusted:e.isTrusted});}
function cancelPaste(e){e.preventDefault();send({kind:"paste_cancel",trusted:e.isTrusted});}
function openFonts(e){cancelHover(true);policy("reset",e);send({kind:"settings_open",trusted:e.isTrusted});}
function fillFonts(values) {
    for(const [name,field] of Object.entries(fontFields)) field.value=values[name];
}
function readFonts() {
    const selected=fontPreset.value,fonts=selected==="custom"?{kind:"custom",ascii_font:fontFields.ascii_font.value,cjk_font:fontFields.cjk_font.value}:
        selected==="platform"?{kind:"platform_default"}:{kind:"preset",preset:selected};
    return {fonts,font_size:Number(fontFields.font_size.value),baseline:Number(fontFields.baseline.value)};
}
function flushFontPreview() {
    if(fontPreviewBusy||!fontPreviewNext||!fontDialog.open||stopped) return;
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
    fontDialog.close();restoreEditingFocus();
    refreshNavigation();
}
function saveFonts(e) {
    if(fontSaving||profileBusy) return;
    fontSaving=true;fontSave.disabled=true;fontCancel.disabled=true;
    send({kind:"settings_save",values:readFonts(),trusted:e.isTrusted});
}
const pasteObserver=new MutationObserver(()=>{
    if(pasteDialog.dataset.active==="true") {
        cancelComposition();cancelDrag();clearSelection();
        if(!pasteDialog.open) pasteDialog.showModal();
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
    exportButton.disabled=exportBusy||!latest;
    exportRows.disabled=exportBusy||blocked()||!selection||selection.generation!==generation||selection.revision!==revision;
    exportRows.title=selection?`匯出第 ${selection.value.rows.start+1}–${selection.value.rows.end} 行（完整 80 欄）`:"先拖曳選取要截圖的行";
}
function refreshKeepAlive() {
    const ready=!!latest?.keep_alive&&canNavigate()&&!dragging&&!released&&!hasSelection()&&!exportBusy&&!keyboardState.pending;
    const current=stamp(),key=JSON.stringify([current,ready,latest?.keep_alive_ready,latest?.keep_alive]);
    keepAlive.textContent=latest?.keep_alive?"防閒置：開":"防閒置：關";
    keepAlive.setAttribute("aria-pressed",String(!!latest?.keep_alive));
    keepAlive.title=latest?.keep_alive_failure||(!latest?.keep_alive?"本次啟動暫停防閒置；重新啟動預設開啟":
        ready?"閒置 60 秒後重繪畫面；不移動閱讀位置":"已開啟；等待可辨識畫面或本機操作結束");
    if(!stopped&&latest&&key!==keepAliveGate) {keepAliveGate=key;send({kind:"keep_alive_gate",stamp:current,ready});}
}
function toggleKeepAlive(e){if(latest&&!stopped)send({kind:"keep_alive",enabled:!latest.keep_alive,trusted:e.isTrusted});}
function exportScreen(e, selected=false) {
    if(exportBusy||!latest||stopped) return;
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
            const blob=await new Promise((resolve,reject)=>target.toBlob(value=>value?resolve(value):reject(new Error("無法編碼 PNG")),"image/png"));
            if(blob.size>3*1024*1024) throw new Error("PNG 超過匯出大小上限");
            const png=await new Promise((resolve,reject)=>{
                const reader=new FileReader();reader.onload=()=>resolve(reader.result);reader.onerror=()=>reject(new Error("無法準備 PNG"));reader.readAsDataURL(blob);
            });
            if(!stopped) send({kind:"export_png",generation:packet.generation,revision:packet.frame.revision,
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
    zoom=Math.max(.25,Math.min(4,value)); draw(latest);
    viewport.scrollLeft=Math.max(0,(surface.offsetWidth-viewport.clientWidth)/2);
    viewport.scrollTop=Math.max(0,(surface.offsetHeight-viewport.clientHeight)/2); focus();
}
function wheel(e) {
    cancelHover(true);
    if(!e.deltaY||e.altKey||e.metaKey||e.shiftKey) return;
    if(e.ctrlKey) {e.preventDefault();zoomTo(zoom+(e.deltaY<0?.25:-.25));return;}
    if(blocked()||dragging||hasSelection()||viewport.scrollWidth>viewport.clientWidth+1||viewport.scrollHeight>viewport.clientHeight+1) return;
    const kind=latest?.navigation.kind;
    if(!latest?.connected||!['article_list','article_reading'].includes(kind)) return;
    e.preventDefault();
    const action=kind==="article_reading"?(e.deltaY<0?"previous_page":"next_page"):(e.deltaY<0?"previous_item":"next_item");
    navigate({action},e);
}
const zoomIn=()=>zoomTo(zoom+.25), zoomOut=()=>zoomTo(zoom-.25), zoomFit=()=>zoomTo(1);
const media=installMedia({send,prepare:()=>{cancelHover(true);cancelComposition();cancelDrag();clearSelection();released=null;setHover(null);},changed:refreshNavigation,restore:restoreEditingFocus});
const bindings=[
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
    [keepAlive,"click",toggleKeepAlive],
    [hoverPosition,"change",changeHoverPreference],
    [browseBack,"click",browseReturn],[browsePrevious,"click",browseUp],[browseNext,"click",browseDown],
    [fontButton,"click",openFonts],[fontPreset,"change",chooseFontPreset],
    [profileImport,"click",importProfile],[profileExport,"click",exportProfile],
    ...Object.values(fontFields).map(field=>[field,"input",editFont]),
    [fontDefaults,"click",resetFonts],[fontCancel,"click",closeFonts],[fontDialog,"cancel",closeFonts],[fontSave,"click",saveFonts],
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
        ctx.font=(packet.settings.font_size*scale)+"px "+fontFamily(glyph.width===2?packet.settings.cjk_font:packet.settings.ascii_font);
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
    const changed=!sameStamp(stamp(),{generation:packet.generation,revision:packet.frame.revision,input_epoch:packet.input_epoch});
    if(changed||latest?.input_pending&&!packet.input_pending) {released=null;clearSelection();cancelDrag();setHover(null,false,false);navigationLocked=false;}
    if(packet.generation!==generation){
        pastePending=false;cancelComposition();focus();
    }
    const previousFailure=latest?.keep_alive_failure;
    const failureChanged=packet.keep_alive_failure!==previousFailure;
    generation=packet.generation;revision=packet.frame.revision;latest=packet;
    if(changed&&hoverCandidate) {
        const lease=packet.hover,candidate=hoverCandidate;
        if(candidate.started&&lease?.sequence===candidate.sequence&&sameStamp(lease.stamp,stamp())&&JSON.stringify(lease.link)===candidate.identity&&linkAt(cellAt(pointerClient,false))===lease.index) {
            candidate.stamp=lease.stamp;candidate.index=lease.index;
        } else cancelHover();
    }
    if(pointerClient)setHover(cellAt(pointerClient,false),false);
    if(failureChanged) {
        if(packet.keep_alive_failure) surfaceNote.textContent=packet.keep_alive_failure;
        else if(surfaceNote.textContent===previousFailure) surfaceNote.textContent="";
    }
    refreshNavigation();
    const geometry=draw(packet);send({kind:"painted",revision,generation});if(packet.probe) probe(packet,geometry);
    surface.dataset.revision=String(revision);
    surface.dataset.fontSize=String(packet.settings.font_size);surface.dataset.baseline=String(packet.settings.baseline);
    surface.dataset.cursorRow=String(packet.frame.cursor[1]);surface.dataset.inputEpoch=String(packet.input_epoch);
    surface.dataset.keepAlive=String(packet.keep_alive);surface.dataset.keepAliveReady=String(packet.keep_alive_ready);
    surface.dataset.keepAliveSent=String(packet.keep_alive_sent);
}
focus();
try {
    while(true){const packet=await dioxus.recv();if(packet.stop) break;
        if(packet.keyboard_state){receiveKeyboard(packet.keyboard_state);continue;}
        if(packet.hover_settings){hoverPreference=packet.hover_settings;hoverPosition.value=hoverPreference;
            surface.dataset.hoverPosition=hoverPreference;
            if(hoverPreference==="off")cancelHover(true);else if(pointerClient)media.locate(pointerClient,hoverPreference);continue;}
        if(packet.media_hover){if(hoverCandidate?.sequence===packet.media_hover.sequence){media.hover(packet.media_hover);media.locate(pointerClient,hoverPreference);}continue;}
        if(packet.hover_reset){cancelHover();continue;}
        if(packet.hover_rejected){if(hoverCandidate?.sequence===packet.hover_rejected)cancelHover();continue;}
        if(packet.media_open){media.open(packet.media_open);continue;}
        if(packet.media_image){media.receiveImage(packet.media_image);continue;}
        if(packet.media_error){media.fail(packet.media_error);continue;}
        if(packet.media_external){media.externalResult(packet.media_external);continue;}
        if(packet.media_reset){media.reset();continue;}
        if(packet.settings_open) {
            fontDraft=packet.settings_open;fillFonts(fontDraft.values);fontPreset.value=profileChoice(fontDraft.profile);
            fontNote.textContent=fontDraft.warning||"";cancelComposition();cancelDrag();clearSelection();
            if(!fontDialog.open) fontDialog.showModal();
            released=null;cancelDrag();setHover(null);refreshNavigation();
            fontAvailability.textContent=fontLoadNote(fontDraft.values,await loadFonts(fontDraft.values));render(fontPreview,fontDraft.sample,{cursor:false});
            fontFields.ascii_font.focus();continue;
        }
        if(packet.font_preview) {
            const preview=packet.font_preview;fontPreviewBusy=false;
            if(fontDialog.open&&preview.request===fontRequest) {
                if(preview.packet){fontAvailability.textContent=fontLoadNote(preview.packet.settings,await loadFonts(preview.packet.settings));render(fontPreview,preview.packet,{cursor:false});fontPreview.dataset.request=String(preview.request);
                    fontPreview.dataset.fontSize=String(preview.packet.settings.font_size);fontPreview.dataset.baseline=String(preview.packet.settings.baseline);}
                if(preview.error) fontNote.textContent=preview.error;
            }
            flushFontPreview();continue;
        }
        if(packet.profile_result) {
            profileBusy=false;profileImport.disabled=false;profileExport.disabled=false;fontDialog.dataset.profileResult=packet.profile_result;
            if(packet.profile_import&&fontDialog.open){const imported=packet.profile_import;clearTimeout(fontTimer);fontTimer=0;fontRequest++;fontPreviewNext=null;
                fillFonts(imported.values);fontPreset.value=profileChoice(imported.profile);profileName.value=imported.name;
                fontAvailability.textContent=fontLoadNote(imported.values,await loadFonts(imported.values));render(fontPreview,imported.sample,{cursor:false});}
            if(packet.notice)fontNote.textContent=packet.notice;
            if(fontDialog.open)profileName.focus();continue;
        }
        if(typeof packet.settings_saved==="boolean") {
            fontSaving=false;fontSave.disabled=false;fontCancel.disabled=false;
            if(packet.settings_saved) closeFonts();
        }
        if(packet.selection){receiveSelection(packet.selection);continue;}
        if(packet.paste_result) {surface.dataset.pasteResult=packet.paste_result;pastePending=packet.paste_result==="preview";
            if(["cancelled","rejected"].includes(packet.paste_result)) navigationLocked=false;refreshNavigation();}
        if(packet.input_result==="rejected") {navigationLocked=false;refreshNavigation();}
        if(packet.navigation_result) {navigationRequest=false;surface.dataset.navigationResult=packet.navigation_result;
            if(packet.navigation_result==="rejected") navigationLocked=false;refreshNavigation();}
        if(packet.link_result) surface.dataset.linkResult=packet.link_result;
        if(packet.export_result) {surface.dataset.exportResult=packet.export_result;exportBusy=false;refreshExportControls();restoreEditingFocus();}
        if(packet.notice) (fontDialog.open?fontNote:surfaceNote).textContent=packet.notice;
        if(packet.paste_result||packet.link_result||packet.export_result||packet.notice||packet.navigation_result||packet.input_result) continue;
        await paint(packet);
    }
} catch(e){send({kind:"error",message:String(e)});}
finally {
    cancelHover(true);
    stopped=true;latest=null;observer.disconnect();pasteObserver.disconnect();if(animation){cancelAnimationFrame(animation);animation=0;}
    const mediaClean=await media.destroy();
    clearTimeout(fontTimer);fontTimer=0;fontPreviewNext=null;
    for(const [target,event,callback,options] of bindings) target.removeEventListener(event,callback,options);
    if(exportTask) await exportTask;
    if(pasteDialog.open) pasteDialog.close();
    if(fontDialog.open) fontDialog.close();
    setHover(null);refreshNavigation();
    cancelComposition();keyboard.blur();hideChrome();cancelDrag();selectionBusy=null;clearSelection();
    const before=sent,beforeDraws=draws,beforeZoom=zoom;
    keyboard.dispatchEvent(new KeyboardEvent("keydown",{key:"ArrowUp",bubbles:true}));
    keyboard.dispatchEvent(new InputEvent("input",{data:"a",inputType:"insertText",bubbles:true}));
    surface.dispatchEvent(new PointerEvent("pointerdown",{bubbles:true}));window.dispatchEvent(new Event("resize"));plus.click();
    chrome.dispatchEvent(new PointerEvent("pointerenter"));
    fontButton.click();fontFields.font_size.dispatchEvent(new InputEvent("input",{bubbles:true}));
    browseNext.click();exportRows.click();keepAlive.click();viewport.dispatchEvent(new WheelEvent("wheel",{deltaY:120,bubbles:true}));
    document.getElementById("media-close").click();document.getElementById("media-motion").click();
    send({kind:"stopped",clean:mediaClean&&sent===before&&draws===beforeDraws&&zoom===beforeZoom&&animation===0
        &&fontTimer===0&&hoverTimer===0&&hoverCandidate===null&&!fontDialog.open&&!chrome.classList.contains("open")&&document.activeElement!==keyboard});
}
