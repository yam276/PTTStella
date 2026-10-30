// UI owns viewport geometry and unread presentation; Rust owns refresh work.
function createArticleUpdates({root,status,toggle,fresh,station,menuToggle,tr,current,send,bottom,position,move,restore,changed}) {
    let state=null,scope=null,total=0,seen=0,disposed=false,paused=null,tracking=false;
    const identity=p=>p?.article?`${p.generation}:${p.article.id}`:null;
    const matches=()=>state&&identity(current())===`${state.generation}:${state.article}`;
    function refresh(reason=paused){
        paused=reason;
        const p=current(),article=p?.article,same=matches(),enabled=same&&state.enabled;
        const unread=Math.max(0,total-seen);
        root.hidden=disposed||!article||!p.connected||!(enabled||same&&(state.failed||state.recovery)||unread);
        if(menuToggle)menuToggle.textContent=tr(enabled?"article-updates-pause":"article-updates-start");
        station.hidden=!(same&&state.recovery);
        toggle.hidden=!(same&&state.available)&&!enabled;
        toggle.textContent=tr(enabled?"article-updates-pause":"article-updates-start");
        toggle.setAttribute("aria-pressed",String(!!enabled));
        status.hidden=!(enabled||same&&(state.failed||state.recovery));
        status.textContent=status.hidden?"":tr(same&&state.recovery?"article-updates-retained":same&&state.failed?"article-updates-stopped":state.busy?"article-updates-refreshing":reason?"article-updates-paused":"article-updates-waiting");
        fresh.hidden=!unread;
        fresh.textContent=tr("article-updates-new",{count:unread});
    }
    function receive(value){
        if(disposed)return;
        if(value!==null&&(!Number.isSafeInteger(value?.generation)||!Number.isSafeInteger(value?.article)||typeof value.available!=="boolean"||typeof value.enabled!=="boolean"||typeof value.busy!=="boolean"||typeof value.failed!=="boolean"||typeof value.recovery!=="boolean"))return;
        if(value&&state&&(value.generation<state.generation||value.generation===state.generation&&value.article<state.article))return;
        state=value;if(matches()&&state.enabled)tracking=true;refresh();changed();
    }
    function toggleUpdates(e){
        const p=current();if(disposed||!e.isTrusted||!p?.connected||!p.article)return;
        const enabled=!!(matches()&&state.enabled);
        if(!enabled&&!(matches()&&state.available))return;
        send({kind:"article_updates",generation:p.generation,article:p.article.id,enabled:!enabled,trusted:true});restore();
    }
    function afterPaint(packet){
        const next=identity(packet),count=packet.article?.reply_count??0;
        if(next!==scope){scope=next;total=count;seen=count;tracking=!!(matches()&&state.enabled);}else{total=count;seen=tracking?Math.min(seen,total):total;}
        refresh();
    }
    function scroll(){if(disposed)return;if(Math.abs(position()-bottom())<=2){seen=total;refresh();}}
    function jump(e){if(disposed||!e.isTrusted||identity(current())!==scope)return;move(bottom());seen=total;refresh();restore();}
    function showStation(e){const p=current();if(disposed||!e.isTrusted||!matches()||!state.recovery)return;send({kind:"article_station_restore",generation:p.generation,article:p.article.id,trusted:true});restore();}
    return {receive,refresh,afterPaint,scroll,toggle:toggleUpdates,available:()=>!disposed&&!!matches()&&(state.available||state.enabled),bindings:[[toggle,"click",toggleUpdates],[fresh,"click",jump],[station,"click",showStation]],destroy(){disposed=true;state=null;scope=null;total=seen=0;root.hidden=true;}};
}
