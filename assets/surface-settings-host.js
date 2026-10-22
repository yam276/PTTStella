// One modal owns the management sections as well as appearance autosave.
function createSurfaceSettings(options) {
    const {document,send}=options;
    const dialog=document.getElementById("font-dialog");
    let requestedPage=null,disposed=false;
    const features=()=>options.features?.()??{};
    const visiblePage=()=>navigation.visible?navigation.visible():navigation.current();
    const panel=page=>{const id=({sites:"sites-dialog",filters:"filters-dialog",account:"login-dialog"})[page];return id?document.getElementById(id):null;};
    function leave(page){
        const node=panel(page),active=document.activeElement;
        if(node?.open)features()[page]?.close({isTrusted:true,preventDefault(){}});
        if(dialog.open&&active&&dialog.contains(active)&&!active.closest("[hidden]"))active.focus({preventScroll:true});
    }
    const ui=createSettingsDialog({...options,
        page:()=>navigation.current(),
        onClose(){
            for(const page of ["sites","filters","account"]){leave(page);const node=panel(page);if(node?.open)node.close();}
            requestedPage=null;send({kind:"settings_visibility",open:false});
        }
    });
    const navigation=createSettingsNavigation({document,tr:options.tr,changed(page,previous,event){
        if(disposed||!dialog.open)return;
        if(previous!==page)leave(previous);
        if(!panel(page)?.open)features()[page]?.request(event??{isTrusted:true});
    }});
    function request(e,page=null){
        if(disposed||!e.isTrusted)return;
        if(dialog.open){if(page)navigation.show(page,null,e);else document.getElementById("settings-search").focus();return;}
        requestedPage=page;ui.request(e);
    }
    function manage(page,e){
        if(disposed||!e.isTrusted)return false;
        if(dialog.open&&visiblePage()===page)return true;
        request(e,page);return false;
    }
    const shortcuts=[document,"keydown",e=>{if(e.isTrusted&&(e.metaKey||e.ctrlKey)&&e.key===","&&!e.isComposing){e.preventDefault();e.stopImmediatePropagation();if(!e.repeat)request(e);}},true];
    return {...ui,request,manage,visible:page=>dialog.open&&visiblePage()===page,relocalize(){ui.relocalize();navigation.relocalize();},
        open:async data=>{const opening=ui.open(data);if(dialog.open){navigation.show(requestedPage??navigation.current());requestedPage=null;}await opening;},
        bindings:[...ui.bindings,...navigation.bindings,shortcuts],
        destroy(){disposed=true;requestedPage=null;navigation.destroy();ui.destroy();}
    };
}
