// Menu lifetime and keyboard/focus handling. Actions retain their existing owners.
// Shared ordered policy data, also consumed by Rust's context_menu owner.
function contextMenuItems(state) {
    const flags={...state,word:!!state.word,reply:state.replyRow!==null&&state.replyRow!==undefined,
        link:state.link>=0,background:!state.selection&&state.link<0,
        mask:state.word?.policy==="mask",maskRow:state.word?.policy==="mask_row",allow:state.word?.policy==="allow"};
    const matches=predicate=>(predicate?.all??[]).every(flag=>!!flags[flag])
        &&(!(predicate?.any?.length)||predicate.any.some(flag=>!!flags[flag]))
        &&(predicate?.none??[]).every(flag=>!flags[flag]);
    const entries=new Map();
    for(const step of CONTEXT_MENU_POLICY){
        if(!matches(step.when))continue;
        for(const item of step.entries){
            if(!matches(item.when))continue;
            const action=item.toggle&&flags[item.toggle.flag]?item.toggle.action:item.action;
            entries.set(action,matches(item.enabled));
        }
        if(step.stop)break;
    }
    return entries;
}
function contextMenuPosition(x,y,width,height,viewportWidth,viewportHeight) {
    return {left:Math.max(8,Math.min(x,viewportWidth-width-8)),top:Math.max(8,Math.min(y,viewportHeight-height-8))};
}
function createSurfaceContextMenu({root,document,window,snapshot,prepare,execute,restore,changed}) {
    const buttons=[...root.querySelectorAll("[data-context-action]")];
    let lease=null,disposed=false,dismissClick=false;
    const enabled=()=>buttons.filter(b=>!b.hidden&&!b.disabled);
    function close(focus=true){
        if(!lease)return;
        lease=null;root.hidden=true;changed();if(focus)restore();
    }
    function open(event,context){
        event.preventDefault();
        if(disposed||!event.isTrusted||!context)return;
        close(false);prepare();dismissClick=false;
        lease={...context,identity:snapshot()};
        const target=root.querySelector?.("#context-account-target");
        if(target){target.textContent=context.word?.id||"";target.hidden=!context.word;}
        for(const button of buttons){const state=context.entries.get(button.dataset.contextAction);button.hidden=state===undefined;button.disabled=state===false;}
        root.hidden=false;root.scrollTop=0;
        const bounds=root.getBoundingClientRect();
        const position=contextMenuPosition(event.clientX,event.clientY,bounds.width,bounds.height,window.innerWidth,window.innerHeight);
        root.style.left=position.left+"px";root.style.top=position.top+"px";
        changed();enabled()[0]?.focus({preventScroll:true});
    }
    function invoke(button,event){
        if(disposed||!event.isTrusted||!lease||button.hidden||button.disabled)return;
        const active=lease,action=button.dataset.contextAction;
        const valid=active.identity===snapshot()&&active.entries.get(action)===true;
        close();if(valid)execute(action,active,event);
    }
    function click(event){
        const button=event.target.closest("[data-context-action]");
        if(button&&buttons.includes(button)){event.preventDefault();event.stopPropagation();invoke(button,event);}
    }
    function key(event){
        if(!lease)return;
        // Menu keystrokes never reach the terminal or a floating-tool shortcut.
        event.stopImmediatePropagation();
        if(!event.isTrusted)return;
        if(["Escape","Tab"].includes(event.key)){event.preventDefault();close();return;}
        const available=enabled(),index=available.indexOf(document.activeElement);
        if(["ArrowDown","ArrowUp","Home","End"].includes(event.key)){
            event.preventDefault();const next=event.key==="Home"?0:event.key==="End"?available.length-1:(index+(event.key==="ArrowUp"?-1:1)+available.length)%available.length;
            available[next]?.focus();
        } else if(["Enter"," "].includes(event.key)){
            event.preventDefault();if(!event.repeat&&index>=0)invoke(available[index],event);
        } else if(!event.metaKey&&!event.ctrlKey)event.preventDefault();
    }
    function outside(event){
        if(!lease){dismissClick=false;return;}
        if(root.contains(event.target))return;
        event.preventDefault();event.stopImmediatePropagation();dismissClick=true;close();
    }
    function suppressClick(event){if(dismissClick){dismissClick=false;event.preventDefault();event.stopImmediatePropagation();}}
    function blur(){close(false);}
    function reposition(){close();}
    function scroll(event){if(lease&&!root.contains(event.target))close();}
    function validate(){if(lease&&lease.identity!==snapshot())close();}
    function destroy(){close(false);disposed=true;dismissClick=false;}
    return {open,close,validate,destroy,isOpen:()=>!!lease,clean:()=>disposed&&!lease&&root.hidden,
        bindings:[[root,"click",click],[document,"keydown",key,true],[document,"pointerdown",outside,true],
            [document,"click",suppressClick,true],[window,"blur",blur],[window,"resize",reposition],
            [document,"scroll",scroll,true]]};
}
