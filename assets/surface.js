const canvas=document.getElementById("screen"), keyboard=document.getElementById("keyboard");
const surface=document.getElementById("surface"), viewport=document.querySelector(".workspace");
const chrome=document.getElementById("chrome"), toolbar=chrome.querySelector("header");
const minus=document.getElementById("zoom-out"), plus=document.getElementById("zoom-in");
const fit=document.getElementById("zoom-fit"), level=document.getElementById("zoom-level");
const ctx=canvas.getContext("2d",{willReadFrequently:true});
const palette=["#000000","#800000","#008000","#808000","#000080","#800080","#008080","#c0c0c0",
    "#808080","#ff0000","#00ff00","#ffff00","#0000ff","#ff00ff","#00ffff","#ffffff"];
const family='"Cascadia Mono", "Microsoft JhengHei", monospace', font='20px '+family;
let generation=0, revision=-1, composing=false, composed=null, sent=0, compositionGeneration=0;
let latest=null, animation=0, stopped=false, draws=0, zoom=1;
const send=value=>{ sent++; dioxus.send(value); };
const keys={Enter:[13],Backspace:[8],Tab:[9],Escape:[27],ArrowUp:[27,91,65],ArrowDown:[27,91,66],
    ArrowRight:[27,91,67],ArrowLeft:[27,91,68],Home:[27,91,72],End:[27,91,70],
    PageUp:[27,91,53,126],PageDown:[27,91,54,126],Delete:[27,91,51,126]};
