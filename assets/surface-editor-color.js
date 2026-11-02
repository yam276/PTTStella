// A local, bounded colour picker. Only Rust owns selection and station bytes.
function editorColorCode(foreground,background) {
    if(!Number.isInteger(foreground)||foreground<0||foreground>15||!Number.isInteger(background)||background<0||background>7)throw new Error("Invalid PTT colour");
    return "0;"+(foreground>=8?"1;":"")+(30+foreground%8)+";"+(40+background);
}
function createEditorColor({document,root,palette,tr,identity,send,restore,changed}) {
    let lease=null,disposed=false,foreground=15,background=0;
    const groups=[document.getElementById("editor-color-foreground"),document.getElementById("editor-color-background")];
    const preview=document.getElementById("editor-color-preview"),code=document.getElementById("editor-color-code");
    const apply=document.getElementById("editor-color-apply"),cancel=document.getElementById("editor-color-cancel"),reset=document.getElementById("editor-color-reset");
    const swatches=[];
    for(let group=0;group<2;group++)for(let index=0;index<(group===0?16:8);index++){
        const button=document.createElement("button");button.type="button";button.dataset.group=String(group);button.dataset.color=String(index);
        button.style.backgroundColor=palette[index];button.textContent=String(group===0?30+index%8:40+index);
        button.style.color=(index%8===0||index%8===1||index%8===4||index%8===5)?"#fff":"#000";
        button.title=tr("editor-color-name-"+(index%8))+(index>=8?" · "+tr("editor-color-bright"):"")+" ("+(group===0?(index>=8?"1;":"0;")+(30+index%8):40+index)+")";
        button.setAttribute("aria-label",button.title);groups[group].append(button);swatches.push(button);
    }
    function paint(){
        for(const b of swatches)b.setAttribute("aria-pressed",String(Number(b.dataset.color)===(b.dataset.group==="0"?foreground:background)));
        preview.style.color=palette[foreground];preview.style.backgroundColor=palette[background];
        code.textContent="ESC["+editorColorCode(foreground,background)+"m";
    }
    function close(focus=true){if(!lease)return;lease=null;root.close();changed();if(focus)restore();}
    function open(selection,stamp){
        if(disposed||!selection)return;close(false);lease={identity:identity(),selection:{presentation:selection.presentation,request:selection.request},stamp:{...stamp}};
        paint();root.showModal();changed();swatches[foreground].focus();
    }
    function click(event){
        if(!lease||!event.isTrusted)return;
        const b=event.target.closest("button");if(!b||!root.contains(b))return;
        if(swatches.includes(b)){if(b.dataset.group==="0")foreground=Number(b.dataset.color);else background=Number(b.dataset.color);paint();}
        else if(b===reset){foreground=7;background=0;paint();}
        else if(b===cancel)close();
        else if(b===apply){const active=lease,valid=active.identity===identity();close();if(valid)send({kind:"editor_color",...active.selection,stamp:active.stamp,color:{foreground,background},trusted:true});}
    }
    function key(event){if(!lease)return;event.stopImmediatePropagation();if(event.key==="Escape"){event.preventDefault();if(event.isTrusted)close();}}
    function cancelEvent(event){event.preventDefault();if(event.isTrusted)close();}
    function validate(){if(lease&&lease.identity!==identity())close();}
    function destroy(){close(false);disposed=true;for(const g of groups)g.replaceChildren();}
    return {open,validate,destroy,clean:()=>disposed&&!lease&&!root.open,bindings:[[root,"click",click],[root,"cancel",cancelEvent],[root,"keydown",key,true]]};
}
