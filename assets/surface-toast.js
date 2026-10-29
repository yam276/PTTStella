// One bounded notification outlet, one observer and at most one expiry timer.
// Existing producers write plain text; no toast can send station input.
function createSurfaceToast({root,message,dismiss,document,MutationObserver,restore,
    now=()=>performance.now(),later=setTimeout,cancel=clearTimeout,duration=8000}) {
    let disposed=false,timer=null,remaining=duration,started=0,hovered=false,focused=false;
    function stopTimer(){if(timer!==null){cancel(timer);timer=null;remaining=Math.max(0,remaining-(now()-started));}}
    function arm(){
        stopTimer();
        if(disposed||root.hidden||hovered||focused)return;
        started=now();timer=later(()=>{timer=null;clear();},remaining);
    }
    function clear(){stopTimer();root.hidden=true;message.textContent="";remaining=duration;hovered=false;focused=false;}
    function update(){
        if(disposed)return;
        stopTimer();remaining=duration;
        root.hidden=!message.textContent.trim();
        if(!root.hidden)arm();
    }
    function enter(){hovered=true;stopTimer();}
    function leave(){hovered=false;arm();}
    function focus(){focused=true;stopTimer();}
    function blur(event){if(root.contains(event.relatedTarget))return;focused=false;arm();}
    function close(event){
        if(!event.isTrusted||disposed)return;
        const owned=root.contains(document.activeElement);
        event.preventDefault();event.stopPropagation();clear();if(owned)restore();
    }
    const observer=new MutationObserver(update);
    observer.observe(message,{childList:true,subtree:true,characterData:true});
    update();
    function destroy(){disposed=true;observer.disconnect();stopTimer();root.hidden=true;message.textContent="";}
    return {destroy,clean:()=>disposed&&timer===null&&root.hidden,
        bindings:[[root,"pointerenter",enter],[root,"pointerleave",leave],[root,"focusin",focus],[root,"focusout",blur],[dismiss,"click",close]]};
}
