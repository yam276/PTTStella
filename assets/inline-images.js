// Inline layout consumes Rust row numbers; it never reparses article text or URLs.
function inlineRowLayout(rows, baseY, gaps) {
    const tops=[], bottoms=[];let added=0;
    for(let row=0;row<rows;row++) {
        tops.push(baseY(row)+added);bottoms.push(baseY(row+1)+added);
        added+=gaps.get(row)||0;
    }
    return {y:row=>tops[row]??baseY(rows)+added,bottom:row=>bottoms[row],height:baseY(rows)+added,
        rowAt(value,clamp=false){
            for(let row=0;row<rows;row++) {
                if(value<tops[row])return clamp?Math.max(0,row-1):null;
                if(value<bottoms[row])return row;
            }
            return clamp?rows-1:null;
        }};
}
function inlineShortcut(key, state, packet) {
    return key.key==="Enter"&&!key.ctrlKey&&!key.altKey&&!key.shiftKey
        &&!state.pending&&(!state.enabled||state.mode==="normal")
        &&packet?.article_mode!=="terminal"&&packet?.ptt_features&&packet.connected&&!packet.input_pending&&!packet.output_pending
        &&packet.navigation.kind==="article_reading"&&packet.image_links.length>0
        &&packet.mouse.route==="local";
}
function installInlineImages({surface,canvas,viewport,tr,render,redraw,changed,notice}) {
    const layer=document.createElement("div");layer.id="inline-images";surface.insertBefore(layer,canvas.nextSibling);
    let current=null,items=[],visible=[],strips=[],footer=null,footerLayer=null,footerImage=null,footerPacket=null,footerProgress=null,dead=false,lastLayout=null,lastArticle=null,latestLayout=null;const jobs=new Set();
    function detach(preserve=false){if(!preserve)lastLayout=null;for(const strip of strips){strip.width=0;strip.height=0;}strips=[];visible=[];
        if(footer){footer.width=0;footer.height=0;footerPacket=null;footerProgress=null;footerImage.removeAttribute("src");footerLayer.hidden=true;}layer.replaceChildren();canvas.style.visibility="";}
    function clear() {
        current=null;
        for(const item of items){item.controller?.abort();item.image?.removeAttribute("src");if(item.url)URL.revokeObjectURL(item.url);if(item.still){item.still.width=0;item.still.height=0;}}
        items=[];detach(true);
    }
    function reset(){if(!current)return;clear();redraw();changed();}
    function matches(packet){return current&&current.presentation===packet.presentation&&current.generation===packet.generation&&current.revision===packet.frame.revision&&(current.article_id??null)===(packet.article?.id??null);}
    function open(packet,latest) {
        if(dead||latest?.article_mode==="terminal"||(packet.article_id??null)!==(latest?.article?.id??null)||packet.presentation!==latest?.presentation||packet.generation!==latest.generation||packet.revision!==latest.frame.revision)return;
        if(current?.batch===packet.batch&&matches(latest))return;
        if(current?.batch!==packet.batch)clear();else detach(true);
        current=packet;latestLayout=latest;
        visible=packet.entries.map(entry=>{
            const cached=items.find(item=>item.key===entry.key);
            if(cached){Object.assign(cached,entry);layer.append(cached.box);return cached;}
            const box=document.createElement("div"),note=document.createElement("div");box.className="inline-image";note.className="inline-image-note";
            note.textContent=tr("image-loading");box.append(note);layer.append(box);
            const item={...entry,box,note,still:null};items.push(item);return item;
        });
        if(packet.limited)notice?.(tr("inline-image-limit"));
        redraw();changed();
    }
    function fail(packet){if(current?.batch!==packet.batch)return;const item=items.find(i=>i.key===packet.index);if(item){item.note.textContent=packet.message;item.box.dataset.state="failed";redraw();}}
    function receive(packet) {
        if(dead||current?.batch!==packet.batch)return;
        const item=items.find(i=>i.key===packet.index);if(!item||item.controller)return;
        const controller=new AbortController();item.controller=controller;
        const data=packet.image;
        const job=(async()=>{
            let timeout;
            try {
                const response=await fetch("data:"+data.mime+";base64,"+data.data,{signal:controller.signal});
                const blob=await response.blob();if(controller.signal.aborted)return;
                item.url=URL.createObjectURL(blob);item.image=document.createElement("img");
                // A bounded decode wait also settles when the page is revoked.
                await new Promise((resolve,reject)=>{
                    const finish=error=>{clearTimeout(timeout);controller.signal.removeEventListener("abort",abort);item.image.onload=null;item.image.onerror=null;error?reject(error):resolve();};
                    const abort=()=>finish(new Error("cancelled"));
                    controller.signal.addEventListener("abort",abort,{once:true});
                    item.image.onload=()=>finish();item.image.onerror=()=>finish(new Error(tr("image-decode-failed")));
                    timeout=setTimeout(()=>finish(new Error(tr("image-decode-failed"))),10000);item.image.src=item.url;
                });
                if(dead||controller.signal.aborted||current?.batch!==packet.batch)return;
                if(item.image.naturalWidth!==data.width||item.image.naturalHeight!==data.height)throw new Error(tr("image-size-mismatch"));
                const still=document.createElement("canvas");still.width=data.width;still.height=data.height;
                still.getContext("2d").drawImage(item.image,0,0);still.setAttribute("aria-label",tr("media-title"));
                item.still=still;item.box.append(still);item.note.textContent="";item.box.dataset.state="ready";
                redraw();
            } catch(error){if(!controller.signal.aborted)fail({...packet,message:error.message});}
            finally {clearTimeout(timeout);item.image?.removeAttribute("src");item.image=null;if(item.url){URL.revokeObjectURL(item.url);item.url=null;}}
        })();jobs.add(job);job.finally(()=>jobs.delete(job));
    }
    function layout(packet,base,ratio) {
        latestLayout=packet;
        const bottom=row=>base.y(row+1);
        const cached=!!packet.article;
        if(current&&cached&&((current.article_id??null)!==packet.article.id||current.generation!==packet.generation))clear();
        if(!cached&&!matches(packet)){
            if(current||strips.length)detach();
            // A station grid has no historical scroll position. Reset after a
            // cached layout so WebView cannot retain the old document offset.
            if(lastArticle&&viewport){viewport.scrollTop=0;viewport.scrollLeft=0;}
            lastArticle=null;return {...base,bottom};
        }
        const article=packet.article;
        let anchor=null;
        if(cached&&lastLayout&&lastArticle?.id===article.id&&lastArticle.generation===packet.generation&&viewport){
            const oldRatio=lastArticle.ratio??ratio;
            const top=Math.max(0,viewport.scrollTop-surface.offsetTop)*oldRatio;
            const row=lastLayout.rowAt(top,true);
            anchor={line:lastArticle.first_line+row,offset:(top-lastLayout.y(row))*ratio/oldRatio};
        } else if(cached&&lastArticle&&(lastArticle.id!==article.id||lastArticle.generation!==packet.generation)&&viewport)viewport.scrollTop=0;
        const rowOffset=cached?(current?.first_line??article.first_line)-article.first_line:0;
        const gaps=new Map(),width=canvas.width/ratio;
        for(const item of visible){
            const scale=item.still?Math.min(1,(width-24)/item.still.width,560/item.still.height):1;
            item.height=item.still?Math.max(1,item.still.height*scale)+24:48;
            if(item.still){item.still.style.width=Math.max(1,item.still.width*scale)+"px";item.still.style.height=Math.max(1,item.still.height*scale)+"px";}
            const row=item.row+rowOffset;
            if(row>=0&&row<packet.frame.rows-1)gaps.set(row,(gaps.get(row)||0)+item.height*ratio);
        }
        const mapping=inlineRowLayout(packet.frame.rows,base.y,gaps);
        const bodyRows=cached?Math.min(packet.frame.rows-1,article.last_line-article.first_line+1):packet.frame.rows;
        if(cached){
            const footerHeight=base.y(packet.frame.rows)-base.y(packet.frame.rows-1);
            // Images use the short article's existing blank rows first. Only
            // actual content beyond the original grid adds scrollable height.
            mapping.height=Math.max(base.y(packet.source_frame?.rows??packet.frame.rows),mapping.y(bodyRows)+footerHeight);
        }
        canvas.style.visibility="hidden";
        surface.style.height=mapping.height/ratio+"px";
        if(anchor&&viewport){
            const row=Math.max(0,Math.min(packet.frame.rows-1,anchor.line-article.first_line));
            viewport.scrollTop=Math.max(0,surface.offsetTop+(mapping.y(row)+anchor.offset)/ratio);
        }
        if(cached&&viewport)viewport.scrollTop=Math.max(0,Math.min(viewport.scrollTop,
            Math.max(0,surface.offsetTop+mapping.height/ratio-viewport.clientHeight)));
        // Only rasterize viewport rows. Cache size never becomes canvas height
        // or a collection of hundreds of full-resolution backing stores.
        const top=viewport?Math.max(0,viewport.scrollTop-surface.offsetTop)*ratio:0;
        const start=viewport?Math.max(0,mapping.rowAt(top,true)-2):0;
        const textRows=bodyRows;
        const end=viewport?Math.min(textRows,mapping.rowAt(top+viewport.clientHeight*ratio,true)+3):textRows;
        const count=end-start;
        while(strips.length>count){const strip=strips.pop();strip.width=0;strip.height=0;strip.remove();}
        while(strips.length<count){const strip=document.createElement("canvas");strip.className="inline-text-row";strips.push(strip);layer.append(strip);}
        function paintRow(strip,row,top,glyphs=null){
            const height=base.y(row+1)-base.y(row);
            if(strip.width!==canvas.width)strip.width=canvas.width;
            if(strip.height!==height)strip.height=height;
            if(cached){
                const frame={...packet.frame,rows:1,cursor_visible:false,glyphs:(glyphs??packet.frame.glyphs.filter(g=>g.row===row)).map(g=>({...g,row:0}))};
                const links=packet.links.filter(link=>link.row===row).map(link=>({...link,row:0}));
                render(strip,{...packet,frame,links},{cursor:false,linkHints:true});
            } else strip.getContext("2d").drawImage(canvas,0,base.y(row),canvas.width,height,0,0,canvas.width,height);
            Object.assign(strip.style,{width:width+"px",height:height/ratio+"px"});
            if(top!==null)strip.style.top=top/ratio+"px";
        }
        for(let at=0;at<count;at++)paintRow(strips[at],start+at,mapping.y(start+at));
        const used=new Map();
        for(const item of visible){
            const row=item.row+rowOffset,shown=row>=0&&row<packet.frame.rows-1;
            item.box.hidden=!shown;if(!shown)continue;
            const offset=used.get(row)||0;
            Object.assign(item.box.style,{top:(mapping.bottom(row)/ratio+offset)+"px",width:width+"px",height:item.height+"px"});used.set(row,offset+item.height);
        }
        surface.style.height=mapping.height/ratio+"px";
        lastLayout=mapping;lastArticle=cached?{...article,generation:packet.generation,ratio}:null;
        if(cached){
            const row=packet.frame.rows-1,height=base.y(row+1)-base.y(row);
            // Rust identifies the numeric status cells. Only the visible
            // page and percentage are local decorations. Colours come from
            // the visible source page; source PNGs stay intact.
            let footerGlyphs=packet.frame.glyphs.filter(g=>g.row===row),progress=null;
            const cells=article.percent_cells;
            if(cells&&viewport){
                // Minimum-height blank padding is not unread article content.
                const bodyRows=Math.min(row,article.last_line-article.first_line+1);
                const total=Math.max(1,mapping.y(bodyRows));
                const seen=Math.max(0,(viewport.scrollTop-surface.offsetTop+viewport.clientHeight)*ratio-height);
                const percent=article.first_line===1&&article.end?(seen>=total-1?100:Math.min(99,Math.floor(seen/total*100))):null;
                progress=(percent===null?"--":String(percent)).padStart(cells.end-cells.start," ");
                footerGlyphs=footerGlyphs.map(g=>g.col>=cells.start&&g.col<cells.end?{...g,text:progress[g.col-cells.start]}:g);
            }
            if(article.pages?.length&&viewport){
                const line=article.first_line+Math.min(article.last_line-article.first_line,mapping.rowAt(top,true));
                const page=article.pages.reduce((best,page)=>Math.abs(page.line-line)<Math.abs(best.line-line)?page:best);
                if(page.styles?.length===packet.frame.cols){
                    footerGlyphs=footerGlyphs.map(g=>({...g,left:page.styles[g.col],right:page.styles[g.col+g.width-1]}));
                }
                progress=JSON.stringify([progress,page.line,page.number]);
                const cells=article.page_cells,value=cells?String(page.number).padStart(cells.end-cells.start," "):null;
                if(cells&&value.length===cells.end-cells.start){
                    footerGlyphs=footerGlyphs.map(g=>g.col>=cells.start&&g.col<cells.end?{...g,text:value[g.col-cells.start]}:g);
                }
            }
            if(!footer){footer=document.createElement("canvas");footer.className="article-footer-canvas";
                footerLayer=document.createElement("div");footerLayer.className="article-footer";
                footerImage=document.createElement("img");footerImage.className="article-footer-image";footerImage.draggable=false;footerLayer.append(footerImage);}
            const host=viewport?.parentNode??surface;
            if(footerLayer.parentNode!==host)host.append(footerLayer);
            footerLayer.hidden=false;
            // WebKit can lose a standalone canvas while scrolling a long view.
            // Present the same renderer's one-row bitmap; scrolling only
            // re-encodes it when the integer reading percentage changes.
            // Fonts are loaded before Paint is published by the surface owner.
            if(footerPacket!==packet||footerProgress!==progress||footer.width!==canvas.width||footer.height!==height){
                paintRow(footer,row,null,footerGlyphs);footerImage.src=footer.toDataURL("image/png");footerPacket=packet;footerProgress=progress;
                footerImage.alt=footerGlyphs.map(g=>g.text).join("");
            }
            // The overlay belongs to the main view, outside the scroll port.
            const bounds=surface.getBoundingClientRect(),origin=host.getBoundingClientRect();
            const footerTop=Math.min(bounds.top+(mapping.height-height)/ratio,viewport?viewport.getBoundingClientRect().bottom-height/ratio:Infinity);
            Object.assign(footerLayer.style,{left:(bounds.left-origin.left)+"px",top:(footerTop-origin.top)+"px",
                width:width+"px",height:height/ratio+"px"});
            const drawn=footerImage.getBoundingClientRect(),drawnTop=(drawn.top-bounds.top)*ratio,drawnHeight=drawn.height*ratio;
            return {...base,...mapping,footerHeight:drawn.height,
                y:r=>r===row?drawnTop:Math.min(mapping.y(r),drawnTop),
                bottom:r=>r===row?drawnTop+drawnHeight:Math.min(mapping.bottom(r),drawnTop),
                rowAt(value,clamp=false){return value>=drawnTop&&value<drawnTop+drawnHeight?row:mapping.rowAt(value,clamp);}};
        }
        return {...base,...mapping};
    }
    function linkAtPoint(event){
        if(!current||dead||!matches(latestLayout))return -1;
        if(footerLayer&&!footerLayer.hidden){const r=footerImage.getBoundingClientRect();if(event.clientX>=r.left&&event.clientX<r.right&&event.clientY>=r.top&&event.clientY<r.bottom)return -1;}
        const item=visible.find(item=>{const r=item.box.getBoundingClientRect();return event.clientX>=r.left&&event.clientX<r.right&&event.clientY>=r.top&&event.clientY<r.bottom;});
        return item?.index??-1;
    }
    return {open,receive,fail,reset,layout,linkAtPoint,active:()=>!!current,
        async destroy(){dead=true;clear();lastLayout=null;lastArticle=null;latestLayout=null;await Promise.allSettled([...jobs]);layer.remove();footerLayer?.remove();return jobs.size===0;}};
}
