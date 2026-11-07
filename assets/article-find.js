// Rust owns the filtered text index and matching. This adapter owns DOM focus,
// trusted gestures, edit debounce and display-only colours from typed hits.
function createArticleFind({document,tr,current,allowed,prepare,restore,redraw,jump,changed,send}) {
    const root=document.getElementById("article-find"),input=document.getElementById("article-find-input");
    const status=document.getElementById("article-find-status"),previous=document.getElementById("article-find-previous");
    const next=document.getElementById("article-find-next"),closeButton=document.getElementById("article-find-close");
    const normal={fg:0,bg:11,bright:false,inverse:false},selected={fg:15,bg:4,bright:false,inverse:false};
    let dead=false,opened=false,scope=null,key=null,matches=[],rows=new Map(),active=-1,version=0;
    let timer=0,limited=false,request=0,received=0,dataVersion=0,composing=false;
    const identity=p=>p?.navigation?.kind==="article_reading"&&p.ptt_features?
        JSON.stringify([p.generation,p.article?.id??"terminal",p.article?null:p.frame.revision]):null;
    const source=p=>p?JSON.stringify([p.generation,p.frame.revision,p.presentation??0,p.input_epoch??0,p.article?.id??null]):null;
    function labels(){
        root.setAttribute("aria-label",tr("article-find-title"));input.setAttribute("aria-label",tr("article-find-title"));
        input.placeholder=tr("article-find-placeholder");input.title=tr("article-find-hint");
        for(const [node,id] of [[previous,"previous"],[next,"next"],[closeButton,"close"]]){
            node.setAttribute("aria-label",tr("article-find-"+id));node.title=tr("article-find-"+id);
        }
        status.textContent=!input.value?tr("article-find-empty"):!matches.length?tr("article-find-none"):
            tr(limited?"article-find-count-limited":"article-find-count",{current:active+1,total:matches.length});
        previous.disabled=next.disabled=!matches.length;
    }
    function clear(){matches=[];rows.clear();active=-1;limited=false;dataVersion=0;version++;labels();}
    function command(command){
        const p=current();if(dead||!p)return;
        send({kind:"article_find",presentation:p.presentation??0,
            stamp:{generation:p.generation,revision:p.frame.revision,input_epoch:p.input_epoch??0},
            request:++request,command,trusted:true});
    }
    function query(){
        timer=0;if(dead||!opened||composing)return;
        // DOM maxlength is UTF-16. Do not serialize a lone surrogate if a host
        // clipped a supplementary character at that boundary.
        let value="",units=0;
        for(const c of input.value){if(c.length===1&&c.charCodeAt(0)>=0xd800&&c.charCodeAt(0)<=0xdfff)continue;
            if(units+c.length>256)break;value+=c;units+=c.length;}
        command({action:"edit",query:value});
    }
    function queue(){clearTimeout(timer);timer=0;clear();command({action:"pending"});
        if(!composing)timer=setTimeout(query,120);}
    function update(packet){
        if(dead)return;const nextScope=identity(packet),nextKey=source(packet);
        if(nextScope!==scope){close(false);scope=nextScope;}
        if(opened&&nextKey!==key){key=nextKey;clear();redraw();}
    }
    function close(focus=true,notify=true){
        clearTimeout(timer);timer=0;const wasOpen=opened;opened=false;root.hidden=true;input.value="";composing=false;clear();
        if(wasOpen){if(notify)command({action:"close"});if(focus)restore();changed();redraw();}
    }
    function open(event){
        if(dead||!event.isTrusted||event.isComposing||!identity(current())||!allowed())return false;
        if(!opened){prepare(event);scope=identity(current());key=source(current());opened=true;root.hidden=false;command({action:"open"});}
        labels();input.focus({preventScroll:true});input.select();changed();return true;
    }
    function edit(event){
        if(dead||!opened||!event.isTrusted)return;
        composing=!!event.isComposing;queue();redraw();
    }
    function step(back=false){
        if(dead||!opened||composing)return;
        if(timer){clearTimeout(timer);query();}else command({action:"step",back});
    }
    function receive(data){
        if(dead||!opened||data.request!==request||!Number.isSafeInteger(data.version)||data.version<=received)return;
        received=data.version;
        if(!data.opened){close(false,false);return;}
        const s=data.source,p=current();
        if(!s||source(p)!==JSON.stringify([s.generation,s.revision,s.presentation,s.input_epoch,s.article])){clear();redraw();return;}
        if(data.hits!==null&&data.hits!==undefined){
            if(!Array.isArray(data.hits)||data.hits.length>4096||data.hits.some((h,i)=>h.index!==i
                ||![h.row,h.start,h.end].every(Number.isSafeInteger)||h.row<0||h.row>=p.frame.rows-1||h.start<0||h.end<=h.start||h.end>p.frame.cols))return;
            matches=data.hits;rows=new Map();for(const hit of matches){if(!rows.has(hit.row))rows.set(hit.row,[]);rows.get(hit.row).push(hit);}
            dataVersion=data.data_version;
        }else if(dataVersion!==data.data_version)return;
        active=Number.isInteger(data.active)&&data.active>=0&&data.active<matches.length?data.active:-1;
        limited=!!data.limited;version++;labels();redraw();if(data.jump&&matches[active])jump(matches[active]);
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
    return {bindings,update,receive,close,open,isOpen:()=>opened,ownsFocus:()=>opened&&root.contains(document.activeElement),version:()=>version,relocalize:labels,
        clean:()=>dead&&!opened&&!timer&&!matches.length&&!rows.size&&scope===null,
        highlighter(row=null){if(!opened||!matches.length)return null;
            return glyph=>{let hit=false;for(const match of rows.get(row??glyph.row)||[]){
                if(match.start<glyph.col+glyph.width&&match.end>glyph.col){if(match.index===active)return selected;hit=true;}
            }return hit?normal:null;};
        },destroy(){if(dead)return;close(false);dead=true;scope=null;}};
}
