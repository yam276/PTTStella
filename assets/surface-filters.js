// Rules editing and temporary presentation blocking share this owner.
function createFiltersDialog({tr,send,current,invalidate,reset,restore,changed,manage=null,present=dialog=>dialog.showModal(),visible=()=>true}) {
    const filtersButton=document.getElementById("filters-open"),filtersDialog=document.getElementById("filters-dialog");
    const filtersList=document.getElementById("filters-list"),filtersId=document.getElementById("filters-id"),filtersNote=document.getElementById("filters-note");
    const filtersSave=document.getElementById("filters-save"),filtersRemove=document.getElementById("filters-remove"),filtersClose=document.getElementById("filters-close");
    const filtersMessage=document.getElementById("filters-note-status"),filtersReveal=document.getElementById("filters-reveal"),filtersStatus=document.getElementById("filters-status");
    const filtersColor=document.getElementById("filters-color"),defaultColor=document.getElementById("filters-default-color"),defaultSave=document.getElementById("filters-default-color-save");
    let savedColor=14;
    const filtersPolicy=document.getElementById("filters-policy");
    const importButton=document.getElementById("filters-import"),importApply=document.getElementById("filters-import-apply"),importCancel=document.getElementById("filters-import-cancel");
    let importSequence=0,importRequest=null,importPreview=false,savedEntries=[];
    let filterEntries=[],filtersBusy=false,presentationBlocked=false,disposed=false;
    function openFilters(e){if(disposed||!e.isTrusted||manage&&!manage(e))return;if(filtersDialog.open)return;invalidate();reset(e);present(filtersDialog);changed();send({kind:"filters_open",trusted:e.isTrusted});}
    function closeFilters(e){e?.preventDefault();cancelImport(e);if(filtersDialog.open)filtersDialog.close();filtersId.value="";filtersNote.value="";filtersPolicy.value="mask";filtersColor.value="";defaultColor.value=String(savedColor);colorControls();filtersList.selectedIndex=-1;restore();changed();}
    function fillFilters(entries){filterEntries=entries;const selected=filtersId.value.toLowerCase();filtersList.replaceChildren();const fresh=document.createElement("option");fresh.value="";fresh.textContent=tr("filters-new");fresh.selected=!selected;filtersList.append(fresh);
        for(const entry of entries){const option=document.createElement("option");option.value=entry.id;option.textContent=tr(entry.policy==="allow"?"filters-policy-allow":entry.policy==="mask_row"?"filters-policy-mask-row":"filters-policy-mask")+" · "+entry.id+(entry.note?" — "+entry.note:"");option.selected=entry.id.toLowerCase()===selected;filtersList.append(option);}}
    function chooseFilter(){const entry=filterEntries.find(x=>x.id===filtersList.value);filtersId.value=entry?.id??"";filtersNote.value=entry?.note??"";filtersPolicy.value=entry?.policy??"mask";filtersColor.value=entry?.color==null?"":String(entry.color);colorControls();}
    function editFilters(e,remove){if(disposed||!e.isTrusted||filtersBusy||importRequest!==null||!filtersDialog.open)return;filtersBusy=true;filtersSave.disabled=true;filtersRemove.disabled=true;
        presentationBlocked=true;invalidate();changed();importControls();filtersMessage.textContent=tr("saving");
        send({kind:"filters_edit",id:filtersId.value,note:filtersNote.value,policy:filtersPolicy.value||"mask",color:filtersPolicy.value==="allow"&&filtersColor.value!==""?Number(filtersColor.value):null,remove,trusted:e.isTrusted});}
    function colorControls(){filtersColor.disabled=filtersBusy||importRequest!==null||filtersPolicy.value!=="allow";}
    function saveDefault(e){
        if(disposed||!e.isTrusted||filtersBusy||importRequest!==null||!filtersDialog.open)return;
        filtersBusy=true;presentationBlocked=true;invalidate();changed();importControls();filtersMessage.textContent=tr("saving");
        send({kind:"filters_default_color",color:Number(defaultColor.value),trusted:true});
    }
    const saveFilter=e=>editFilters(e,false),removeFilter=e=>editFilters(e,true);
    function revealFilters(e){const latest=current();if(!latest||presentationBlocked)return;presentationBlocked=true;invalidate();changed();
        send({kind:"filters_reveal",presentation:latest.presentation,generation:latest.generation,revision:latest.frame.revision,reveal:!latest.revealed,trusted:e.isTrusted});}

    function open(data){if(disposed||!filtersDialog.open)return;savedEntries=data.entries;savedColor=data.allow_color??14;defaultColor.value=String(savedColor);filtersColor.value="";colorControls();fillFilters(data.entries);filtersMessage.textContent=data.warning||tr("filters-save-help");if(visible())filtersId.focus();}
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
        filtersSave.disabled=filtersBusy||pending;filtersRemove.disabled=filtersBusy||pending;
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
    return {show:openFilters,request:openFilters,close:closeFilters,open,receive,paint,revoke,destroy,blocked:()=>presentationBlocked,
        clean:()=>disposed&&!filtersDialog.open&&filterEntries.length===0&&!filtersId.value&&!filtersNote.value,
        bindings:[[filtersPolicy,"change",colorControls],[defaultSave,"click",saveDefault],[importButton,"click",startImport],[importApply,"click",applyImport],[importCancel,"click",cancelImport],[filtersButton,"click",openFilters],[filtersClose,"click",closeFilters],[filtersDialog,"cancel",closeFilters],
            [filtersList,"change",chooseFilter],[filtersSave,"click",saveFilter],[filtersRemove,"click",removeFilter],[filtersReveal,"click",revealFilters]]};
}
