// Explicit diagnostic builds only. No key names, text, URLs or DOM text.
function createSurfaceDiagnostics({send,snapshot,document,window,clock=()=>performance.now(),
    every=setInterval,cancel=clearInterval}) {
    let active=false,timer=null,last=clock(),period=last,count=0,packets=0,suppressed=0;
    const bindings=[];
    const number=value=>Number.isFinite(value)?Math.max(0,Math.min(0xffffffff,Math.round(value))):0;
    function note(event,phase=0,target=0,elapsed=0,force=false) {
        if(!active)return;
        if(event==="packet"&&elapsed<16)return;
        const now=clock();
        if(now-period>=1000){period=now;count=0;packets=0;}
        const packet=event==="packet";
        if(!force&&(packet?packets>=8:count>=32)){suppressed=number(suppressed+1);return;}
        if(!force){if(packet)packets++;else count++;}
        try {
            const state=snapshot();
            send({kind:"diagnostics",trace:{event,phase:phase&255,target:target&255,
                flags:number(state.flags),generation:number(state.generation),revision:number(state.revision),
                presentation:number(state.presentation),elapsed_ms:number(elapsed),scroll:number(state.scroll),
                extent:number(state.extent),suppressed}});
            suppressed=0;
        } catch(_) { destroy(); } // Telemetry failure must not terminate reading/input.
    }
    function target(event) {
        const value=event.target;
        const base=value?.id==="keyboard"?1:value?.closest?.("dialog")?3:value?.closest?.("#chrome")?4:value?.closest?.(".workspace")?2:0;
        if(event.type!=="keydown")return base;
        const key=typeof event.key==="string"?event.key:"";
        const shape=/^[a-z]$/i.test(key)?1:/^[\x00-\x1f\x7f]$/.test(key)?2:
            ["Process","Dead","Unidentified"].includes(key)?3:key.length>1?4:key.length===1?(key.charCodeAt(0)>127?5:6):0;
        return base|(shape<<4)|(/^Key[A-Z]$/.test(event.code)?128:0);
    }
    function phase(event,stage) {
        if(event.type!=="keydown")return stage;
        const control=typeof event.key==="string"&&/^[\x00-\x1f\x7f]$/.test(event.key);
        return stage|(event.ctrlKey?4:0)|(event.metaKey?8:0)|(event.altKey?16:0)|(event.shiftKey?32:0)
            |(event.isComposing||event.keyCode===229||event.key==="Process"?64:0)|(control?128:0);
    }
    function bind(element,name,callback,options) {
        element.addEventListener(name,callback,options);bindings.push([element,name,callback,options]);
    }
    function enable(value) {
        if(value!==true||active)return;
        active=true;last=clock();period=last;
        const names={keydown:"key",input:"text",pointerdown:"pointer_down",pointerup:"pointer_up",
            click:"click",wheel:"wheel",focusin:"focus",focusout:"focus",visibilitychange:"visibility",
            compositionstart:"composition",compositionend:"composition"};
        for(const [name,kind] of Object.entries(names)) {
            bind(document,name,e=>note(kind,phase(e,e.defaultPrevented?1:0),target(e)),{capture:true,passive:true});
            if(["keydown","click","wheel"].includes(name))
                bind(document,name,e=>note(kind,phase(e,e.defaultPrevented?3:2),target(e)),{passive:true});
        }
        bind(window,"keydown",e=>{
            if(e.isTrusted&&!e.repeat&&(e.metaKey||e.ctrlKey)&&e.shiftKey&&!e.altKey&&e.code==="KeyL") {
                e.preventDefault();e.stopImmediatePropagation();note("marker",0,0,0,true);
            }
        },true);
        bind(window,"error",()=>note("error"));
        bind(window,"unhandledrejection",()=>note("error",1));
        timer=every(()=>{const now=clock();note("pulse",0,0,now-last,true);last=now;},1000);
        note("pulse",0,0,0,true);
    }
    function destroy() {
        active=false;
        if(timer!==null){cancel(timer);timer=null;}
        for(const [element,name,callback,options] of bindings)element.removeEventListener(name,callback,options);
        bindings.length=0;
    }
    return {enable,note,destroy};
}
