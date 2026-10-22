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
// One held Enter owns one timer. Repeats never become station input or extra requests.
function createInlineImageKey({scope,allowed,single,all,setTimer=setTimeout,clearTimer=clearTimeout}) {
    let held=null;
    function cancel(){if(held?.timer!==null&&held?.timer!==undefined)clearTimer(held.timer);held=null;}
    function valid(press){return press.scope===scope()&&allowed();}
    function down(event){
        if(!event.isTrusted||event.repeat||held)return;
        const press={scope:scope(),event,fired:false,timer:null};held=press;
        press.timer=setTimer(()=>{press.timer=null;if(held!==press)return;press.fired=true;if(valid(press))all(press.event);},500);
    }
    function up(event){
        if(event.key!=="Enter"||!event.isTrusted||!held)return false;
        const press=held;cancel();if(!press.fired&&valid(press)&&!event.ctrlKey&&!event.altKey&&!event.metaKey&&!event.shiftKey&&!event.isComposing)single(event);return true;
    }
    return {down,up,cancel,held:()=>!!held,validate(){if(held&&!valid(held))cancel();},clean:()=>held===null};
}
function installInlineImages({surface,canvas,viewport,tr,render,redraw,changed,notice,allowed=()=>false,request=()=>{}}) {
    const layer=document.createElement("div");layer.id="inline-images";surface.insertBefore(layer,canvas.nextSibling);
    let current=null,items=[],visible=[],strips=[],footer=null,footerLayer=null,footerImage=null,footerPacket=null,footerProgress=null,dead=false,lastLayout=null,lastArticle=null,latestLayout=null,freeScroll=false;const jobs=new Set();
    function setFreeScroll(value){const changed=freeScroll!==value;freeScroll=value;return changed;}
    const gifs=createGifPlayback({document,window:typeof window!=="undefined"?window:null,limit:2});
    function gifScore(item){
        if(dead||!current||!geometry||!matches(latestLayout)||item.box.hidden||item.box.isConnected===false||document.querySelector?.("dialog[open]"))return Infinity;
        const box=item.box.getBoundingClientRect(),bounds=viewport?.getBoundingClientRect();
        if(!bounds)return 0;
        const bottom=bounds.bottom-(geometry.footerHeight??0);
        return box.top<bottom&&box.bottom>bounds.top?Math.abs((box.top+box.bottom-bounds.top-bottom)/2):Infinity;
    }
    const controls=new Map();let candidates=[],geometry=null,knownScroll=viewport?.scrollTop??0;
    const scope=()=>JSON.stringify([latestLayout?.generation,latestLayout?.article?.id??latestLayout?.presentation]);
    const gesture=createInlineImageKey({scope,allowed:()=>!dead&&allowed(),single:singleImage,all:event=>request({kind:"all"},event)});
    function suspend(){gesture.cancel();}
    function moved(){if(viewport&&Math.abs(viewport.scrollTop-knownScroll)>1){suspend();knownScroll=viewport.scrollTop;}gifs.refresh();}
    function candidateItem(candidate){return items.find(item=>item.source===candidate.source);}
    function visibleCandidates(){
        moved();
        if(!viewport||!geometry)return [];
        const bounds=viewport.getBoundingClientRect(),origin=surface.getBoundingClientRect();
        const bottom=bounds.bottom-(geometry.footerHeight??0);
        return candidates.map(candidate=>{
            const item=candidateItem(candidate);
            const top=origin.top+geometry.y(candidate.row)/geometry.ratio;
            const end=origin.top+geometry.bottom(candidate.row)/geometry.ratio;
            const box=item&&!item.box.hidden?item.box.getBoundingClientRect():null;
            const shown=top<bottom&&end>bounds.top||box&&box.top<bottom&&box.bottom>bounds.top;
            return {...candidate,item,shown};
        }).filter(candidate=>candidate.shown).sort((a,b)=>a.row-b.row||a.end-b.end||a.index-b.index);
    }
    function widen(candidate){
        const item=candidateItem(candidate);
        if(item){if(item.wide||item.box.dataset.state==="failed")return false;item.wide=true;redraw();return true;}
        return false;
    }
    function singleImage(event){
        if(dead||!event.isTrusted||!allowed())return;
        const shown=visibleCandidates();
        const next=shown.find(candidate=>!candidate.item);
        if(next){expandOne(next,event);return;}
        const first=shown.find(candidate=>candidate.item&&!candidate.item.wide&&candidate.item.box.dataset.state!=="failed");
        if(first)widen(first);
    }
    function expandOne(candidate,event){
        if(dead||!event.isTrusted||!allowed()||candidateItem(candidate))return;
        request({kind:"link",index:candidate.index},event);
    }
    function triggerDown(event){if(event.button===0&&event.target.closest?.(".inline-expand-trigger")){event.preventDefault();event.stopPropagation();}}
    function triggerClick(event){
        const button=event.target.closest?.(".inline-expand-trigger");if(!button)return;
        event.preventDefault();event.stopPropagation();
        if(event.button!==undefined&&event.button!==0||event.shiftKey||event.ctrlKey||event.metaKey||event.altKey)return;
        const candidate=candidates.find(candidate=>controls.get(candidate.source)===button);
        if(candidate)expandOne(candidate,event);
    }
    function buildCandidates(packet){
        const result=[];
        if(!packet.article||packet.article_mode==="terminal")return result;
        for(const index of packet.image_links??[]){
            const link=packet.links[index];if(!link||link.row>=packet.frame.rows-1)continue;
            const previous=result.find(candidate=>candidate.source===link.url);
            if(previous){if(link.row<=previous.row+1){previous.row=Math.max(previous.row,link.row);previous.start=link.start;previous.end=link.end;}continue;}
            result.push({index,row:link.row,start:link.start,end:link.end,source:link.url});
        }
        return result;
    }
    function layoutControls(packet,mapping,base,ratio,wholeRow){
        const live=new Set(candidates.map(candidate=>candidate.source));
        for(const [source,button] of controls)if(!live.has(source)){button.remove();controls.delete(source);}
        const bounds=viewport?.getBoundingClientRect(),origin=surface.getBoundingClientRect();
        for(const candidate of candidates){
            const height=(base.y(candidate.row+1)-base.y(candidate.row))/ratio;
            const size=Math.min(Math.max(14,Math.min(26,height-2)),(base.x(candidate.end)-base.x(candidate.start))/ratio);
            const top=mapping.y(candidate.row)/ratio+(height-size)/2,left=base.x(candidate.end)/ratio-size;
            let button=controls.get(candidate.source);
            if(!button){button=document.createElement("button");button.className="inline-expand-trigger";button.type="button";button.textContent="";controls.set(candidate.source,button);layer.append(button);}
            if(button.parentNode!==layer)layer.append(button);
            const expanded=!!candidateItem(candidate);button.disabled=expanded||!allowed();
            button.setAttribute("aria-label",tr(expanded?"inline-image-opened":"inline-image-open-one",{url:candidate.source}));button.setAttribute("aria-expanded",String(expanded));
            button.hidden=!wholeRow(candidate.row)||!!bounds&&(origin.top+top>=bounds.bottom-height||origin.top+top+size<=bounds.top);
            Object.assign(button.style,{left:left+"px",top:top+"px",width:size+"px",height:size+"px",fontSize:size+"px"});
        }
    }
    function detach(preserve=false){if(!preserve)lastLayout=null;for(const strip of strips){strip.width=0;strip.height=0;}strips=[];visible=[];
        if(footer){footer.width=0;footer.height=0;footerPacket=null;footerProgress=null;footerImage.removeAttribute("src");footerLayer.hidden=true;}layer.replaceChildren();canvas.style.visibility="";gifs.refresh();}
    function clear() {
        current=null;suspend();freeScroll=false;
        for(const button of controls.values())button.remove();controls.clear();candidates=[];geometry=null;
        for(const item of items){item.gif?.remove();item.animation?.removeAttribute("src");item.controller?.abort();item.image?.removeAttribute("src");if(item.url)URL.revokeObjectURL(item.url);if(item.still){item.still.width=0;item.still.height=0;}}
        items=[];detach(true);
    }
    function reset(){suspend();if(!current)return;clear();redraw();changed();}
    function matches(packet){return current&&current.presentation===packet.presentation&&current.generation===packet.generation&&current.revision===packet.frame.revision&&(current.article_id??null)===(packet.article?.id??null);}
    function open(packet,latest) {
        if(dead||latest?.article_mode==="terminal"||(packet.article_id??null)!==(latest?.article?.id??null)||packet.presentation!==latest?.presentation||packet.generation!==latest.generation||packet.revision!==latest.frame.revision)return;
        if(current?.batch===packet.batch&&matches(latest)&&current.limited===packet.limited&&JSON.stringify(current.entries)===JSON.stringify(packet.entries))return;
        if(current?.batch!==packet.batch)clear();else detach(true);
        current=packet;latestLayout=latest;
        visible=packet.entries.map(entry=>{
            const source=latest.links?.[entry.index]?.url;
            const cached=items.find(item=>item.key===entry.key);
            if(cached){Object.assign(cached,entry,{source});layer.append(cached.box);return cached;}
            const box=document.createElement("div"),note=document.createElement("div");box.className="inline-image";note.className="inline-image-note";
            note.textContent=tr("image-loading");box.append(note);layer.append(box);
            const item={...entry,source,box,note,still:null,wide:false};items.push(item);return item;
        });
        if(packet.limited){
            notice?.(tr("inline-image-limit"));
        }
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
                if(data.mime==="image/gif"&&data.animated){
                    item.animation=item.image;item.image=null;item.animation.alt=tr("media-title");item.animation.draggable=false;
                    item.box.append(item.animation);still.setAttribute("aria-label",tr("gif-paused"));
                    item.box.dataset.motion="paused";
                    item.gif=gifs.add({image:item.animation,still,url:item.url,cost:data.animation_pixels,score:()=>gifScore(item),
                        changed:playing=>{item.box.dataset.motion=playing?"playing":"paused";item.animation.alt=tr("gif-playing");still.setAttribute("aria-label",tr("gif-paused"));}});
                }

                redraw();
            } catch(error){if(!controller.signal.aborted)fail({...packet,message:error.message});}
            finally {clearTimeout(timeout);item.image?.removeAttribute("src");item.image=null;if(item.url&&!item.animation){URL.revokeObjectURL(item.url);item.url=null;}}
        })();jobs.add(job);job.finally(()=>jobs.delete(job));
    }
    function layout(packet,base,ratio) {
        moved();
        latestLayout=packet;gesture.validate();
        if(lastArticle&&(lastArticle.id!==packet.article?.id||lastArticle.generation!==packet.generation)){suspend();freeScroll=false;}
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
        candidates=buildCandidates(packet);
        for(const item of visible){
            const scale=item.still?(item.wide?width/item.still.width:Math.min(1,(width-24)/item.still.width,560/item.still.height)):1;
            item.box.style.padding=item.wide?"12px 0":"12px";
            item.height=item.still?Math.max(1,item.still.height*scale)+24:48;
            if(item.still){item.still.style.width=Math.max(1,item.still.width*scale)+"px";item.still.style.height=Math.max(1,item.still.height*scale)+"px";}
            if(item.animation&&item.still){item.animation.style.width=item.still.style.width;item.animation.style.height=item.still.style.height;}

            const row=item.row+rowOffset;
            if(row>=0&&row<packet.frame.rows-1)gaps.set(row,(gaps.get(row)||0)+item.height*ratio);
        }
        const mapping=inlineRowLayout(packet.frame.rows,base.y,gaps);
        const bodyRows=cached?Math.min(packet.frame.rows-1,article.last_line-article.first_line+1):packet.frame.rows;
        if(cached){
            const footerHeight=base.y(packet.frame.rows)-base.y(packet.frame.rows-1);
            // Images use the short article's existing blank rows first. Only
            // actual content beyond the original grid adds scrollable height.
            const minimum=base.y(packet.source_frame?.rows??packet.frame.rows),content=mapping.y(bodyRows)+footerHeight;
            // Native scroll extents are integer CSS pixels. Round growing
            // content outward once so a fractional image height cannot leave
            // the final text row behind the pinned footer at maximum scroll.
            mapping.height=content>minimum?Math.ceil(content/ratio)*ratio:minimum;
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
        const footerHeight=cached?base.y(packet.frame.rows)-base.y(packet.frame.rows-1):0;
        const readingBottom=viewport?Math.min(mapping.height-footerHeight,top+viewport.clientHeight*ratio-footerHeight):mapping.height;
        // A fitted/zoomed viewport and inserted images need not contain a whole
        // number of rows. Discrete navigation shows only complete edge rows;
        // wheel scrolling intentionally retains the normal continuous clipping.
        const wholeRow=row=>!cached||!viewport||freeScroll||mapping.y(row)>=top-.01&&mapping.bottom(row)<=readingBottom+.01;
        const reading={free:setFreeScroll,clippedAt(pixel){
            const row=mapping.rowAt(pixel);
            return row!==null&&row<bodyRows&&!wholeRow(row)?row:-1;
        },align(target,maximum,edge="nearest"){
            if(target<=0||target>=maximum&&edge!=="before")return target;
            const pixel=(target-surface.offsetTop)*ratio,row=mapping.rowAt(pixel);
            if(row===null||row>=bodyRows)return target; // Images have no terminal cells.
            const before=mapping.y(row),after=mapping.bottom(row);
            const aligned=Math.abs(pixel-before)<.01?before:edge==="before"?before:edge==="after"?after:pixel-before<=after-pixel?before:after;
            return Math.max(0,Math.min(maximum,surface.offsetTop+aligned/ratio));
        }};
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
        for(let at=0;at<count;at++){
            paintRow(strips[at],start+at,mapping.y(start+at));
            strips[at].style.visibility=wholeRow(start+at)?"":"hidden";
        }
        const used=new Map();
        for(const item of visible){
            const row=item.row+rowOffset,shown=row>=0&&row<packet.frame.rows-1;
            item.box.hidden=!shown;if(!shown)continue;
            const offset=used.get(row)??0;
            Object.assign(item.box.style,{top:(mapping.bottom(row)/ratio+offset)+"px",width:width+"px",height:item.height+"px"});used.set(row,offset+item.height);
        }
        surface.style.height=mapping.height/ratio+"px";
        layoutControls(packet,mapping,base,ratio,wholeRow);
        geometry={...mapping,ratio,footerHeight:cached?(base.y(packet.frame.rows)-base.y(packet.frame.rows-1))/ratio:0};knownScroll=viewport?.scrollTop??0;
        lastLayout=mapping;lastArticle=cached?{...article,generation:packet.generation,ratio}:null;
        gifs.refresh();
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
            return {...base,...mapping,reading,footerHeight:drawn.height,
                y:r=>r===row?drawnTop:Math.min(mapping.y(r),drawnTop),
                bottom:r=>r===row?drawnTop+drawnHeight:Math.min(mapping.bottom(r),drawnTop),
                rowAt(value,clamp=false){
                    if(value>=drawnTop&&value<drawnTop+drawnHeight)return row;
                    const at=mapping.rowAt(value,clamp);
                    return at===null||!clamp&&!wholeRow(at)?null:at;
                }};
        }
        return {...base,...mapping};
    }
    function linkAtPoint(event){
        if(!current||dead||!matches(latestLayout))return -1;
        if(footerLayer&&!footerLayer.hidden){const r=footerImage.getBoundingClientRect();if(event.clientX>=r.left&&event.clientX<r.right&&event.clientY>=r.top&&event.clientY<r.bottom)return -1;}
        const item=visible.find(item=>{const r=item.box.getBoundingClientRect();return event.clientX>=r.left&&event.clientX<r.right&&event.clientY>=r.top&&event.clientY<r.bottom;});
        return item?.index??-1;
    }
    return {open,receive,fail,reset,layout,linkAtPoint,refreshAnimations:gifs.refresh,active:()=>!!current,keydown:gesture.down,keyup:gesture.up,keyHeld:gesture.held,suspend,
        bindings:[...gifs.bindings,[layer,"pointerdown",triggerDown],[layer,"click",triggerClick],...(viewport?[[viewport,"scroll",moved]]:[]),
            ...(typeof window!=="undefined"?[[window,"blur",suspend]]:[]),[document,"visibilitychange",suspend]],
        async destroy(){dead=true;gifs.destroy();clear();lastLayout=null;lastArticle=null;latestLayout=null;await Promise.allSettled([...jobs]);layer.remove();footerLayer?.remove();return jobs.size===0;}};
}
