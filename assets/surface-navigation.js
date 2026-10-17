// Owns the local draft and request identity; Rust owns board names and navigation.
function createNavigationDialog({tr,send,stamp,prepare,restore,changed,current,isEditing}) {
    const openButton=document.getElementById("destination-open"),stopButton=document.getElementById("destination-stop");
    const dialog=document.getElementById("destination-dialog"),name=document.getElementById("destination-name");
    const submitButton=document.getElementById("destination-submit"),cancelButton=document.getElementById("destination-cancel"),note=document.getElementById("destination-note");
    const recentList=document.getElementById("board-recent"),recentEmpty=document.getElementById("board-recent-empty"),recentNote=document.getElementById("board-recent-note"),clearButton=document.getElementById("board-recent-clear");
    let disposed=false,composing=false,pending=null,active=null,sequence=0;
    function working(){const busy=!!pending||!!active;name.disabled=busy;submitButton.disabled=busy;for(const button of recentList.querySelectorAll("button"))button.disabled=busy;}
    function open(e,draft=""){
        if(disposed||!e.isTrusted||document.querySelector("dialog[open]")&&!dialog.open)return false;
        if(dialog.open){name.focus();return true;}
        prepare(e);name.value=active?(active.target.kind==="article"?"#"+active.target.aid+"@"+active.target.board:active.target.board):draft;note.textContent=active?tr(active.target.kind==="article"?"article-open-active":"board-switch-active",{board:active.target.board,aid:active.target.aid}):"";
        composing=false;dialog.showModal();working();changed();name.focus();
        return true;
    }
    function shortcut(e){
        if(disposed||!e.isTrusted||e.isComposing||composing||isEditing()||e.altKey||!(e.metaKey||e.ctrlKey)
            ||!(e.code==="KeyK"||e.key?.toLowerCase()==="k")||document.querySelector("dialog[open]")&&!dialog.open)return;
        e.preventDefault();e.stopPropagation();if(!e.repeat)open(e);
    }
    function submit(e){
        if(disposed||!e.isTrusted||!dialog.open||pending||active||composing)return;
        e.preventDefault();
        if(!current()?.connected){note.textContent=tr("destination-dialog-disconnected");return;}
        pending={request:++sequence,operation:stamp()};working();note.textContent=tr("destination-dialog-starting");
        send({kind:"destination_submit",request:pending.request,stamp:pending.operation,value:name.value,trusted:true});
    }
    function keys(e){
        if(e.target!==name||e.key!=="Enter"||e.isComposing||composing||e.keyCode===229||e.key==="Process")return;
        e.preventDefault();if(!e.repeat)submit(e);
    }
    function receiveRecent(packet){
        if(disposed)return;
        const rows=packet.boards.map(board=>{
            const item=document.createElement("li"),button=document.createElement("button");
            button.type="button";button.dataset.board=board;button.textContent=board;item.append(button);return item;
        });
        recentList.replaceChildren(...rows);recentEmpty.hidden=rows.length>0;recentNote.textContent=packet.notice||"";working();
    }
    function chooseRecent(e){
        if(disposed||!e.isTrusted||!dialog.open||pending||active||composing)return;
        const button=e.target.closest("button[data-board]");if(!button||!recentList.contains(button))return;
        e.preventDefault();name.value=button.dataset.board;note.textContent="";name.focus();
    }
    function clearRecent(e){if(disposed||!e.isTrusted||!dialog.open)return;e.preventDefault();send({kind:"recent_boards_clear",trusted:true});}
    function cancel(e){
        if(disposed||!e?.isTrusted)return;e.preventDefault();
        const operation=pending?.operation||active?.operation;
        if(operation)send({kind:"destination_cancel",operation,trusted:true});
        pending=null;name.value="";composing=false;if(dialog.open)dialog.close();restore();changed();
    }
    function stop(e){if(disposed||!e.isTrusted||!active)return;send({kind:"destination_cancel",operation:active.operation,trusted:true});restore();}
    function receive(result){
        if(disposed||!pending||result.request!==pending.request)return;
        pending=null;working();
        if(!dialog.open)return;
        if(result.feedback.status==="started"){name.value="";dialog.close();restore();changed();}
        else {note.textContent=result.feedback.notice||tr("destination-stale");name.focus();}
    }
    function update(packet){if(disposed)return;if(pending&&(!packet.connected||pending.operation.generation!==packet.generation)){pending=null;if(dialog.open)note.textContent=tr("destination-stale");}active=packet.destination?.state==="active"?packet.destination:null;stopButton.hidden=!active;working();}
    function revoke(){disposed=true;pending=null;active=null;composing=false;name.value="";}
    function destroy(){revoke();recentList.replaceChildren();recentNote.textContent="";if(dialog.open)dialog.close();}
    return {receive,receiveRecent,update,revoke,destroy,openDraft:(e,value)=>!active&&!pending&&open(e,value),clean:()=>disposed&&!dialog.open&&!pending&&!active&&!composing&&!name.value,
        bindings:[[recentList,"click",chooseRecent],[clearButton,"click",clearRecent],[openButton,"click",open],[stopButton,"click",stop],[submitButton,"click",submit],[cancelButton,"click",cancel],
            [dialog,"cancel",cancel],[dialog,"keydown",keys],[name,"compositionstart",()=>{composing=true;}],[name,"compositionend",()=>{composing=false;}],
            [document,"keydown",shortcut,{capture:true}]]};
}
