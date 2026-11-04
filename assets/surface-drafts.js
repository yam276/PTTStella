// Rust owns persistence and ANSI projection. This owner retains the live source,
// IME snapshot, one pending save/projection, and the local editor's lifetime.
function createDraftEditor({document,window,tr,send,current,prepare,restore,changed,palette,render,notice,createEditor=StellaComposer.create}){
    const node=id=>document.getElementById('draft-'+id),root=node('dialog'),title=node('title'),list=node('list'),note=node('notice'),colors=node('color-panel'),confirm=node('confirm'),bar=node('station-bar');
    let disposed=false,token=null,sequence=0,revision=0,request=0,opening=null,saving=false,dirty=false,composing=false,timer=0,projectTimer=0,projectPending=false,projectAgain=false,awaiting=false,colorPending=false,foreground=15,background=0,colorSelection=null,canCreate=false,entries=[],committed=null,lastEdit=0,lastSave=0,flushAfter=false;
    const now=()=>Date.now();
    let editor=null;
    function ensureEditor(){
        if(editor)return true;
        try {editor=createEditor({parent:node('body'),palette,label:tr('draft-body'),onChange:()=>edited({isTrusted:true}),onContext:openColors,graphicCode,renderGlyph:(canvas,text,data)=>render(canvas,{frame:{cols:data.width,rows:1,cursor_visible:false,glyphs:[{row:0,col:0,width:data.width,text,left:data.left,right:data.right}]},links:[],settings:current().settings},{cursor:false})});return true;}
        catch(error){console.error('PTTStella composer initialization',error);note.textContent=tr('draft-editor-unavailable');return false;}
    }
    function status(key){note.textContent=tr(key);}
    function source(){return editor?.value()??'';}
    function cancelTimer(){if(timer)window.clearTimeout(timer);timer=0;}
    function editable(){return token!==null&&!disposed&&root.open&&!composing&&!awaiting&&!colorPending;}
    function repaint(){
        const busy=saving||dirty||awaiting||opening!==null;
        for(const id of ['restore','delete','new'])node(id).disabled=busy||composing||!list.value&&id!=='new';
        node('new').disabled=busy||!canCreate;
        node('new').hidden=!canCreate;
        node('save').disabled=token===null||saving||awaiting||composing||opening!==null;
        node('close').disabled=composing;
        node('native').disabled=awaiting||opening!==null;
        node('transfer').disabled=token===null||busy||!current()?.connected||!current()?.editor||!current()?.draft_post_scope;
        node('transfer-confirm').disabled=token===null||busy||composing||!current()?.connected||!current()?.editor||!current()?.draft_post_scope;
        bar.hidden=!current()?.connected||!current()?.draft_post_scope||!current()?.editor||root.open;
        editor?.readOnly(awaiting||colorPending);
    }
    function snapshot(){if(token===null||!dirty&&!saving)return null;return {token,sequence:sequence+1,title:composing?committed.title:title.value,body:composing?committed.body:source()};}
    function schedule(){cancelTimer();if(dirty&&!composing)timer=window.setTimeout(()=>{timer=0;flush();},Math.max(2000-(now()-lastEdit),5000-(now()-lastSave),0));}
    function flush(){
        cancelTimer();if(disposed||token===null||composing||!dirty)return;
        const body=source();if(new TextEncoder().encode(body).length>128*1024||body.split('\n').length>4096||new TextEncoder().encode(title.value).length>256){status('draft-limit');return;}
        if(saving){flushAfter=true;return;}
        saving=true;dirty=false;sequence++;lastSave=now();status('draft-saving');repaint();
        send({kind:'draft_edit',edit:{token,sequence,title:title.value,body},trusted:true});
    }
    function project(){
        if(disposed||token===null)return;
        if(projectPending){projectAgain=true;return;}
        projectPending=true;send({kind:'draft_project',token,revision,body:source(),trusted:true});
    }
    function scheduleProject(){if(projectTimer)window.clearTimeout(projectTimer);projectTimer=window.setTimeout(()=>{projectTimer=0;project();},32);}
    function edited(event){
        if(disposed||!event.isTrusted||!root.open||token===null)return;
        if(!composing&&!event.isComposing)committed={title:title.value,body:source()};
        dirty=true;revision++;lastEdit=now();confirm.hidden=true;colors.hidden=true;colorSelection=null;status('draft-unsaved');repaint();schedule();if(!composing&&!event.isComposing)scheduleProject();
    }
    function open(event){
        if(disposed||!event.isTrusted||document.querySelector('dialog[open]'))return;
        if(token!==null&&(saving||dirty)){root.showModal();repaint();changed();editor.focus();return;}
        if(awaiting||opening!==null){notice(tr('draft-save-first'));return;}
        prepare(event);opening=++request;token=null;entries=[];projectAgain=false;projectPending=false;node('editor').hidden=true;node('chooser').hidden=true;node('delete-panel').hidden=true;confirm.hidden=true;status('draft-loading');root.showModal();changed();repaint();
        send({kind:'draft_open',request:opening,trusted:true});
    }
    function close(event){
        if(disposed||!event.isTrusted||composing||colorPending)return;event.preventDefault();
        if(awaiting||opening!==null){send({kind:'draft_cancel',trusted:true});opening=null;awaiting=false;}
        flush();if(dirty&&!saving)return;root.close();confirm.hidden=true;colors.hidden=true;restore();changed();repaint();
    }
    function choose(event,fresh=false){if(disposed||!event.isTrusted||opening!==null||saving||dirty||awaiting||fresh&&!canCreate)return;awaiting=true;repaint();send({kind:'draft_choose',id:fresh?null:list.value,trusted:true});}
    function receive(reply){
        if(disposed)return;
        if(reply.state==='list'){
            if(opening!==reply.request)return;opening=null;awaiting=false;if(!root.open)return;entries=reply.entries;canCreate=reply.can_create===true;
            node('scope').textContent=reply.scope.account+' · '+reply.scope.board;
            list.replaceChildren(...entries.map(entry=>{const option=document.createElement('option');option.value=entry.id;option.textContent=(entry.title||tr('draft-untitled'))+' · '+new Date(entry.saved_at*1000).toLocaleString()+(entry.conflict?' · '+tr('draft-conflict-copy'):'');return option;}));
            node('chooser').hidden=false;status(entries.length?'draft-found':canCreate?'draft-empty':'draft-unavailable');if(entries.length){list.value=entries[0].id;list.focus();}else if(canCreate)choose({isTrusted:true},true);
        }else if(reply.state==='deleted'){
            awaiting=false;entries=entries.filter(e=>e.id!==reply.id);for(const option of [...list.children])if(option.value===reply.id)option.remove();list.value=entries[0]?.id??'';node('delete-panel').hidden=true;status('draft-deleted');
        }else if(reply.state==='selected'){
            if(!root.open)return;node('chooser').hidden=true;node('editor').hidden=false;if(!ensureEditor())return;token=reply.token;sequence=0;revision=0;awaiting=false;saving=false;dirty=false;composing=false;flushAfter=false;lastSave=0;
            title.value=reply.entry.title;editor.load(reply.entry.body);committed={title:title.value,body:reply.entry.body};colors.hidden=true;confirm.hidden=true;
            node('chooser').hidden=true;node('editor').hidden=false;status(reply.entry.revision?'draft-saved':'draft-local');if(!reply.entry.revision&&(title.value||source())){dirty=true;lastEdit=now();schedule();}project();editor.focus();
        }else if(reply.state==='saved'){
            if(reply.token!==token||reply.sequence!==sequence)return;saving=false;status(dirty?'draft-unsaved':reply.conflicted?'draft-conflict-saved':'draft-saved');
            if(dirty){if(flushAfter||!root.open){flushAfter=false;flush();}else schedule();}
        }else if(reply.state==='projected'){
            if(reply.token!==token)return;projectPending=false;if(reply.revision===revision)editor.project(reply.decorations);if(projectAgain||reply.revision!==revision){projectAgain=false;project();}
        }else if(reply.state==='colored'){
            if(reply.token!==token)return;colorPending=false;if(reply.revision===revision){editor.apply(reply.changes);colors.hidden=true;editor.focus();}else status('draft-color-stale');
        }else if(reply.state==='decoration_error'){
            if(reply.token!==token)return;if(reply.coloring)colorPending=false;else projectPending=false;
            if(reply.revision===revision)note.textContent=reply.message;
        }else if(reply.state==='transferred'){
            awaiting=false;if(root.open)root.close();notice(reply.message);restore();changed();
        }else if(reply.state==='error'){
            const failedSave=saving;saving=false;awaiting=false;opening=null;colorPending=false;projectPending=false;projectAgain=false;if(failedSave)dirty=true;note.textContent=reply.message;if(!root.open)notice(reply.message);
        }
        repaint();
    }
    function station(event){if(!event.isTrusted||!editable()||saving||dirty)return;const p=current();if(!p?.connected||!p.editor||!p.draft_post_scope||p.input_pending||p.output_pending)return;confirm.hidden=false;repaint();node('transfer-confirm').focus();}
    function transfer(event){if(!event.isTrusted||!editable()||dirty||saving||confirm.hidden)return;const p=current();if(!p?.editor||!p.draft_post_scope)return;awaiting=true;repaint();send({kind:'draft_station',token,stamp:{generation:p.generation,revision:p.frame.revision,input_epoch:p.input_epoch},trusted:true});}
    function openColors(event){if(!event.isTrusted||!editable())return;const ranges=editor.ranges();if(!ranges.length)return;colorSelection={ranges,revision};event.preventDefault();colors.hidden=false;
        // The palette is a top-layer popover, independent of editor layout/scroll.
        const r=root.getBoundingClientRect(),box=colors.getBoundingClientRect();colors.style.left=Math.max(8,Math.min(event.clientX-r.left,r.width-box.width-8))+'px';colors.style.top=Math.max(8,Math.min(event.clientY-r.top,r.height-box.height-8))+'px';
    }
    const swatches=[];
    for(const [group,count]of [['foreground',16],['background',8]])for(let i=0;i<count;i++){
        const b=document.createElement('button');b.type='button';b.dataset.group=group;b.dataset.color=String(i);b.textContent=(group==='foreground'?(i>=8?'1;':'')+(30+i%8):40+i).toString();b.style.backgroundColor=palette[i];b.style.color=[0,1,4,5].includes(i%8)?'#fff':'#000';b.setAttribute('aria-label',tr('editor-color-name-'+i%8)+(i>=8?' · '+tr('editor-color-bright'):'')+' '+b.textContent);node(group).append(b);swatches.push(b);
    }
    function swatch(event){if(!event.isTrusted||!editable())return;const b=event.target.closest('button');if(!swatches.includes(b))return;if(!colorSelection||colorSelection.revision!==revision)return;const ranges=colorSelection.ranges;
        if(b.dataset.group==='foreground')foreground=Number(b.dataset.color);else background=Number(b.dataset.color);
        colorPending=true;repaint();send({kind:'draft_color',token,revision,body:source(),ranges,color:{foreground,background},trusted:true});
    }
    function key(event){event.stopPropagation();if(event.isComposing||composing||event.keyCode===229)return;if(event.key==='Escape'){event.preventDefault();if(!colors.hidden){colors.hidden=true;editor.focus();}else close(event);}else if((event.ctrlKey||event.metaKey)&&event.key.toLowerCase()==='s'){event.preventDefault();if(event.isTrusted){dirty=true;flush();}}}
    function update(){if(disposed)return;const p=current();if(!p?.editor||!p?.draft_post_scope)confirm.hidden=true;repaint();}
    function destroy(){cancelTimer();if(projectTimer)window.clearTimeout(projectTimer);projectTimer=0;editor?.destroy();editor=null;disposed=true;opening=null;saving=false;dirty=false;awaiting=false;token=null;entries=[];committed=null;title.value='';list.replaceChildren();for(const group of ['foreground','background'])node(group).replaceChildren();if(root.open)root.close();bar.hidden=true;}
    return {open,receive,update,snapshot,destroy,clean:()=>disposed&&!timer&&!projectTimer&&!root.open&&!token&&!title.value,
        bindings:[[node('open'),'click',open],[node('station-open'),'click',open],[node('close'),'click',close],[root,'cancel',close],[root,'keydown',key],[list,'change',repaint],[node('restore'),'click',e=>choose(e)],[node('new'),'click',e=>choose(e,true)],
            [node('native'),'click',close],
            [node('delete'),'click',e=>{if(e.isTrusted&&!saving&&!dirty&&!awaiting&&list.value){node('delete-panel').hidden=false;node('delete-panel').dataset.id=list.value;}}],
            [node('delete-confirm'),'click',e=>{if(e.isTrusted&&!saving&&!dirty&&!awaiting&&!node('delete-panel').hidden){awaiting=true;repaint();send({kind:'draft_delete',id:node('delete-panel').dataset.id,trusted:true});}}],
            [node('delete-cancel'),'click',e=>{if(e.isTrusted)node('delete-panel').hidden=true;}],
            [title,'input',edited],[root,'compositionstart',()=>{committed={title:title.value,body:source()};composing=true;cancelTimer();repaint();}],[root,'compositionend',e=>{composing=false;edited(e);}],[node('save'),'click',e=>{if(e.isTrusted&&!disposed&&token!==null){dirty=true;flush();}}],
            [node('transfer'),'click',station],[node('transfer-confirm'),'click',transfer],[node('transfer-cancel'),'click',e=>{if(e.isTrusted)confirm.hidden=true;}],
            [colors,'mousedown',e=>e.preventDefault()],[colors,'click',swatch],[node('color-close'),'click',e=>{if(e.isTrusted){colors.hidden=true;editor.focus();}}],
            ...['color','plain','source'].map(mode=>[node('mode-'+mode),'click',e=>{if(e.isTrusted&&editable()){editor.mode(mode);for(const m of ['color','plain','source'])node('mode-'+m).setAttribute('aria-pressed',String(mode===m));project();}}]),
            ...['text','line','rectangle'].map(mode=>[node('select-'+mode),'click',e=>{if(e.isTrusted&&editable()){editor.selection(mode);for(const m of ['text','line','rectangle'])node('select-'+m).setAttribute('aria-pressed',String(mode===m));}}]),
            [window,'blur',flush],[document,'visibilitychange',()=>{if(document.visibilityState==='hidden')flush();}]]};
}
