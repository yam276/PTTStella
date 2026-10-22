// Rules editing and temporary presentation blocking share this owner.
function createFiltersDialog({tr,send,current,invalidate,reset,restore,changed,manage=null,present=dialog=>dialog.showModal(),visible=()=>true}) {
    const filtersButton=document.getElementById("filters-open"),filtersDialog=document.getElementById("filters-dialog");
    const filtersList=document.getElementById("filters-list"),filtersId=document.getElementById("filters-id"),filtersNote=document.getElementById("filters-note");
    const filtersSave=document.getElementById("filters-save"),filtersRemove=document.getElementById("filters-remove"),filtersClose=document.getElementById("filters-close");
    const filtersMessage=document.getElementById("filters-note-status"),filtersReveal=document.getElementById("filters-reveal"),filtersStatus=document.getElementById("filters-status");
    const filtersColor=document.getElementById("filters-color"),defaultColor=document.getElementById("filters-default-color"),defaultSave=document.getElementById("filters-default-color-save");
    let savedColor=14,tab="block",sort="id";
    const tabBlock=document.getElementById("filters-tab-block"),tabAllow=document.getElementById("filters-tab-allow");
    const editor=document.getElementById("filters-editor"),sortButton=document.getElementById("filters-sort"),sortLabel=document.getElementById("filters-sort-label"),newButton=document.getElementById("filters-new");
    const empty=document.getElementById("filters-empty"),policyField=document.getElementById("filters-policy-field"),colorField=document.getElementById("filters-color-field"),sharedColor=document.getElementById("filters-shared-color");
    const filtersPolicy=document.getElementById("filters-policy");
    const importButton=document.getElementById("filters-import"),importApply=document.getElementById("filters-import-apply"),importCancel=document.getElementById("filters-import-cancel");
    let importSequence=0,importRequest=null,importPreview=false,savedEntries=[];
    let filterEntries=[],filtersBusy=false,presentationBlocked=false,disposed=false;
    function openFilters(e){if(disposed||!e.isTrusted||manage&&!manage(e))return;if(filtersDialog.open)return;invalidate();reset(e);present(filtersDialog);changed();send({kind:"filters_open",trusted:e.isTrusted});}
    const inTab=entry=>(entry.policy==="allow")===(tab==="allow");
    function clearDraft(){filtersId.value="";filtersNote.value="";filtersPolicy.value=tab==="allow"?"allow":"mask";filtersColor.value="";filtersList.selectedIndex=-1;colorControls();}
    function closeFilters(e){e?.preventDefault();cancelImport(e);if(filtersDialog.open)filtersDialog.close();clearDraft();defaultColor.value=String(savedColor);restore();changed();}
    function colorName(color){
        const option=Array.from(filtersColor.options??[]).find(option=>option.value===String(color));
        return option?.dataset.colorKey?tr(option.dataset.colorKey):option?.textContent??String(color);
    }
    function renderList(){
        const selected=filtersId.value.toLowerCase(),allow=tab==="allow";
        const category=entry=>allow?(entry.color??savedColor):entry.policy==="mask_row"?1:0;
        const entries=filterEntries.filter(inTab).sort((a,b)=>{
            const group=sort==="category"?category(a)-category(b):0;
            const left=a.id.toLowerCase(),right=b.id.toLowerCase();return group||(left<right?-1:left>right?1:0);
        });
        filtersList.replaceChildren();
        for(const entry of entries){
            const option=document.createElement("option");option.value=entry.id;
            const detail=allow?colorName(entry.color??savedColor)+(entry.color==null?" · "+tr("filters-color-shared"):""):
                tr(entry.policy==="mask_row"?"filters-policy-mask-row":"filters-policy-mask");
            option.textContent=entry.id+" · "+detail+(entry.note?" — "+entry.note:"");filtersList.append(option);
        }
        filtersList.selectedIndex=entries.findIndex(entry=>entry.id.toLowerCase()===selected);
        empty.hidden=entries.length!==0;empty.textContent=tr(allow?"filters-empty-allow":"filters-empty-block");
        tabBlock.setAttribute("aria-selected",String(!allow));tabAllow.setAttribute("aria-selected",String(allow));
        tabBlock.setAttribute("tabindex",allow?"-1":"0");tabAllow.setAttribute("tabindex",allow?"0":"-1");
        editor.setAttribute("aria-labelledby",allow?"filters-tab-allow":"filters-tab-block");
        policyField.hidden=allow;colorField.hidden=!allow;sharedColor.hidden=!allow;
        for(const option of Array.from(filtersPolicy.options??[])){option.hidden=allow?option.value!=="allow":option.value==="allow";option.disabled=option.hidden;}
        sortLabel.textContent=tr(sort==="id"?"filters-sort-id":"filters-sort-category");sortButton.setAttribute("aria-pressed",String(sort==="category"));
    }
    function fillFilters(entries){filterEntries=entries;renderList();}
    function chooseTab(next,e){if(disposed||!e.isTrusted||!filtersDialog.open||tab===next)return;tab=next;clearDraft();renderList();importControls();}
    const chooseBlock=e=>chooseTab("block",e),chooseAllow=e=>chooseTab("allow",e);
    function tabKey(e){
        if(!e.isTrusted||!["ArrowLeft","ArrowRight","Home","End"].includes(e.key))return;
        e.preventDefault();e.stopPropagation();const next=e.key==="Home"?"block":e.key==="End"?"allow":tab==="block"?"allow":"block";
        chooseTab(next,e);(next==="allow"?tabAllow:tabBlock).focus();
    }
    function sortFilters(e){if(disposed||!e.isTrusted||!filtersDialog.open)return;sort=sort==="id"?"category":"id";renderList();}
    function newFilter(e){if(disposed||!e.isTrusted||filtersBusy||importRequest!==null||!filtersDialog.open)return;clearDraft();if(visible())filtersId.focus();}
    function chooseFilter(){const entry=filterEntries.find(x=>x.id===filtersList.value&&inTab(x));filtersId.value=entry?.id??"";filtersNote.value=entry?.note??"";filtersPolicy.value=entry?.policy??(tab==="allow"?"allow":"mask");filtersColor.value=entry?.color==null?"":String(entry.color);colorControls();}
    function editFilters(e,remove){if(disposed||!e.isTrusted||filtersBusy||importRequest!==null||!filtersDialog.open)return;filtersBusy=true;filtersSave.disabled=true;filtersRemove.disabled=true;
        presentationBlocked=true;invalidate();changed();importControls();filtersMessage.textContent=tr("saving");
        const policy=tab==="allow"?"allow":filtersPolicy.value==="mask_row"?"mask_row":"mask";
        send({kind:"filters_edit",id:filtersId.value,note:filtersNote.value,policy,color:policy==="allow"&&filtersColor.value!==""?Number(filtersColor.value):null,remove,trusted:e.isTrusted});}
    function colorControls(){filtersColor.disabled=filtersBusy||importRequest!==null||tab!=="allow";}
    function saveDefault(e){
        if(disposed||!e.isTrusted||tab!=="allow"||filtersBusy||importRequest!==null||!filtersDialog.open)return;
        filtersBusy=true;presentationBlocked=true;invalidate();changed();importControls();filtersMessage.textContent=tr("saving");
        send({kind:"filters_default_color",color:Number(defaultColor.value),trusted:true});
    }
    const saveFilter=e=>editFilters(e,false),removeFilter=e=>editFilters(e,true);
    function revealFilters(e){const latest=current();if(!latest||presentationBlocked)return;presentationBlocked=true;invalidate();changed();
        send({kind:"filters_reveal",presentation:latest.presentation,generation:latest.generation,revision:latest.frame.revision,reveal:!latest.revealed,trusted:e.isTrusted});}

    function open(data){if(disposed||!filtersDialog.open)return;savedEntries=data.entries;savedColor=data.allow_color??14;defaultColor.value=String(savedColor);clearDraft();fillFilters(data.entries);filtersMessage.textContent=data.warning||tr("filters-save-help");if(visible())filtersId.focus();}
    function receive(data){
        if(disposed)return;
        if(data.state==="preview"||data.state==="import_rejected") {
            if(data.request!==importRequest||!filtersDialog.open)return;
            filtersBusy=false;
            if(data.state==="preview"){importPreview=true;fillFilters(data.entries);}
            else {importRequest=null;importPreview=false;}
            importControls();filtersMessage.textContent=data.message;return;
        }
        filtersBusy=false;
        if(data.state==="saved"){savedEntries=data.entries;savedColor=data.allow_color??savedColor;defaultColor.value=String(savedColor);fillFilters(data.entries);}
        importControls();
        filtersMessage.textContent=data.message;
        if(data.state==="rejected"){fillFilters(savedEntries);defaultColor.value=String(savedColor);presentationBlocked=false;changed();}
    }
    function importControls(){
        const pending=importRequest!==null;
        filtersSave.disabled=filtersBusy||pending;filtersRemove.disabled=filtersBusy||pending;newButton.disabled=filtersBusy||pending;
        filtersId.disabled=pending||filtersBusy;filtersNote.disabled=pending||filtersBusy;filtersPolicy.disabled=pending||filtersBusy;
        defaultColor.disabled=pending||filtersBusy;defaultSave.disabled=pending||filtersBusy;colorControls();
        importButton.disabled=filtersBusy||pending;importApply.hidden=!importPreview;importApply.disabled=filtersBusy;
        importCancel.hidden=!pending;
    }
    function startImport(e){
        if(disposed||!e.isTrusted||filtersBusy||importRequest!==null||!filtersDialog.open)return;
        importRequest=++importSequence;importPreview=false;filtersBusy=true;importControls();
        filtersMessage.textContent=tr("filters-import-loading");send({kind:"filters_import",request:importRequest,trusted:true});
    }
    function cancelImport(e){
        if(disposed||!e?.isTrusted||importRequest===null)return;
        if(!disposed)send({kind:"filters_import_cancel",request:importRequest,trusted:!!e?.isTrusted});
        importRequest=null;importPreview=false;filtersBusy=false;fillFilters(savedEntries);importControls();
        filtersMessage.textContent=tr("filters-import-cancelled");
    }
    function applyImport(e){
        if(disposed||!e.isTrusted||filtersBusy||!importPreview||importRequest===null||!filtersDialog.open)return;
        const request=importRequest;importRequest=null;importPreview=false;filtersBusy=true;
        presentationBlocked=true;invalidate();changed();importControls();
        filtersMessage.textContent=tr("saving");send({kind:"filters_import_apply",request,trusted:true});
    }
    function paint(packet){
        if(disposed)return;
        presentationBlocked=false;
        filtersReveal.hidden=!packet.masked_pushes;filtersReveal.textContent=packet.revealed?tr("filters-mask"):tr("filters-reveal");
        filtersStatus.textContent=packet.masked_pushes?tr(packet.revealed?"filters-count-revealed":"filters-count-hidden",{count:packet.masked_pushes}):"";
        filtersStatus.title=packet.filter_reason?tr("filters-reason",{reason:packet.filter_reason}):"";
    }
    function revoke(){disposed=true;}
    function destroy(){revoke();importRequest=null;importPreview=false;savedEntries=[];if(filtersDialog.open)filtersDialog.close();filterEntries=[];filtersId.value="";filtersNote.value="";}
    return {show:openFilters,request:openFilters,close:closeFilters,open,receive,paint,revoke,destroy,relocalize(){if(!disposed)renderList();},blocked:()=>presentationBlocked,
        clean:()=>disposed&&!filtersDialog.open&&filterEntries.length===0&&!filtersId.value&&!filtersNote.value,
        bindings:[[tabBlock,"click",chooseBlock],[tabAllow,"click",chooseAllow],[tabBlock,"keydown",tabKey],[tabAllow,"keydown",tabKey],[sortButton,"click",sortFilters],[newButton,"click",newFilter],[filtersPolicy,"change",colorControls],[defaultSave,"click",saveDefault],[importButton,"click",startImport],[importApply,"click",applyImport],[importCancel,"click",cancelImport],[filtersButton,"click",openFilters],[filtersClose,"click",closeFilters],[filtersDialog,"cancel",closeFilters],
            [filtersList,"change",chooseFilter],[filtersSave,"click",saveFilter],[filtersRemove,"click",removeFilter],[filtersReveal,"click",revealFilters]]};
}
