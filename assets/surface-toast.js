// One notification outlet and at most one timer. Producers supply semantic kinds;
// translated message text never decides whether a failure can disappear.
function createSurfaceToast({root,message,dismiss,document,restore,motion=null,
    now=()=>performance.now(),later=setTimeout,cancel=clearTimeout}) {
    let disposed=false,timer=null,epoch=0,remaining=0,started=0,hovered=false,focused=false,phase="idle",kind="persistent";
    function stopTimer(){epoch++;if(timer!==null){cancel(timer);timer=null;remaining=Math.max(0,remaining-(now()-started));}}
    function opaque(){root.dataset.fading="false";root.style.transitionDuration="0ms";}
    function clear(){stopTimer();phase="idle";hovered=false;focused=false;root.hidden=true;message.textContent="";opaque();}
    function arm(){
        if(disposed||root.hidden||hovered||focused||phase==="persistent"||timer!==null)return;
        if(phase==="fade"&&motion?.matches){clear();return;}
        if(phase==="fade"){root.style.transitionDuration=remaining+"ms";root.dataset.fading="true";}
        started=now();const ticket=++epoch;
        timer=later(()=>{
            if(disposed||ticket!==epoch)return;
            timer=null;
            if(phase==="wait"&&!motion?.matches){phase="fade";remaining=500;arm();}
            else clear();
        },remaining);
    }
    function show(text,nextKind="persistent"){
        if(disposed||!text?.trim())return;
        // An unrelated late success must not erase a failure awaiting attention.
        if(!root.hidden&&kind==="error"&&["copy","success"].includes(nextKind))return;
        stopTimer();opaque();kind=nextKind;message.textContent=text;root.hidden=false;
        root.dataset.level=kind==="error"?"error":"notice";
        focused=root.contains(document.activeElement);
        phase=["copy","success"].includes(kind)?"wait":"persistent";
        remaining=kind==="copy"?2000:3000;arm();
    }
    function enter(){hovered=true;stopTimer();opaque();}
    function leave(){hovered=false;arm();}
    function focus(){focused=true;stopTimer();opaque();}
    function blur(event){if(root.contains(event.relatedTarget))return;focused=false;arm();}
    function motionChanged(){if(motion?.matches&&phase==="fade"&&!hovered&&!focused)clear();}
    function close(event){
        if(!event.isTrusted||disposed)return;
        const owned=root.contains(document.activeElement);
        event.preventDefault();event.stopPropagation();clear();if(owned)restore();
    }
    motion?.addEventListener("change",motionChanged);
    function destroy(){disposed=true;motion?.removeEventListener("change",motionChanged);clear();}
    return {show,clear,destroy,clean:()=>disposed&&timer===null&&root.hidden,
        bindings:[[root,"pointerenter",enter],[root,"pointerleave",leave],[root,"focusin",focus],[root,"focusout",blur],[dismiss,"click",close]]};
}
