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
        &&packet?.ptt_features&&packet.connected&&!packet.input_pending&&!packet.output_pending
        &&packet.navigation.kind==="article_reading"&&packet.image_links.length>0
        &&packet.mouse.route==="local";
}
function installInlineImages({surface,canvas,tr,redraw,changed,notice}) {
    const layer=document.createElement("div");layer.id="inline-images";surface.insertBefore(layer,canvas.nextSibling);
    let current=null,items=[],visible=[],strips=[],dead=false;const jobs=new Set();
    function detach(){for(const strip of strips){strip.width=0;strip.height=0;}strips=[];visible=[];layer.replaceChildren();canvas.style.visibility="";}
    function clear() {
        current=null;
        for(const item of items){item.controller?.abort();item.image?.removeAttribute("src");if(item.url)URL.revokeObjectURL(item.url);if(item.still){item.still.width=0;item.still.height=0;}}
        items=[];detach();
    }
    function reset(){if(!current)return;clear();redraw();changed();}
    function matches(packet){return current&&current.presentation===packet.presentation&&current.generation===packet.generation&&current.revision===packet.frame.revision;}
    function open(packet,latest) {
        if(dead||packet.presentation!==latest?.presentation||packet.generation!==latest.generation||packet.revision!==latest.frame.revision)return;
        if(current?.batch===packet.batch&&matches(latest))return;
        if(current?.batch!==packet.batch)clear();else detach();
        current=packet;
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
        const bottom=row=>base.y(row+1);
        if(!matches(packet)){if(current)detach();return {...base,bottom};}
        const gaps=new Map(),width=canvas.width/ratio;
        for(const item of visible){
            const scale=item.still?Math.min(1,(width-24)/item.still.width,560/item.still.height):1;
            item.height=item.still?Math.max(1,item.still.height*scale)+24:48;
            if(item.still){item.still.style.width=Math.max(1,item.still.width*scale)+"px";item.still.style.height=Math.max(1,item.still.height*scale)+"px";}
            gaps.set(item.row,(gaps.get(item.row)||0)+item.height*ratio);
        }
        const mapping=inlineRowLayout(packet.frame.rows,base.y,gaps);
        canvas.style.visibility="hidden";
        while(strips.length<packet.frame.rows){const strip=document.createElement("canvas");strip.className="inline-text-row";strips.push(strip);layer.append(strip);}
        for(let row=0;row<packet.frame.rows;row++){
            const strip=strips[row],height=base.y(row+1)-base.y(row);strip.width=canvas.width;strip.height=height;
            strip.getContext("2d").drawImage(canvas,0,base.y(row),canvas.width,height,0,0,canvas.width,height);
            Object.assign(strip.style,{top:mapping.y(row)/ratio+"px",width:width+"px",height:height/ratio+"px"});
        }
        const used=new Map();
        for(const item of visible){const offset=used.get(item.row)||0;Object.assign(item.box.style,{top:(mapping.bottom(item.row)/ratio+offset)+"px",width:width+"px",height:item.height+"px"});used.set(item.row,offset+item.height);}
        surface.style.height=mapping.height/ratio+"px";
        return {...base,...mapping};
    }
    function linkAtPoint(event){
        if(!current||dead)return -1;
        const item=visible.find(item=>{const r=item.box.getBoundingClientRect();return event.clientX>=r.left&&event.clientX<r.right&&event.clientY>=r.top&&event.clientY<r.bottom;});
        return item?.index??-1;
    }
    return {open,receive,fail,reset,layout,linkAtPoint,active:()=>!!current,
        async destroy(){dead=true;clear();await Promise.allSettled([...jobs]);layer.remove();return jobs.size===0;}};
}
