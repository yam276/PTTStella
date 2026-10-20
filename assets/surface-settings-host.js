// Same-WebView settings: one modal, one autosave owner, no auxiliary window.
function createSurfaceSettings(options) {
    const {document,send}=options;
    let launch=null;
    const dialog=document.getElementById("font-dialog");
    const ui=createSettingsDialog({...options,
        page:()=>navigation.current(),
        onClose(){send({kind:"settings_visibility",open:false});if(launch){const feature=launch;launch=null;send({kind:"settings_launch",feature,trusted:true});}}
    });
    const navigation=createSettingsNavigation({document,tr:options.tr,launch:(feature,e)=>{launch=feature;ui.close(e);}});
    function request(e){if(!e.isTrusted)return;if(dialog.open){document.getElementById("settings-search").focus();return;}ui.request(e);}
    const shortcuts=[document,"keydown",e=>{if(e.isTrusted&&(e.metaKey||e.ctrlKey)&&e.key===","&&!e.isComposing){e.preventDefault();e.stopImmediatePropagation();if(!e.repeat)request(e);}},true];
    return {...ui,request,relocalize(){ui.relocalize();navigation.relocalize();},
        open:async data=>{await ui.open(data);if(dialog.open)document.getElementById("settings-search").focus();},
        bindings:[...ui.bindings,...navigation.bindings,shortcuts],
        destroy(){launch=null;navigation.destroy();ui.destroy();}
    };
}
