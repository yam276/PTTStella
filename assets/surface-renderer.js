// Shared grid renderer for the surface, settings preview and PNG.
const palette=["#000000","#800000","#008000","#808000","#000080","#800080","#008080","#c0c0c0",
    "#808080","#ff0000","#00ff00","#ffff00","#0000ff","#ff00ff","#00ffff","#ffffff"];
function colours(style) {
    const fg=style.fg+(style.bright&&style.fg<8?8:0), bg=style.bg;
    return style.inverse?[palette[bg],palette[fg]]:[palette[fg],palette[bg]];
}
// Filled BBS graphics follow the grid, independent of font metrics/fallback.
function graphicCode(text){
    if([...text].length!==1)return null;
    const code=text.codePointAt(0);
    return code>=0x2580&&code<=0x2590||code===0x2594||code===0x2595||code>=0x2596&&code<=0x259f||code>=0x25e2&&code<=0x25e5?code:null;
}
function graphic(ctx,text,x,y,w,h) {
    const code=graphicCode(text);if(code===null)return false;
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
function render(target,packet,{cursor=true,hovered=-1,linkHints=false,highlight=null}={}) {
    const frame=packet.frame,width=frame.cols*12,height=frame.rows*24;
    const ctx=target.getContext("2d");
    const scale=Math.min(target.width/width,target.height/height);
    // Each backing dimension was rounded independently by the host. In a
    // one-row article strip its height can be one pixel taller than 24*scale;
    // using the smaller scale for both axes leaves that pixel black. Shared
    // edges must span the actual backing extent, including the final edge.
    const x=col=>Math.round(col*target.width/frame.cols),y=row=>Math.round(row*target.height/frame.rows);
    ctx.setTransform(1,0,0,1,0,0); ctx.fillStyle="#000"; ctx.fillRect(0,0,target.width,target.height);
    // Opaque backgrounds first; both sides of every shared edge use one integer.
    for(const glyph of frame.glyphs) for(let half=0;half<glyph.width;half++) {
        const [,bg]=colours(highlight?.(glyph)??(half===0?glyph.left:glyph.right)),col=glyph.col+half;
        ctx.fillStyle=bg; ctx.fillRect(x(col),y(glyph.row),x(col+1)-x(col),y(glyph.row+1)-y(glyph.row));
    }
    ctx.textBaseline="alphabetic";
    const fontInk=new Map(),finite=value=>Number.isFinite(value)?value:0;
    for(const glyph of frame.glyphs) {
        if(glyph.text===" ") continue;
        ctx.font=fontDescription(packet.settings,glyph.width===2,scale);
        const left=x(glyph.col),top=y(glyph.row),span=x(glyph.col+glyph.width)-left,h=y(glyph.row+1)-top;
        const metrics=ctx.measureText(glyph.text),advance=Math.max(metrics.width,1);
        // Text keeps the font's aspect ratio; narrow symbols are centred instead
        // of stretched into ovals. Connected box/shade glyphs retain grid fitting.
        const code=glyph.text.codePointAt(0),connected=[...glyph.text].length===1&&code>=0x2500&&code<=0x2593;
        let fitting=connected?span/advance:Math.min(1,span/advance),baseline=packet.settings.baseline*scale;
        if(!connected){
            // Font line boxes contain leading and can dwarf the visible ink.
            // A shared ink sample keeps ordinary letters/CJK on one baseline;
            // the actual glyph also contributes for taller fallback characters.
            let ink=fontInk.get(ctx.font);
            if(!ink){
                const sample=ctx.measureText("[](){}Agjpqy_中文");
                ink={ascent:sample.actualBoundingBoxAscent,descent:sample.actualBoundingBoxDescent};fontInk.set(ctx.font,ink);
            }
            const ascent=Math.ceil(Math.max(0,finite(ink.ascent),finite(metrics.actualBoundingBoxAscent)));
            const descent=Math.ceil(Math.max(0,finite(ink.descent),finite(metrics.actualBoundingBoxDescent)));
            if(ascent+descent>0){
                fitting=Math.min(fitting,h/(ascent+descent));
                baseline=Math.max(ascent*fitting,Math.min(baseline,h-descent*fitting));
            }
        }
        // Optional horizontal width fills more/less of the same cell without
        // moving columns or changing text height. Connected ANSI geometry stays fitted.
        const horizontal=connected?fitting:Math.min(span/advance,fitting*(packet.settings.font_width??100)/100);
        const origin=connected?left:left+(span-advance*horizontal)/2;
        for(let half=0;half<glyph.width;half++) {
            const [fg]=colours(highlight?.(glyph)??(half===0?glyph.left:glyph.right)),col=glyph.col+half;
            ctx.save(); ctx.beginPath(); ctx.rect(x(col),top,x(col+1)-x(col),h); ctx.clip(); ctx.fillStyle=fg;
            if(!graphic(ctx,glyph.text,left,top,span,h)) {
                ctx.translate(origin,top+baseline); ctx.scale(horizontal,connected?1:fitting); ctx.fillText(glyph.text,0,0);
            }
            ctx.restore();
        }
    }
    if(cursor&&frame.cursor_visible) {
        const [col,row]=frame.cursor,bottom=y(row+1),thickness=Math.max(1,Math.round(2*scale));
        ctx.fillStyle=palette[7]; ctx.fillRect(x(col),bottom-thickness,x(col+1)-x(col),thickness);
    }
    // Rust supplies the same visible spans used for hit testing, including OSC 8.
    // Link hints decorate the live view only; PNG exports keep the source pixels.
    if(linkHints) {
        const thickness=Math.max(1,Math.round(scale));
        packet.links.forEach((link,index)=>{
            ctx.fillStyle=palette[index===hovered?7:8];
            ctx.fillRect(x(link.start),y(link.row+1)-thickness,x(link.end)-x(link.start),thickness);
        });
    }
    return {x,y};
}
