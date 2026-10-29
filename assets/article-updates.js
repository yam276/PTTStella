// UI owns viewport geometry and unread presentation; Rust owns refresh work.
function createArticleUpdates({root,status,toggle,fresh,tr,current,send,bottom,position,move,restore,changed}) {
    let state=null,scope=null,total=0,seen=0,disposed=false,paused=null,tracking=false;
    const identity=p=>p?.article?`${p.generation}:${p.article.id}`:null;
    const matches=()=>state&&identity(current())===`${state.generation}:${state.article}`;
    function refresh(reason=paused){
        paused=reason;
        const p=current(),article=p?.article,same=matches(),enabled=same&&state.enabled;
        root.hidden=disposed||!article||!p.connected;
        if(root.hidden)return;
        toggle.hidden=!(same&&state.available)&&!enabled;
        toggle.textContent=tr(enabled?"article-updates-pause":"article-updates-start");
        toggle.setAttribute("aria-pressed",String(!!enabled));
        status.textContent=tr(same&&state.failed?"article-updates-stopped":enabled?(state.busy?"article-updates-refreshing":reason?"article-updates-paused":"article-updates-waiting"):"article-updates-off");
        const unread=Math.max(0,total-seen);fresh.hidden=!unread;
        fresh.textContent=tr("article-updates-new",{count:unread});
    }
    function receive(value){
        if(disposed)return;
        if(value!==null&&(!Number.isSafeInteger(value?.generation)||!Number.isSafeInteger(value?.article)||typeof value.available!=="boolean"||typeof value.enabled!=="boolean"||typeof value.busy!=="boolean"||typeof value.failed!=="boolean"))return;
        if(value&&state&&(value.generation<state.generation||value.generation===state.generation&&value.article<state.article))return;
        state=value;if(matches()&&state.enabled)tracking=true;refresh();changed();
    }
    function toggleUpdates(e){
        const p=current();if(disposed||!e.isTrusted||!p?.connected||!p.article)return;
        const enabled=!!(matches()&&state.enabled);
        if(!enabled&&!(matches()&&state.available))return;
        send({kind:"article_updates",generation:p.generation,article:p.article.id,enabled:!enabled,trusted:true});restore();
    }
    function beforePaint(packet){return identity(packet)===scope&&matches()&&state.enabled&&(packet.article?.reply_count??0)>total&&Math.abs(position()-bottom())<=2;}
    function afterPaint(packet,follow){
        const next=identity(packet),count=packet.article?.reply_count??0;
        if(next!==scope){scope=next;total=count;seen=count;tracking=!!(matches()&&state.enabled);}else{total=count;seen=tracking?Math.min(seen,total):total;}
        if(follow){move(bottom());seen=total;}
        refresh();
    }
    function scroll(){if(disposed)return;if(Math.abs(position()-bottom())<=2){seen=total;refresh();}}
    function jump(e){if(disposed||!e.isTrusted||identity(current())!==scope)return;move(bottom());seen=total;refresh();restore();}
    return {receive,refresh,beforePaint,afterPaint,scroll,bindings:[[toggle,"click",toggleUpdates],[fresh,"click",jump]],destroy(){disposed=true;state=null;scope=null;total=seen=0;root.hidden=true;}};
}
