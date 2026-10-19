// Owns only the local editor/request lifetime. Rust owns validation and disk commits.
function createBookmarksDialog({tr,send,prepare,restore,changed,openDestination,current}) {
    const button=document.getElementById("bookmarks-open"),dialog=document.getElementById("bookmarks-dialog");
    const list=document.getElementById("bookmarks-list"),value=document.getElementById("bookmarks-target"),label=document.getElementById("bookmarks-label");
    const save=document.getElementById("bookmarks-save"),remove=document.getElementById("bookmarks-remove"),fresh=document.getElementById("bookmarks-new"),go=document.getElementById("bookmarks-go"),closeButton=document.getElementById("bookmarks-close"),note=document.getElementById("bookmarks-note");
    let disposed=false,composing=false,sequence=0,view=0,opening=null,pending=null,entries=[],writable=false;
    const identity=entry=>"#"+entry.aid+"@"+entry.board;
    function working(){
        const busy=opening!==null||pending!==null;
        for(const node of [list,value,label,fresh])node.disabled=busy;
        save.disabled=busy||!writable;remove.disabled=busy||!writable||!list.value;
        go.disabled=busy||!value.value||current()?.destination?.state==="active";
    }
    function rows(){
        const selected=list.value;
        list.replaceChildren(...entries.map(entry=>{
            const option=document.createElement("option");option.value=identity(entry);
            option.textContent=(entry.label?entry.label+" — ":"")+identity(entry);
            option.selected=option.value===selected;return option;
        }));
        list.value=entries.some(entry=>identity(entry)===selected)?selected:"";
        working();
    }
    function open(e){
        if(disposed||!e.isTrusted||document.querySelector("dialog[open]"))return;
        prepare(e);view=++sequence;opening=view;value.value="";label.value="";list.value="";composing=false;
        note.textContent=tr(pending?"saving":"bookmarks-loading");dialog.showModal();working();changed();
        send({kind:"bookmarks_open",request:opening,trusted:true});
    }
    function close(e,restoreFocus=true){
        if(disposed||!e.isTrusted)return;e.preventDefault();opening=null;composing=false;
        value.value="";label.value="";list.value="";if(dialog.open)dialog.close();
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
        if(disposed||!pending||packet.request!==pending.request)return;
        const sameView=pending.view===view;pending=null;
        entries=packet.data.entries;writable=packet.data.writable;
        if(!dialog.open)return;
        rows();note.textContent=packet.feedback.notice||packet.data.warning||"";
        if(sameView)value.focus();
    }
    function revoke(){disposed=true;opening=null;pending=null;composing=false;entries=[];value.value="";label.value="";}
    function destroy(){revoke();list.replaceChildren();if(dialog.open)dialog.close();}
    return {show:open,receiveOpen,receive,revoke,destroy,update:()=>{if(!disposed)working();},
        clean:()=>disposed&&!dialog.open&&!pending&&opening===null&&!composing&&!entries.length&&!value.value&&!label.value,
        bindings:[[button,"click",open],[closeButton,"click",close],[dialog,"cancel",close],[list,"change",choose],
            [fresh,"click",newDraft],[save,"click",e=>edit(e)],[remove,"click",e=>edit(e,true)],[go,"click",goTo],[value,"input",working],
            [dialog,"keydown",keys],[dialog,"compositionstart",()=>{if(!disposed)composing=true;}],[dialog,"compositionend",()=>{if(!disposed)composing=false;}]]};
}
