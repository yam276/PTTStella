// Owns station editor drafts and request lifetime. Rust validates targets and owns persistence.
function createSitesDialog({tr,send,prepare,restore,changed}) {
    const button=document.getElementById("sites-open"),dialog=document.getElementById("sites-dialog");
    const list=document.getElementById("sites-list"),name=document.getElementById("site-name"),address=document.getElementById("site-address"),encoding=document.getElementById("site-encoding");
    const favourite=document.getElementById("site-save"),remove=document.getElementById("site-remove"),makeDefault=document.getElementById("site-default"),connect=document.getElementById("site-connect"),closeButton=document.getElementById("sites-close");
    const note=document.getElementById("sites-note"),defaultNote=document.getElementById("sites-default"),security=document.getElementById("sites-transport");
    let disposed=false,sequence=0,pending=null,opening=null,writable=false,composing=false,entries=[];
    const encodingName=value=>({uao:"Big5-UAO",big5:"Big5",gbk:"GBK"})[value]??value;
    const description=target=>tr("sites-target",{address:target.endpoint,encoding:encodingName(target.encoding)});
    function controls(){
        const busy=pending!==null||opening!==null;
        for(const field of [list,name,address,encoding,closeButton])field.disabled=busy;
        favourite.disabled=busy||!writable;makeDefault.disabled=busy||!writable;
        remove.disabled=busy||!writable||list.value==="";connect.disabled=busy||!address.value.trim();
        // Address validation remains in Rust; this is only a pre-connection transport explanation.
        security.textContent=/^wss:\/\//i.test(address.value.trim())||["https://term.ptt.cc","https://term.ptt2.cc","https://term.gamer.com.tw"].includes(address.value.trim().replace(/\/$/,""))?tr("sites-encrypted"):tr("sites-unencrypted");
    }
    function fill(target,label=""){address.value=target.endpoint;encoding.value=target.encoding;name.value=label;controls();}
    function snapshot(data){
        const previous=list.value!==""?entries[Number(list.value)]?.target:null;entries=data.sites.favourites;writable=data.writable;
        const placeholder=document.createElement("option");placeholder.value="";placeholder.textContent=tr("sites-manual");
        list.replaceChildren(placeholder,...entries.map((entry,index)=>{
            const option=document.createElement("option");option.value=String(index);option.textContent=entry.name+" — "+description(entry.target);return option;
        }));
        const selected=previous?entries.findIndex(entry=>entry.target.endpoint===previous.endpoint&&entry.target.encoding===previous.encoding):-1;
        list.value=selected>=0?String(selected):"";
        defaultNote.textContent=tr("sites-default-value",{target:description(data.sites.default)});controls();
    }
    function open(e){
        if(disposed||!e.isTrusted||document.querySelector("dialog[open]"))return;
        prepare(e);opening=++sequence;pending=null;composing=false;list.value="";note.textContent=tr("sites-loading");
        dialog.showModal();controls();changed();send({kind:"sites_open",request:opening,trusted:true});
    }
    function close(e){
        e?.preventDefault();if(disposed||!e?.isTrusted||pending!==null||opening!==null)return;
        if(dialog.open)dialog.close();composing=false;restore();changed();
    }
    function choose(e){
        if(disposed||!e.isTrusted||pending!==null||opening!==null)return;
        const entry=list.value!==""?entries[Number(list.value)]:null;
        fill(entry?.target??{endpoint:"",encoding:"uao"},entry?.name??"");
    }
    function edit(action,e){
        if(disposed||!e.isTrusted||!dialog.open||pending!==null||opening!==null||composing||!writable)return;
        const entry=action==="remove"&&list.value!==""?entries[Number(list.value)]:null;
        if(action==="remove"&&!entry)return;
        const target=entry?.target??{endpoint:address.value,encoding:encoding.value};
        pending=++sequence;controls();note.textContent=tr("saving");
        send({kind:"sites_edit",request:pending,action,address:target.endpoint,encoding:target.encoding,name:name.value,trusted:true});
    }
    function go(e){
        if(disposed||!e.isTrusted||!dialog.open||pending!==null||opening!==null||composing||!address.value.trim())return;
        pending=++sequence;controls();note.textContent=tr("sites-connecting");
        send({kind:"sites_connect",request:pending,address:address.value,encoding:encoding.value,trusted:true});
    }
    function receiveOpen(packet){
        if(disposed||!dialog.open||packet.request!==opening)return;
        opening=null;snapshot(packet.data);fill(packet.data.current);note.textContent=packet.data.warning??"";address.focus();
    }
    function receive(packet){
        if(disposed||!dialog.open||packet.request!==pending)return;
        pending=null;snapshot(packet.data);note.textContent=packet.feedback.notice??packet.data.warning??"";
        if(packet.feedback.status==="connecting"){dialog.close();restore();changed();}else address.focus();
    }
    function revoke(){disposed=true;opening=null;pending=null;composing=false;entries=[];address.value="";name.value="";}
    function destroy(){revoke();if(dialog.open)dialog.close();}
    return {request:open,receiveOpen,receive,revoke,destroy,clean:()=>disposed&&!dialog.open&&!pending&&!opening,
        bindings:[[button,"click",open],[list,"change",choose],[address,"input",controls],[encoding,"change",controls],
            [favourite,"click",e=>edit("favourite",e)],[remove,"click",e=>edit("remove",e)],[makeDefault,"click",e=>edit("default",e)],
            [connect,"click",go],[closeButton,"click",close],[dialog,"cancel",close],
            ...[name,address].flatMap(field=>[[field,"compositionstart",()=>{composing=true;}],[field,"compositionend",()=>{composing=false;}]])],
    };
}
