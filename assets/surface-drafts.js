// Full local source lives here only while editing; Rust owns disk and station bytes.
// U+241B makes ESC visible. Doubling preserves a literal U+241B reversibly.
function draftDisplay(source){let out="";for(let i=0;i<source.length;i++){const ch=source[i];out+=ch==="␛"?"␛␛":ch==="\x1b"?(source[i+1]==="["?"␛":"␛e"):ch;}return out;}
function draftSource(display){let out="";for(let i=0;i<display.length;i++){if(display[i]!=="␛"){out+=display[i];continue;}if(display[i+1]==="␛"){out+="␛";i++;}else {out+="\x1b";if(display[i+1]==="e")i++;}}return out;}
function createDraftEditor({document,window,tr,send,current,prepare,restore,changed,render,palette,notice}){
    const node=id=>document.getElementById("draft-"+id),root=node("dialog"),title=node("title"),body=node("body"),list=node("list"),note=node("notice");
    const preview=node("preview-panel"),colors=node("color-panel"),confirm=node("confirm"),bar=node("station-bar");
    let disposed=false,token=null,scope=null,sequence=0,request=0,opening=null,saving=false,dirty=false,composing=false,timer=0,first=0,foreground=15,background=0,colorRange=null,entries=[],awaiting=false,committed=null;
    const buttons=["restore","delete","new","native","preview","colors","save","begin","transfer","color-apply","prev","next"];
    function status(key){note.textContent=tr(key);}
    function cancelTimer(){if(timer)window.clearTimeout(timer);timer=0;}
    function editable(){return token!==null&&!disposed&&root.open&&!composing&&!awaiting;}
    function repaint(){
        const busy=saving||dirty||awaiting||opening!==null;
        for(const id of buttons)node(id).disabled=busy||composing;
        node("save").disabled=saving||awaiting||composing||opening!==null;
        node("close").disabled=awaiting||composing;
        node("restore").disabled=busy||!list.value;node("delete").disabled=busy||!list.value;
        node("transfer").hidden=current()?.draft_post_token!==token;
        node("begin").hidden=current()?.draft_post_token===token;
        node("begin").disabled=busy||!current()?.draft_scope||!body.value.trim();
        node("transfer").disabled=busy||!current()?.connected||!current()?.editor||!body.value.trim();
        bar.hidden=!current()?.connected||!current()?.draft_post_token||!current()?.editor||root.open;
        node("transfer-confirm").disabled=busy||composing||!current()?.connected||!current()?.editor||current()?.draft_post_token!==token;
    }
    function snapshot(){if(token===null||!dirty&&!saving)return null;return {token,sequence:sequence+1,title:composing?committed.title:title.value,body:composing?committed.body:draftSource(body.value)};}
    function flush(){
        cancelTimer();if(disposed||token===null||composing||!dirty)return;
        const source=draftSource(body.value);
        if(new TextEncoder().encode(source).length>128*1024||source.split("\n").length>4096||new TextEncoder().encode(title.value).length>256){status("draft-limit");return;}
        if(saving)return;
        saving=true;dirty=false;sequence++;status("draft-saving");repaint();
        send({kind:"draft_edit",edit:{token,sequence,title:title.value,body:source},trusted:true});
    }
    function edited(event){
        if(disposed||!event.isTrusted||!root.open||token===null)return;
        if(!composing&&!event.isComposing)committed={title:title.value,body:draftSource(body.value)};
        dirty=true;confirm.hidden=true;preview.hidden=true;body.hidden=false;node("preview").hidden=false;node("source").hidden=true;
        status("draft-unsaved");repaint();cancelTimer();if(!composing&&!event.isComposing)timer=window.setTimeout(flush,180);
    }
    function open(event){
        if(disposed||!event.isTrusted||document.querySelector("dialog[open]"))return;
        if(token!==null&&(saving||dirty)){root.showModal();repaint();changed();body.focus();return;}
        if(awaiting||opening!==null){notice(tr("draft-save-first"));return;}
        prepare(event);opening=++request;scope=null;token=null;entries=[];node("editor").hidden=true;node("chooser").hidden=true;node("delete-panel").hidden=true;confirm.hidden=true;status("draft-loading");root.showModal();changed();repaint();
        send({kind:"draft_open",request:opening,trusted:true});
    }
    function close(event){if(disposed||!event.isTrusted||composing||awaiting)return;event.preventDefault();flush();if(dirty){status("draft-save-first");return;}root.close();confirm.hidden=true;colors.hidden=true;restore();changed();repaint();}
    function choose(event,fresh=false){if(disposed||!event.isTrusted||opening!==null||saving||dirty||awaiting)return;awaiting=true;repaint();send({kind:"draft_choose",id:fresh?null:list.value,trusted:true});}
    function receive(reply){
        if(disposed)return;
        if(reply.state==="list"){
            if(opening!==reply.request)return;opening=null;awaiting=false;if(!root.open)return;scope=reply.scope;entries=reply.entries;
            node("scope").textContent=scope.account+" · "+scope.board;
            list.replaceChildren(...entries.map(entry=>{const option=document.createElement("option");option.value=entry.id;option.textContent=(entry.title||tr("draft-untitled"))+" · "+new Date(entry.saved_at*1000).toLocaleString()+(entry.conflict?" · "+tr("draft-conflict-copy"):"");return option;}));
            node("chooser").hidden=false;status(entries.length?"draft-found":"draft-empty");if(entries.length){list.value=entries[0].id;list.focus();}else choose({isTrusted:true},true);
        }else if(reply.state==="deleted"){
            awaiting=false;entries=entries.filter(e=>e.id!==reply.id);for(const option of [...list.children])if(option.value===reply.id)option.remove();list.value=entries[0]?.id??"";node("delete-panel").hidden=true;status("draft-deleted");
        }else if(reply.state==="selected"){
            if(!root.open)return;token=reply.token;sequence=0;scope=reply.entry.scope;awaiting=false;saving=false;dirty=false;composing=false;
            title.value=reply.entry.title;body.value=draftDisplay(reply.entry.body);committed={title:title.value,body:reply.entry.body};first=0;body.hidden=false;preview.hidden=true;colors.hidden=true;confirm.hidden=true;node("source").hidden=true;node("preview").hidden=false;
            node("chooser").hidden=true;node("editor").hidden=false;status(reply.entry.revision?"draft-saved":"draft-local");title.focus();
        }else if(reply.state==="saved"){
            if(reply.token!==token||reply.sequence!==sequence)return;saving=false;status(reply.conflicted?"draft-conflict-saved":"draft-saved");
            if(dirty)flush();
        }else if(reply.state==="colored"){
            if(reply.token!==token)return;awaiting=false;body.value=draftDisplay(reply.body);colors.hidden=true;colorRange=null;body.focus();edited({isTrusted:true});
        }else if(reply.state==="transferred"){
            awaiting=false;if(root.open)root.close();notice(reply.message);restore();changed();
        }else if(reply.state==="error"){
            const failedSave=saving;saving=false;awaiting=false;opening=null;if(failedSave)dirty=true;note.textContent=reply.message;if(!root.open)notice(reply.message);
        }
        repaint();
    }
    function showPreview(event){if(!event.isTrusted||!editable()||dirty||saving)return;first=0;requestPreview();}
    function requestPreview(){awaiting=true;repaint();send({kind:"draft_preview",token,first,trusted:true});}
    function receivePreview(reply){if(disposed||!root.open||reply.token!==token)return;awaiting=false;body.hidden=true;preview.hidden=false;node("source").hidden=false;node("preview").hidden=true;colors.hidden=true;
        render(node("canvas"),reply.packet,{cursor:false});const total=draftSource(body.value).split("\n").length;
        node("position").textContent=(reply.first+1)+"–"+Math.min(total,reply.first+23)+" / "+total;first=reply.first;repaint();node("prev").disabled=first===0;node("next").disabled=first+23>=total;
    }
    function changePage(event,delta){if(!event.isTrusted||!editable()||saving||dirty)return;first=Math.max(0,first+delta);requestPreview();}
    function source(event){if(!event.isTrusted||!editable())return;body.hidden=false;preview.hidden=true;node("source").hidden=true;node("preview").hidden=false;body.focus();}
    function station(event,transfer=false){
        if(!event.isTrusted||!editable()||saving||dirty)return;
        const p=current();if(!p?.connected||p.input_pending||p.output_pending)return;
        if(transfer&&p.draft_post_token!==token||!transfer&&!p.draft_scope)return;
        if(transfer){confirm.hidden=false;repaint();node("transfer-confirm").focus();return;}
        awaiting=true;repaint();send({kind:"draft_station",token,stamp:{generation:p.generation,revision:p.frame.revision,input_epoch:p.input_epoch},transfer:false,trusted:true});
    }
    function transfer(event){if(!event.isTrusted||!editable()||dirty||saving||confirm.hidden)return;const p=current();if(!p?.editor||p.draft_post_token!==token)return;awaiting=true;repaint();send({kind:"draft_station",token,stamp:{generation:p.generation,revision:p.frame.revision,input_epoch:p.input_epoch},transfer:true,trusted:true});}
    function native(event){if(!event.isTrusted||opening!==null||saving||dirty||awaiting||!current()?.draft_scope)return;const p=current();root.close();restore();changed();send({kind:"draft_native",stamp:{generation:p.generation,revision:p.frame.revision,input_epoch:p.input_epoch},trusted:true});}
    function openColors(event){if(!event.isTrusted||!editable()||saving||dirty||body.hidden)return;
        if(body.selectionStart===body.selectionEnd){status("editor-color-empty");return;}
        colorRange={start:draftSource(body.value.slice(0,body.selectionStart)).length,end:draftSource(body.value.slice(0,body.selectionEnd)).length};colors.hidden=!colors.hidden;
    }
    function colorApply(event){if(!event.isTrusted||!editable()||dirty||saving||!colorRange)return;awaiting=true;repaint();send({kind:"draft_color",token,...colorRange,color:{foreground,background},trusted:true});}
    const swatches=[];
    for(const [group,count]of [["foreground",16],["background",8]])for(let i=0;i<count;i++){
        const button=document.createElement("button");button.type="button";button.dataset.group=group;button.dataset.color=String(i);button.textContent=(group==="foreground"?(i>=8?"1;":"")+(30+i%8):40+i).toString();button.style.backgroundColor=palette[i];button.style.color=[0,1,4,5].includes(i%8)?"#fff":"#000";button.setAttribute("aria-label",tr("editor-color-name-"+(i%8))+(i>=8?" · "+tr("editor-color-bright"):"")+" "+button.textContent);node(group).append(button);swatches.push(button);
    }
    function paintColors(){for(const b of swatches)b.setAttribute("aria-pressed",String(Number(b.dataset.color)===(b.dataset.group==="foreground"?foreground:background)));}paintColors();
    function swatch(event){if(!event.isTrusted||!editable())return;const b=event.target.closest("button");if(!swatches.includes(b))return;if(b.dataset.group==="foreground")foreground=Number(b.dataset.color);else background=Number(b.dataset.color);paintColors();}
    function paste(event){if(disposed||!event.isTrusted||!editable()||!event.clipboardData)return;event.preventDefault();body.setRangeText(draftDisplay(event.clipboardData.getData("text/plain").replaceAll("\r\n","\n").replaceAll("\r","\n")),body.selectionStart,body.selectionEnd,"end");edited(event);}
    function key(event){event.stopPropagation();if(event.isComposing||composing||event.keyCode===229)return;if(event.key==="Escape"){event.preventDefault();close(event);}else if((event.ctrlKey||event.metaKey)&&event.key.toLowerCase()==="s"){event.preventDefault();if(event.isTrusted){dirty=true;flush();}}}
    function update(){if(disposed)return;const p=current();if(!p?.editor||p?.draft_post_token!==token)confirm.hidden=true;repaint();}
    function destroy(){cancelTimer();disposed=true;opening=null;saving=false;dirty=false;awaiting=false;token=null;scope=null;entries=[];colorRange=null;committed=null;title.value="";body.value="";list.replaceChildren();for(const group of ["foreground","background"])node(group).replaceChildren();if(root.open)root.close();bar.hidden=true;}
    return {open,receive,receivePreview,update,snapshot,destroy,clean:()=>disposed&&!timer&&!root.open&&!token&&!body.value&&!title.value,
        bindings:[[node("open"),"click",open],[node("station-open"),"click",event=>{if(event.isTrusted&&token!==null&&!root.open){root.showModal();repaint();changed();body.focus();}}],
            [node("close"),"click",close],[root,"cancel",close],[root,"keydown",key],[list,"change",repaint],[node("restore"),"click",e=>choose(e)],[node("new"),"click",e=>choose(e,true)],[node("native"),"click",native],
            [node("delete"),"click",event=>{if(event.isTrusted&&!saving&&!dirty&&!awaiting&&list.value){node("delete-panel").hidden=false;node("delete-panel").dataset.id=list.value;}}],
            [node("delete-confirm"),"click",event=>{if(event.isTrusted&&!saving&&!dirty&&!awaiting&&!node("delete-panel").hidden){awaiting=true;repaint();send({kind:"draft_delete",id:node("delete-panel").dataset.id,trusted:true});}}],
            [node("delete-cancel"),"click",event=>{if(event.isTrusted)node("delete-panel").hidden=true;}],
            [title,"input",edited],[body,"input",edited],[body,"paste",paste],[root,"compositionstart",()=>{committed={title:title.value,body:draftSource(body.value)};composing=true;cancelTimer();repaint();}],[root,"compositionend",event=>{composing=false;edited(event);}],[root,"focusout",flush],
            [node("save"),"click",e=>{if(e.isTrusted){dirty=true;flush();}}],[node("preview"),"click",showPreview],[node("source"),"click",source],[node("prev"),"click",e=>changePage(e,-23)],[node("next"),"click",e=>changePage(e,23)],
            [node("begin"),"click",e=>station(e)],[node("transfer"),"click",e=>station(e,true)],[node("transfer-confirm"),"click",transfer],[node("transfer-cancel"),"click",e=>{if(e.isTrusted)confirm.hidden=true;}],
            [node("colors"),"click",openColors],[colors,"click",swatch],[node("color-apply"),"click",colorApply],[window,"blur",flush],[document,"visibilitychange",()=>{if(document.visibilityState==="hidden")flush();}]]};
}
