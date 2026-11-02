// Owns only the local editor/request lifetime. Rust owns validation and disk commits.
function createBookmarksDialog({tr,send,prepare,restore,changed,openDestination,current,window=null,render=()=>{},notice=()=>{}}) {
    const button=document.getElementById("bookmarks-open"),dialog=document.getElementById("bookmarks-dialog");
    const list=document.getElementById("bookmarks-list"),value=document.getElementById("bookmarks-target"),label=document.getElementById("bookmarks-label");
    const save=document.getElementById("bookmarks-save"),remove=document.getElementById("bookmarks-remove"),fresh=document.getElementById("bookmarks-new"),go=document.getElementById("bookmarks-go"),closeButton=document.getElementById("bookmarks-close"),note=document.getElementById("bookmarks-note");
    const currentButton=document.getElementById("bookmarks-current"),cancelCapture=document.getElementById("bookmarks-current-cancel"),readButton=document.getElementById("bookmarks-read");
    const editor=document.getElementById("bookmarks-editor"),reader=document.getElementById("bookmarks-reader"),canvas=document.getElementById("bookmarks-canvas");
    const first=document.getElementById("bookmarks-first"),previous=document.getElementById("bookmarks-prev"),next=document.getElementById("bookmarks-next"),last=document.getElementById("bookmarks-last"),back=document.getElementById("bookmarks-reader-back");
    const captureBar=document.getElementById("bookmarks-capture"),captureStatus=document.getElementById("bookmarks-capture-status");
    let capturePending=null,local=null,localRows=[],top=0,listTarget=null;
    let disposed=false,composing=false,sequence=0,view=0,opening=null,pending=null,entries=[],writable=false;
    const identity=entry=>"#"+entry.aid+"@"+entry.board;
    const listIdentity=p=>JSON.stringify([p?.generation,p?.frame?.revision,p?.input_epoch,p?.presentation]);
    function listRow(){
        const p=current();
        if(listTarget&&listTarget.identity!==listIdentity(p))return null;
        return listTarget?.row??p?.navigation?.cursor_row??null;
    }
    function listAvailable(row=listRow()){
        const p=current();return !disposed&&!capturePending&&!!p?.connected&&!!p?.ptt_features&&p?.mouse?.route==="local"&&p?.article_mode==="web"&&!p?.input_pending&&!p?.output_pending&&p?.navigation?.kind==="article_list"&&p.navigation.articles?.some(a=>a.row===row&&a.bookmark===true);
    }
    function available(){
        const packet=current(),article=packet?.article;
        return listAvailable()||!disposed&&!capturePending&&!!packet?.connected&&!!packet?.ptt_features&&packet?.mouse?.route==="local"&&article?.load==="complete"&&article?.first_line===1&&!!article?.end;
    }
    function clearListTarget(){listTarget=null;working();}
    function setListTarget(row){const p=current();if(disposed||capturePending||p?.navigation?.kind!=="article_list"||!p.navigation.articles?.some(a=>a.row===row))return false;listTarget={row,identity:listIdentity(p)};working();return listAvailable(row);}

    function working(){
        const busy=opening!==null||pending!==null;
        for(const node of [list,value,label,fresh])node.disabled=busy;
        save.disabled=busy||!writable;remove.disabled=busy||!writable||!list.value;
        go.disabled=busy||!value.value||current()?.destination?.state==="active";
        const selected=entries.find(e=>identity(e)===list.value);
        if(readButton)readButton.disabled=busy||!selected?.cached_at;
        if(currentButton){currentButton.disabled=!available();currentButton.textContent=tr(current()?.navigation?.kind==="article_list"?"bookmarks-list-current":"bookmarks-current");currentButton.title=tr(currentButton.disabled?"bookmarks-current-unavailable":current()?.navigation?.kind==="article_list"?"bookmarks-list-hint":"bookmarks-current");}
        if(captureBar)captureBar.hidden=!capturePending;
        if(captureStatus)captureStatus.textContent=capturePending?tr(capturePending.cancellable?"bookmarks-capturing":"saving"):"";
        if(cancelCapture){cancelCapture.hidden=!capturePending;cancelCapture.disabled=capturePending?.cancellable===false||!!capturePending?.cancelling;}

    }
    function rows(){
        const selected=list.value;
        list.replaceChildren(...entries.map(entry=>{
            const option=document.createElement("option");option.value=identity(entry);
            option.textContent=(entry.label?entry.label+" — ":"")+identity(entry)+(entry.cached_at?" · "+tr("bookmarks-local-copy"):"");
            option.selected=option.value===selected;return option;
        }));
        list.value=entries.some(entry=>identity(entry)===selected)?selected:"";
        working();
    }
    function open(e){
        if(disposed||!e.isTrusted||document.querySelector("dialog[open]"))return;
        leaveReader();prepare(e);view=++sequence;opening=view;value.value="";label.value="";list.value="";composing=false;
        note.textContent=tr(pending?"saving":"bookmarks-loading");dialog.showModal();working();changed();
        send({kind:"bookmarks_open",request:opening,trusted:true});
    }
    function close(e,restoreFocus=true){
        if(disposed||!e.isTrusted)return;e.preventDefault();opening=null;composing=false;
        leaveReader();value.value="";label.value="";list.value="";if(dialog.open)dialog.close();
        if(restoreFocus)restore();changed();
    }
    function choose(e){
        if(disposed||!e.isTrusted||pending||opening!==null)return;
        const entry=entries.find(entry=>identity(entry)===list.value);
        value.value=entry?identity(entry):"";label.value=entry?.label||"";working();
    }
    function newDraft(e){
        if(disposed||!e.isTrusted||pending||opening!==null)return;
        list.value="";value.value="";label.value="";working();value.focus();
    }
    function edit(e,removeEntry=false){
        if(disposed||!e.isTrusted||!dialog.open||opening!==null||pending||composing||!writable)return;
        if(removeEntry&&!list.value)return;
        e.preventDefault();pending={request:++sequence,view};working();note.textContent=tr("saving");
        send({kind:"bookmarks_edit",request:pending.request,value:removeEntry?list.value:value.value,label:label.value,remove:removeEntry,trusted:true});
    }
    function keys(e){
        if(e.key!=="Enter"||![value,label].includes(e.target)||e.isComposing||composing||e.keyCode===229)return;
        e.preventDefault();if(!e.repeat)edit(e);
    }
    function goTo(e){
        if(disposed||!e.isTrusted||!dialog.open||pending||opening!==null||composing||!value.value||current()?.destination?.state==="active")return;
        const draft=value.value;close(e,false);if(!openDestination(e,draft))restore();
    }
    function receiveOpen(packet){
        if(disposed||!dialog.open||packet.request!==opening)return;
        opening=null;entries=packet.data.entries;writable=packet.data.writable;rows();
        note.textContent=pending?tr("saving"):packet.data.warning||tr("bookmarks-hint");
        if(!pending)value.focus();
    }
    function receive(packet){
        if(disposed)return;
        if(capturePending?.request===packet.request){capturePending=null;entries=packet.data.entries;writable=packet.data.writable;if(dialog.open)rows();else working();notice(packet.feedback.notice||"");return;}
        if(!pending||packet.request!==pending.request)return;
        const sameView=pending.view===view;pending=null;
        entries=packet.data.entries;writable=packet.data.writable;
        if(!dialog.open)return;
        rows();note.textContent=packet.feedback.notice||packet.data.warning||"";
        if(sameView)value.focus();
    }
    function capture(e,row=null){
        if(disposed||!e.isTrusted||capturePending||document.querySelector("dialog[open]"))return;
        if(row!==null&&!setListTarget(row))return;
        const packet=current(),article=packet?.article,selected=listRow(),fromList=!article&&listAvailable(selected);
        working();if(!available()||!article&&!fromList){notice(tr("bookmarks-current-unavailable"));return;}
        prepare(e);restore();capturePending={request:++sequence,cancellable:true,list:fromList};working();
        const stamp={generation:packet.generation,revision:packet.frame.revision,input_epoch:packet.input_epoch};
        send(fromList?{kind:"bookmark_list",request:capturePending.request,stamp,row:selected,trusted:true}:{kind:"bookmark_current",request:capturePending.request,stamp,article:article.id,trusted:true});
    }
    function captureStage(packet){if(!disposed&&capturePending?.request===packet.request){capturePending.cancellable=packet.cancellable;working();}}
    function cancelCurrent(e){if(disposed||!e.isTrusted||!capturePending?.cancellable||capturePending.cancelling)return;capturePending.cancelling=true;working();send({kind:"bookmark_cancel",request:capturePending.request,trusted:true});restore();}
    function readLocal(e){
        if(disposed||!e.isTrusted||!dialog.open||pending||opening!==null||readButton.disabled)return;
        pending={request:++sequence,view,archive:true};working();note.textContent=tr("bookmarks-loading");
        send({kind:"archive_open",request:pending.request,value:list.value,trusted:true});
    }
    function leaveReader(){local=null;localRows=[];top=0;if(reader)reader.hidden=true;if(editor)editor.hidden=false;}
    function paintLocal(){
        if(!local||!dialog.open)return;
        const count=23,maximum=Math.max(0,local.frame.rows-count);top=Math.max(0,Math.min(top,maximum));
        const glyphs=localRows.slice(top,top+count).flat().map(g=>({...g,row:g.row-top}));
        render(canvas,{...local,frame:{...local.frame,rows:count,glyphs}},{cursor:false});
        document.getElementById("bookmarks-position").textContent=tr("bookmarks-lines",{first:top+1,last:Math.min(local.frame.rows,top+count),total:local.frame.rows});
        first.disabled=previous.disabled=top===0;next.disabled=last.disabled=top===maximum;
    }
    function receiveArchive(packet){
        if(disposed||!pending?.archive||pending.request!==packet.request)return;
        const valid=dialog.open&&pending.view===view;pending=null;working();if(!valid)return;
        if(!packet.article){note.textContent=packet.message||tr("bookmarks-no-archive");return;}
        local=packet.article;localRows=Array.from({length:local.frame.rows},()=>[]);
        for(const glyph of local.frame.glyphs)localRows[glyph.row].push(glyph);
        top=0;editor.hidden=true;reader.hidden=false;
        document.getElementById("bookmarks-archive-info").textContent=identity(local.target)+" · "+new Date(local.saved_at*1000).toLocaleString()+" · "+tr("bookmarks-archive-readonly");
        paintLocal();canvas.focus();changed();
    }
    function moveLocal(delta,e){if(disposed||!e.isTrusted||!local||!dialog.open)return;e.preventDefault();top=delta===Infinity?local.frame.rows:delta===-Infinity?0:top+delta;paintLocal();canvas.focus();}
    function localKeys(e){
        if(disposed||!local||!dialog.open)return;e.stopPropagation();
        if(e.isComposing||e.ctrlKey||e.metaKey||e.altKey)return;
        const delta={Home:-Infinity,End:Infinity,ArrowUp:-1,ArrowDown:1,PageUp:-22,PageDown:22,k:-1,j:1}[e.key];
        if(delta!==undefined){moveLocal(delta,e);}
    }
    function backToList(e){if(disposed||!e.isTrusted||!local)return;leaveReader();note.textContent=tr("bookmarks-hint");working();list.focus();}
    function revoke(){disposed=true;listTarget=null;opening=null;pending=null;capturePending=null;composing=false;entries=[];value.value="";label.value="";if(captureBar)captureBar.hidden=true;if(captureStatus)captureStatus.textContent="";leaveReader();}
    function destroy(){revoke();list.replaceChildren();if(dialog.open)dialog.close();}
    return {show:open,capture,available,listAvailable,setListTarget,clearListTarget,captureStage,receiveArchive,receiveOpen,receive,revoke,destroy,update:()=>{if(!disposed)working();},
        clean:()=>disposed&&!dialog.open&&!pending&&!capturePending&&!local&&!localRows.length&&opening===null&&!composing&&!entries.length&&!value.value&&!label.value,
        bindings:[[button,"click",open],[closeButton,"click",close],[dialog,"cancel",close],[list,"change",choose],
            [currentButton,"click",e=>capture(e)],
            [window,"blur",e=>{if(capturePending?.list&&capturePending.cancellable)cancelCurrent({isTrusted:true});}],
            [document,"visibilitychange",()=>{if(document.visibilityState==="hidden"&&capturePending?.list&&capturePending.cancellable)cancelCurrent({isTrusted:true});}],[cancelCapture,"click",cancelCurrent],[readButton,"click",readLocal],
            [first,"click",e=>moveLocal(-Infinity,e)],[previous,"click",e=>moveLocal(-22,e)],[next,"click",e=>moveLocal(22,e)],[last,"click",e=>moveLocal(Infinity,e)],[back,"click",backToList],
            [reader,"keydown",localKeys],[canvas,"wheel",e=>{if(e.deltaY&&!e.ctrlKey&&!e.metaKey)moveLocal(e.deltaY>0?3:-3,e);}],
            [fresh,"click",newDraft],[save,"click",e=>edit(e)],[remove,"click",e=>edit(e,true)],[go,"click",goTo],[value,"input",working],
            [dialog,"keydown",keys],[dialog,"compositionstart",()=>{if(!disposed)composing=true;}],[dialog,"compositionend",()=>{if(!disposed)composing=false;}]].filter(([node])=>node)};
}