function keydown(e) {
    if(e.isComposing||composing||e.key==="Process"||e.metaKey) return;
    if(e.ctrlKey&&["+","=","-","0"].includes(e.key)) {
        e.preventDefault(); zoomTo(e.key==="0"?1:zoom+(e.key==="-"?-.25:.25)); return;
    }
    composed=null; let bytes=keys[e.key];
    if(e.ctrlKey&&/^[a-z]$/i.test(e.key)) bytes=[e.key.toUpperCase().charCodeAt(0)&31];
    if(bytes) { e.preventDefault(); send({kind:"key",bytes,generation,trusted:e.isTrusted}); }
}
function input(e) {
    if(composing||e.isComposing) return;
    const text=e.data||keyboard.value; keyboard.value="";
    if(composed===text) { composed=null; return; }
    if(text) send({kind:"text",text,generation,trusted:e.isTrusted});
}
function compositionstart() { composing=true; compositionGeneration=generation; }
function compositionend(e) {
    composing=false; composed=e.data; keyboard.value="";
    if(e.data) send({kind:"text",text:e.data,generation:compositionGeneration,trusted:e.isTrusted});
}
function focus() { keyboard.focus({preventScroll:true}); }
function pointerdown(e) {
    if(e.button!==0) return;
    const bounds=viewport.getBoundingClientRect();
    if(e.clientX>=bounds.left+viewport.clientWidth || e.clientY>=bounds.top+viewport.clientHeight) return;
    // Prevent the scroll container's default focus step from overriding ours.
    e.preventDefault(); focus();
}
function activate() { if(!chrome.contains(document.activeElement)) focus(); }
function paste(e) { e.preventDefault(); }
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
    [keyboard,"compositionstart",compositionstart],[keyboard,"compositionend",compositionend],
    [keyboard,"paste",paste],[viewport,"pointerdown",pointerdown],
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
function graphic(text,x,y,w,h) {
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
    const scale=Math.min(canvas.width/width,canvas.height/height);
    const x=col=>Math.round(col*12*scale),y=row=>Math.round(row*24*scale);
    ctx.setTransform(1,0,0,1,0,0); ctx.fillStyle="#000"; ctx.fillRect(0,0,canvas.width,canvas.height);
    // Opaque backgrounds first; both sides of every shared edge use one integer.
    for(const glyph of frame.glyphs) for(let half=0;half<glyph.width;half++) {
        const [,bg]=colours(half===0?glyph.left:glyph.right),col=glyph.col+half;
        ctx.fillStyle=bg; ctx.fillRect(x(col),y(glyph.row),x(col+1)-x(col),y(glyph.row+1)-y(glyph.row));
    }
    ctx.font=(20*scale)+"px "+family; ctx.textBaseline="alphabetic";
    for(const glyph of frame.glyphs) {
        if(glyph.text===" ") continue;
        const left=x(glyph.col),top=y(glyph.row),span=x(glyph.col+glyph.width)-left,h=y(glyph.row+1)-top;
        const advance=Math.max(ctx.measureText(glyph.text).width,1);
        for(let half=0;half<glyph.width;half++) {
            const [fg]=colours(half===0?glyph.left:glyph.right),col=glyph.col+half;
            ctx.save(); ctx.beginPath(); ctx.rect(x(col),top,x(col+1)-x(col),h); ctx.clip(); ctx.fillStyle=fg;
            if(!graphic(glyph.text,left,top,span,h)) {
                ctx.translate(left,top+20*scale); ctx.scale(span/advance,1); ctx.fillText(glyph.text,0,0);
            }
            ctx.restore();
        }
    }
    if(frame.cursor_visible) {
        const [col,row]=frame.cursor,bottom=y(row+1),thickness=Math.max(1,Math.round(2*scale));
        ctx.fillStyle=palette[7]; ctx.fillRect(x(col),bottom-thickness,x(col+1)-x(col),thickness);
    }
    draws++; return {x,y,ratio};
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
    const ink=(col,row,bg)=>{
        const pixels=ctx.getImageData(x(col),y(row),x(col+2)-x(col),y(row+1)-y(row)).data,counts=new Map();
        for(let i=0;i<pixels.length;i+=4) {
            const rgb=Array.from(pixels.slice(i,i+3));if(rgb.every((v,j)=>v===bg[j])) continue;
            const key=rgb.join(",");counts.set(key,(counts.get(key)||0)+1);
        }
        const ordered=[...counts].sort((a,b)=>b[1]-a[1]);return ordered.length?ordered[0][0].split(",").map(Number):[];
    };
    const colour_samples=Array.from({length:16},(_,i)=>ink(i*2,15,i===0?[192,192,192]:[0,0,0]));
    colour_samples.push(...Array.from({length:4},(_,i)=>ink(i*2,16,[0,0,0])));
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
        fonts:document.fonts.status,dpr:ratio,pixel_size:[canvas.width,canvas.height],font_stack:font,fills_body,
        colour_samples,art_seam_pixels:checks[0].seams,art_zoom_seams:checks.map(c=>c.seams),
        art_geometry:checks.every(c=>c.geometry),hover_controls,proportional_zoom,zoom_samples});
}
function paint(packet) {
    if(packet.generation<generation||(packet.generation===generation&&packet.frame.revision<revision)) return;
    if(packet.generation!==generation){keyboard.value="";composed=null;focus();}
    generation=packet.generation;revision=packet.frame.revision;latest=packet;
    const geometry=draw(packet);send({kind:"painted",revision,generation});if(packet.probe) probe(packet,geometry);
    surface.dataset.revision=String(revision);
}
focus();
try {
    await document.fonts.load(font,"中M");await document.fonts.ready;
    while(true){const packet=await dioxus.recv();if(packet.stop) break;paint(packet);}
} catch(e){send({kind:"error",message:String(e)});}
finally {
    stopped=true;latest=null;observer.disconnect();if(animation){cancelAnimationFrame(animation);animation=0;}
    for(const [target,event,callback,options] of bindings) target.removeEventListener(event,callback,options);
    keyboard.value="";keyboard.blur();hideChrome();
    const before=sent,beforeDraws=draws,beforeZoom=zoom;
    keyboard.dispatchEvent(new KeyboardEvent("keydown",{key:"ArrowUp",bubbles:true}));
    keyboard.dispatchEvent(new InputEvent("input",{data:"a",inputType:"insertText",bubbles:true}));
    surface.dispatchEvent(new PointerEvent("pointerdown",{bubbles:true}));window.dispatchEvent(new Event("resize"));plus.click();
    chrome.dispatchEvent(new PointerEvent("pointerenter"));
    send({kind:"stopped",clean:sent===before&&draws===beforeDraws&&zoom===beforeZoom&&animation===0
        &&!chrome.classList.contains("open")&&document.activeElement!==keyboard});
}
