// A single preview owns its browser resources. Descriptors come from Rust.
function installMedia({tr,send,prepare,changed,restore}) {
    const dialog=document.getElementById("media-dialog"),view=document.getElementById("media-viewport");
    const hover=document.getElementById("media-hover"),hoverView=document.getElementById("media-hover-viewport"),hoverNote=document.getElementById("media-hover-status");
    const title=document.getElementById("media-title"),urlText=document.getElementById("media-url"),status=document.getElementById("media-status");
    const buttons=Object.fromEntries(["minus","plus","fit","original","motion","retry","external","close"].map(name=>[name,document.getElementById("media-"+name)]));
    let current=null,image=null,video=null,still=null,stage=null,objectUrl=null,conversion=null,dead=false;
    let width=0,height=0,scale=1,fit=true,playing=false,animated=false;
    let presentation="inspect",point={x:0,y:0},position="top_right",positionFrame=0,state="closed",message="";
    const owner=()=>presentation==="hover"?hover:dialog,container=()=>presentation==="hover"?hoverView:view;
    const jobs=new Set();
    const note=(text,value)=>{message=text;state=value;(presentation==="hover"?hoverNote:status).textContent=text;owner().dataset.state=value;if(presentation==="hover")place();};
    function dispose() {
        conversion?.abort();conversion=null;
        if(image) {image.onload=null;image.onerror=null;image.removeAttribute("src");image=null;}
        if(video) {video.onplaying=null;video.onpause=null;video.onerror=null;video.onstalled=null;video.onwaiting=null;video.pause();video.removeAttribute("src");video.load();video=null;}
        if(still){still.width=0;still.height=0;still=null;}
        if(objectUrl){URL.revokeObjectURL(objectUrl);objectUrl=null;}
        if(positionFrame){cancelAnimationFrame(positionFrame);positionFrame=0;}
        view.replaceChildren();hoverView.replaceChildren();stage=null;width=0;height=0;playing=false;animated=false;
        owner().dataset.resources="0";buttons.motion.hidden=true;
    }
    function resize() {
        if(presentation==="hover") {place();return;}
        if(!stage||!width||!height) return;
        if(fit) scale=Math.min(1,Math.max(.01,view.clientWidth/width),Math.max(.01,view.clientHeight/height));
        stage.style.width=(width*scale)+"px";stage.style.height=(height*scale)+"px";
        dialog.dataset.scale=String(scale);
    }
    function close(e,notify=true) {
        if(e) e.preventDefault();
        const request=current?.request,modal=presentation==="inspect";current=null;dispose();hover.hidden=true;
        if(dialog.open) dialog.close();
        note("","closed");changed();
        if(notify&&request!==undefined&&!dead) send({kind:"media",request,action:"close",trusted:!!e?.isTrusted});
        if(!dead&&modal) restore();
    }
    function place() {
        if(presentation!=="hover"||hover.hidden) return;
        const bounds=document.querySelector(".workspace").getBoundingClientRect();
        // Budget the whole card, including its status and border, before fitting the image.
        const limitW=Math.max(1,Math.min(560,bounds.width*.55,bounds.width-24));
        const limitH=Math.max(1,Math.min(bounds.height*.6,bounds.height-24));
        hover.style.maxWidth=limitW+"px";
        const imageW=Math.max(1,limitW-10),imageH=Math.max(1,limitH-hoverNote.getBoundingClientRect().height-10);
        const scale=width&&height?Math.min(1,imageW/width,imageH/height):1;
        if(stage) {stage.style.width=(width*scale)+"px";stage.style.height=(height*scale)+"px";}
        const box=hover.getBoundingClientRect();
        let x=position==="top_right"?bounds.right-box.width-12:point.x+16;
        let y=position==="top_right"?bounds.top+12:point.y+16;
        if(position==="follow_cursor") {if(x+box.width>bounds.right-8)x=point.x-box.width-16;if(y+box.height>bounds.bottom-8)y=point.y-box.height-16;}
        x=Math.max(bounds.left+4,Math.min(x,bounds.right-box.width-4));y=Math.max(bounds.top+4,Math.min(y,bounds.bottom-box.height-4));
        hover.style.left=x+"px";hover.style.top=y+"px";
    }
    function locate(value,preferred=position) {point=value;position=preferred;
        if(!positionFrame&&!dead)positionFrame=requestAnimationFrame(()=>{positionFrame=0;place();});}
    function openHover(packet) {
        if(current?.request===packet.request) {locate(point,packet.position);return;}
        dispose();presentation="hover";current=packet;position=packet.position;hover.hidden=false;
        hover.dataset.request=String(packet.request);delete hover.dataset.cached;note(tr("image-loading"),"loading");place();
    }
    function hideHover(){if(presentation==="hover"){current=null;dispose();hover.hidden=true;note("","closed");}}
    function open(packet) {
        const promote=presentation==="hover"&&current?.request===packet.request;
        const oldState=state,oldMessage=message,cached=hover.dataset.cached;
        if(!promote)dispose();current=packet;presentation="inspect";hover.hidden=true;fit=true;scale=1;
        if(promote&&stage)view.append(stage);
        title.textContent=packet.target.kind==="youtube"?tr("youtube-title"):packet.target.kind==="video"?tr("video-title"):tr("image-title");
        urlText.textContent=packet.target.url;
        dialog.dataset.request=String(packet.request);dialog.dataset.kind=packet.target.kind;delete dialog.dataset.cached;
        delete dialog.dataset.external;delete dialog.dataset.frame;
        for(const name of ["minus","plus","fit","original"]) buttons[name].disabled=packet.target.kind==="video";
        prepare();if(!dialog.open) dialog.showModal();changed();buttons.close.focus();
        if(promote) {note(oldMessage,oldState);dialog.dataset.resources=objectUrl?"1":"0";if(cached!==undefined)dialog.dataset.cached=cached;buttons.motion.hidden=!animated;resize();return;}
        if(packet.target.kind==="video") openVideo();
        else note(packet.target.kind==="youtube"?tr("youtube-loading"):tr("image-loading"),"loading");
    }
    function openVideo() {
        const request=current.request;
        dispose();stage=document.createElement("div");stage.className="media-video-stage";
        video=document.createElement("video");video.controls=true;video.preload="none";video.playsInline=true;
        video.setAttribute("aria-label",tr("video-player"));
        video.onplaying=()=>{if(current?.request===request) {buttons.motion.textContent=tr("video-pause");note(tr("media-playing"),"playing");}};
        video.onpause=()=>{if(current?.request===request) {buttons.motion.textContent=tr("video-play");note(tr("media-paused"),"ready");}};
        video.onerror=()=>{if(current?.request===request) note(tr("video-unsupported"),"failed");};
        video.onstalled=()=>{if(current?.request===request) note(tr("video-stalled"),"stalled");};
        video.onwaiting=()=>{if(current?.request===request) note(tr("video-buffering"),"buffering");};
        video.src=current.target.url;stage.append(video);view.append(stage);owner().dataset.resources="1";
        buttons.motion.hidden=false;buttons.motion.textContent=tr("video-play");
        note(tr("video-ready"),"ready");
    }
    function snapshot() {
        if(!image||!still) return;
        // Canvas uses the default/first frame of an animated image, not its current frame.
        const context=still.getContext("2d");context.clearRect(0,0,width,height);context.drawImage(image,0,0,width,height);
        still.hidden=false;image.hidden=true;image.removeAttribute("src");playing=false;buttons.motion.textContent=tr("media-play");
        dialog.dataset.motion="paused";
    }
    function receiveImage(packet) {
        if(!current||packet.request!==current.request||dead) return;
        const request=current.request;
        dispose();note(tr("image-decoding"),"loading");
        const controller=new AbortController();conversion=controller;
        // Browser decodes base64 asynchronously; JS does not build an 8 MiB array byte by byte.
        const job=(async()=>{
            try {
                const response=await fetch("data:"+packet.mime+";base64,"+packet.data,{signal:controller.signal});
                const blob=await response.blob();
                if(dead||controller.signal.aborted||current?.request!==request) return;
                objectUrl=URL.createObjectURL(blob);owner().dataset.resources="1";
                width=packet.width;height=packet.height;animated=packet.animated;playing=false;
                stage=document.createElement("div");stage.className="media-image-stage";
                image=document.createElement("img");image.alt=tr("media-title");image.draggable=false;
                still=document.createElement("canvas");still.width=width;still.height=height;still.hidden=true;
                image.onload=()=>{
                    if(current?.request!==request||dead) return;
                    if(image.naturalWidth!==width||image.naturalHeight!==height) {fail({request,message:tr("image-size-mismatch")});return;}
                    if(animated&&!playing) snapshot();
                    owner().dataset.cached=String(packet.cached);buttons.motion.hidden=!animated||presentation==="hover";
                    note(current.target.kind==="youtube"?tr("youtube-ready"):tr(packet.cached?"image-ready-cached":"image-ready"),"ready");
                    resize();
                    send({kind:"media_decoded",request,success:true});
                };
                image.onerror=()=>fail({request,message:tr("image-decode-failed")});
                stage.append(image,still);container().append(stage);resize();image.src=objectUrl;
            } catch(error) {
                if(!controller.signal.aborted) fail({request,message:tr("image-prepare-failed")});
            }
        })();
        jobs.add(job);job.finally(()=>jobs.delete(job));
    }
    function fail(packet) {
        if(current?.request!==packet.request||dead) return;
        dispose();note(packet.message,"failed");
        send({kind:"media_decoded",request:packet.request,success:false});
    }
    function retry(e) {
        if(!current||!e.isTrusted) return;
        if(current.target.kind==="video") {openVideo();return;}
        dispose();note(tr("image-reloading"),"loading");send({kind:"media",request:current.request,action:"retry",trusted:e.isTrusted});
    }
    function motion(e) {
        if(e.isTrusted&&video&&current) {
            if(!video.paused) {video.pause();return;}
            const request=current.request;
            const job=video.play().catch(()=>{if(current?.request===request) note(tr("video-play-failed"),"failed");});
            jobs.add(job);job.finally(()=>jobs.delete(job));return;
        }
        if(!e.isTrusted||!animated||!image||!objectUrl) return;
        if(playing){snapshot();return;}
        playing=true;still.hidden=true;image.hidden=false;buttons.motion.textContent=tr("media-stop");
        dialog.dataset.motion="playing";image.src=objectUrl;
    }
    function external(e) {if(current&&e.isTrusted) send({kind:"media",request:current.request,action:"external",trusted:e.isTrusted});}
    function zoom(delta) {if(width){fit=false;scale=Math.max(.05,Math.min(8,scale+delta));resize();}}
    const bindings=[
        [buttons.close,"click",close],[dialog,"cancel",close],
        [buttons.minus,"click",()=>zoom(-.25)],[buttons.plus,"click",()=>zoom(.25)],
        [buttons.fit,"click",()=>{fit=true;resize();}],[buttons.original,"click",()=>{fit=false;scale=1;resize();}],
        [buttons.retry,"click",retry],[buttons.motion,"click",motion],[buttons.external,"click",external],
    ];
    for(const [target,event,callback] of bindings) target.addEventListener(event,callback);
    const observer=new ResizeObserver(resize);observer.observe(view);observer.observe(document.querySelector(".workspace"));observer.observe(document.getElementById("chrome"));
    return {
        open,receiveImage,fail,hover:openHover,hideHover,locate,reset:()=>close(null,false),
        externalResult(packet){if(current?.request===packet.request){dialog.dataset.external=String(packet.ok);if(!packet.ok) status.textContent=packet.message;}},
        async destroy(){dead=true;for(const [target,event,callback] of bindings) target.removeEventListener(event,callback);
            observer.disconnect();close(null,false);await Promise.allSettled([...jobs]);
            return !objectUrl&&!conversion&&!dialog.open&&!view.childElementCount&&!hoverView.childElementCount&&!positionFrame&&!jobs.size;},
    };
}
