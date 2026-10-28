// Search only the projected glyphs, never raw station bytes or hidden content.
function createArticleFind({document,tr,current,allowed,prepare,restore,redraw,jump,changed}) {
    const root=document.getElementById("article-find"),input=document.getElementById("article-find-input");
    const status=document.getElementById("article-find-status"),previous=document.getElementById("article-find-previous");
    const next=document.getElementById("article-find-next"),closeButton=document.getElementById("article-find-close");
    const normal={fg:0,bg:11,bright:false,inverse:false},selected={fg:15,bg:4,bright:false,inverse:false};
    let dead=false,opened=false,scope=null,indexFrame=null,index=[],matches=[],rows=new Map(),active=-1,version=0,timer=0,limited=false;
    const identity=packet=>packet?.navigation?.kind==="article_reading"&&packet.ptt_features?
        JSON.stringify([packet.generation,packet.article?.id??"terminal",packet.article?null:packet.frame.revision]):null;
    const visible=style=>style&&style.fg+(style.bright&&style.fg<8?8:0)!==style.bg;
    function labels(){
        root.setAttribute("aria-label",tr("article-find-title"));input.setAttribute("aria-label",tr("article-find-title"));
        input.placeholder=tr("article-find-placeholder");input.title=tr("article-find-hint");
        previous.setAttribute("aria-label",tr("article-find-previous"));next.setAttribute("aria-label",tr("article-find-next"));
        closeButton.setAttribute("aria-label",tr("article-find-close"));
        previous.title=tr("article-find-previous");next.title=tr("article-find-next");closeButton.title=tr("article-find-close");
        status.textContent=!input.value?tr("article-find-empty"):!matches.length?tr("article-find-none"):
            tr(limited?"article-find-count-limited":"article-find-count",{current:active+1,total:matches.length});
        previous.disabled=next.disabled=!matches.length;
    }
    function releaseIndex(){indexFrame=null;index=[];matches=[];rows.clear();active=-1;limited=false;version++;}
    function build(packet){
        if(indexFrame===packet.frame)return;
        indexFrame=packet.frame;index=[];
        const grouped=new Map(),end=packet.article?Math.min(packet.frame.rows-1,packet.article.last_line-packet.article.first_line+1):packet.frame.rows-1;
        for(const g of packet.frame.glyphs){
            if(g.row>=end)continue;
            if(!grouped.has(g.row))grouped.set(g.row,[]);grouped.get(g.row).push(g);
        }
        for(const [row,glyphs] of grouped){
            glyphs.sort((a,b)=>a.col-b.col);
            let text="",column=0;const spans=[];
            for(const g of glyphs){
                if(g.col>column){spans.push(text.length+g.col-column,column,g.col);text+=" ".repeat(g.col-column);}
                // Concealed cells break a match. Highlighting must not reveal
                // blacklisted text or turn invisible merged identities visible.
                const value=visible(g.left)&&(g.width===1||visible(g.right))?g.text.toLowerCase():"\0";
                spans.push(text.length+value.length,g.col,g.col+g.width);
                text+=value;column=g.col+g.width;
            }
            // Three integers per glyph/gap, rather than one object per cell.
            index.push({row,text,spans:new Uint32Array(spans)});
        }
        index.sort((a,b)=>a.row-b.row);
    }
    function search(packet,preserve=false){
        const old=preserve?matches[active]:null,query=input.value.slice(0,256).toLowerCase();
        matches=[];rows=new Map();active=-1;limited=false;
        if(!query||query.includes("\0")||!packet){releaseIndex();labels();return;}
        build(packet);
        outer:for(const line of index){
            const cell=offset=>{let lo=0,hi=line.spans.length/3;while(lo<hi){const mid=(lo+hi)>>>1;if(line.spans[mid*3]<=offset)lo=mid+1;else hi=mid;}return lo*3;};
            let from=0,found;
            while((found=line.text.indexOf(query,from))!==-1){
                if(matches.length===4096){limited=true;break outer;}
                const end=found+query.length;
                const a=cell(found),b=cell(end-1);
                if(b<line.spans.length){
                    const hit={row:line.row,start:line.spans[a+1],end:line.spans[b+2],index:matches.length};matches.push(hit);
                    if(!rows.has(line.row))rows.set(line.row,[]);rows.get(line.row).push(hit);
                }
                from=end;
            }
        }
        active=matches.length?Math.max(0,matches.findIndex(m=>old&&m.row===old.row&&m.start===old.start)):-1;
        version++;labels();
    }
    function update(packet){
        if(dead)return;
        const nextScope=identity(packet);
        if(nextScope!==scope){close(false);scope=nextScope;}
        if(opened&&input.value&&indexFrame!==packet.frame)search(packet,true);
    }
    function close(focus=true){
        clearTimeout(timer);timer=0;
        const wasOpen=opened;opened=false;root.hidden=true;input.value="";releaseIndex();
        if(wasOpen){if(focus)restore();changed();redraw();}
    }
    function open(event){
        if(dead||!event.isTrusted||event.isComposing||!identity(current())||!allowed())return false;
        if(!opened)prepare(event);scope=identity(current());opened=true;root.hidden=false;labels();
        input.focus({preventScroll:true});input.select();changed();return true;
    }
    function edit(event){
        if(dead||!opened||!event.isTrusted)return;
        clearTimeout(timer);timer=0;
        // Clear old highlights immediately; index/repaint only after the edit
        // settles, keeping long articles cheap while entering a query or IME.
        matches=[];rows.clear();active=-1;version++;labels();redraw();
        if(event.isComposing)return;
        timer=setTimeout(()=>{timer=0;if(dead||!opened)return;search(current());redraw();if(matches[active])jump(matches[active]);},120);
    }
    function step(back=false){
        if(dead||!opened)return;
        const pending=!!timer;
        if(timer){clearTimeout(timer);timer=0;search(current());}
        if(matches.length){if(!pending)active=(active+(back?-1:1)+matches.length)%matches.length;version++;labels();redraw();jump(matches[active]);}
    }
    function keys(event){
        if(dead||!event.isTrusted||event.isComposing)return;
        const shortcut=(!!event.metaKey!==!!event.ctrlKey)&&!event.altKey&&!event.shiftKey&&(event.key.toLowerCase()==="f"||event.code==="KeyF");
        if(shortcut){if(open(event)){event.preventDefault();event.stopPropagation();}return;}
        if(event.keyCode===229||!opened||!allowed()||event.ctrlKey||event.metaKey||event.altKey)return;
        if(event.key==="Escape"){event.preventDefault();event.stopPropagation();close();}
        else if(event.key==="Enter"&&root.contains(event.target)){event.preventDefault();event.stopPropagation();step(event.shiftKey);}
    }
    function click(action){return event=>{if(event.isTrusted&&!dead){event.preventDefault();action();}};}
    const bindings=[[document,"keydown",keys,true],[input,"input",edit],[input,"compositionend",edit],
        [previous,"click",click(()=>step(true))],[next,"click",click(()=>step())],[closeButton,"click",click(()=>close())]];
    return {bindings,update,close,open,isOpen:()=>opened,ownsFocus:()=>opened&&root.contains(document.activeElement),version:()=>version,relocalize:labels,
        clean:()=>dead&&!opened&&!timer&&indexFrame===null&&!index.length&&!matches.length&&!rows.size&&scope===null,
        highlighter(row=null){
            if(!opened||!matches.length)return null;
            return glyph=>{let hit=false;for(const match of rows.get(row??glyph.row)||[]){
                if(match.start<glyph.col+glyph.width&&match.end>glyph.col){if(match.index===active)return selected;hit=true;}
            }return hit?normal:null;};
        },destroy(){if(dead)return;dead=true;close(false);scope=null;}};
}
