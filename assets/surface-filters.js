// Rules editing and temporary presentation blocking share this owner.
function createFiltersDialog({tr,send,current,invalidate,reset,restore,changed}) {
    const filtersButton=document.getElementById("filters-open"),filtersDialog=document.getElementById("filters-dialog");
    const filtersList=document.getElementById("filters-list"),filtersId=document.getElementById("filters-id"),filtersNote=document.getElementById("filters-note");
    const filtersSave=document.getElementById("filters-save"),filtersRemove=document.getElementById("filters-remove"),filtersClose=document.getElementById("filters-close");
    const filtersMessage=document.getElementById("filters-note-status"),filtersReveal=document.getElementById("filters-reveal"),filtersStatus=document.getElementById("filters-status");
    let filterEntries=[],filtersBusy=false,presentationBlocked=false,disposed=false;
    function openFilters(e){if(disposed)return;invalidate();reset(e);filtersDialog.showModal();changed();send({kind:"filters_open",trusted:e.isTrusted});}
    function closeFilters(e){e?.preventDefault();if(filtersDialog.open)filtersDialog.close();filtersId.value="";filtersNote.value="";filtersList.selectedIndex=-1;restore();changed();}
    function fillFilters(entries){filterEntries=entries;const selected=filtersId.value.toLowerCase();filtersList.replaceChildren();const fresh=document.createElement("option");fresh.value="";fresh.textContent=tr("filters-new");fresh.selected=!selected;filtersList.append(fresh);
        for(const entry of entries){const option=document.createElement("option");option.value=entry.id;option.textContent=entry.id+(entry.note?" — "+entry.note:"");option.selected=entry.id.toLowerCase()===selected;filtersList.append(option);}}
    function chooseFilter(){const entry=filterEntries.find(x=>x.id===filtersList.value);filtersId.value=entry?.id??"";filtersNote.value=entry?.note??"";}
    function editFilters(e,remove){if(filtersBusy||!filtersDialog.open)return;filtersBusy=true;filtersSave.disabled=true;filtersRemove.disabled=true;
        presentationBlocked=true;invalidate();changed();filtersMessage.textContent=tr("saving");
        send({kind:"filters_edit",id:filtersId.value,note:filtersNote.value,remove,trusted:e.isTrusted});}
    const saveFilter=e=>editFilters(e,false),removeFilter=e=>editFilters(e,true);
    function revealFilters(e){const latest=current();if(!latest||presentationBlocked)return;presentationBlocked=true;invalidate();changed();
        send({kind:"filters_reveal",presentation:latest.presentation,generation:latest.generation,revision:latest.frame.revision,reveal:!latest.revealed,trusted:e.isTrusted});}

    function open(data){if(disposed)return;fillFilters(data.entries);filtersMessage.textContent=data.warning||tr("filters-save-help");filtersId.focus();}
    function receive(data){
        if(disposed)return;
        filtersBusy=false;filtersSave.disabled=false;filtersRemove.disabled=false;
        if(data.state==="saved")fillFilters(data.entries);
        filtersMessage.textContent=data.message;
        if(data.state==="rejected"){presentationBlocked=false;changed();}
    }
    function paint(packet){
        if(disposed)return;
        presentationBlocked=false;
        filtersReveal.hidden=!packet.masked_pushes;filtersReveal.textContent=packet.revealed?tr("filters-mask"):tr("filters-reveal");
        filtersStatus.textContent=packet.masked_pushes?tr(packet.revealed?"filters-count-revealed":"filters-count-hidden",{count:packet.masked_pushes}):"";
        filtersStatus.title=packet.filter_reason?tr("filters-reason",{reason:packet.filter_reason}):"";
    }
    function revoke(){disposed=true;}
    function destroy(){revoke();if(filtersDialog.open)filtersDialog.close();filterEntries=[];filtersId.value="";filtersNote.value="";}
    return {open,receive,paint,revoke,destroy,blocked:()=>presentationBlocked,
        clean:()=>disposed&&!filtersDialog.open&&filterEntries.length===0&&!filtersId.value&&!filtersNote.value,
        bindings:[[filtersButton,"click",openFilters],[filtersClose,"click",closeFilters],[filtersDialog,"cancel",closeFilters],
            [filtersList,"change",chooseFilter],[filtersSave,"click",saveFilter],[filtersRemove,"click",removeFilter],[filtersReveal,"click",revealFilters]]};
}
