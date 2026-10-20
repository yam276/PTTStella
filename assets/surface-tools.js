// Owns only tool chrome geometry and pointer capture; never sends station input.
function createFloatingTools({root,ball,fan,panel,window,document,tr,prepare,restore,changed=()=>{},
    raf=window.requestAnimationFrame.bind(window),caf=window.cancelAnimationFrame.bind(window),
    now=()=>performance.now(),reduced=()=>window.matchMedia('(prefers-reduced-motion: reduce)').matches}) {
    const petals=[...fan.querySelectorAll('[data-tools-target]')];
    const groups=[...panel.querySelectorAll('[data-tools-group]')];
    let point={x:window.innerWidth-80,y:window.innerHeight*.65},drag=null,animation=0,stopped=false,opened=false,selected=null;
    let suppressClickUntil=-1,lastFrame=0,coastUntil=0,velocity={x:0,y:0};
    let viewportWidth=window.innerWidth;
    let visible=true,parked=null;
    const observations={presses:0,moves:0,drag_releases:0,coast_frames:0,max_coast_distance:0,last_release:null};
    const count=key=>observations[key]=Math.min(65535,observations[key]+1);
    const bounds=()=>({left:16,right:Math.max(16,window.innerWidth-72),top:Math.min(56,Math.max(0,window.innerHeight-72)),bottom:Math.max(16,window.innerHeight-72)});
    const clamp=(value,min,max)=>Math.max(min,Math.min(Math.max(min,max),value));
    function place(){const b=bounds();point.x=clamp(point.x,b.left,b.right);point.y=clamp(point.y,b.top,b.bottom);
        root.style.setProperty('--tools-x',`${point.x}px`);root.style.setProperty('--tools-y',`${point.y}px`);}
    function setOpacity(percent){
        if(!Number.isInteger(percent)||percent<20||percent>100)throw new Error("Invalid tool opacity");
        root.style.setProperty("--tools-opacity",String(percent/100));
    }
    function halt(){if(animation)caf(animation);animation=0;velocity={x:0,y:0};}
    function updateTrigger(){
        ball.hidden=!visible&&!opened;
        const returning=selected!==null;
        ball.dataset.toolsAction=returning?'back':'toggle';
        ball.setAttribute('aria-label',tr(returning?'tools-back':'tools-toggle'));
        ball.setAttribute('title',tr(returning?'tools-back-drag-hint':'tools-drag-hint'));
        ball.setAttribute('aria-controls',returning?'tools-fan':'tools-fan tools-panel');
        if(returning)ball.removeAttribute('aria-expanded');else ball.setAttribute('aria-expanded',String(opened));
    }
    function dock(){const b=bounds();point.x=point.x+28<window.innerWidth/2?b.left:b.right;place();}
    function frame(time){animation=0;if(stopped||drag)return;
        const dt=Math.min(32,Math.max(0,time-lastFrame));lastFrame=time;
        if(time<coastUntil&&Math.hypot(velocity.x,velocity.y)>.025){
            count('coast_frames');
            const b=bounds(),next={x:point.x+velocity.x*dt,y:point.y+velocity.y*dt};
            if(next.x<b.left||next.x>b.right)velocity.x=0;
            if(next.y<b.top||next.y>b.bottom)velocity.y=0;
            point=next;velocity.x*=Math.exp(-dt/220);velocity.y*=Math.exp(-dt/220);place();
            const release=observations.last_release;
            if(release)observations.max_coast_distance=Math.max(observations.max_coast_distance,Math.hypot(point.x-release.x,point.y-release.y));
        }else{
            const b=bounds(),goal=point.x+28<window.innerWidth/2?b.left:b.right;
            point.x+=(goal-point.x)*(1-Math.exp(-dt/110));place();
            if(Math.abs(goal-point.x)<.5){point.x=goal;place();return;}
        }
        animation=raf(frame);
    }
    function close(focus=false){opened=false;selected=null;root.classList.remove('open');fan.hidden=true;panel.hidden=true;
        if(parked){point=parked;parked=null;place();}
        updateTrigger();petals.forEach(p=>p.setAttribute('aria-expanded','false'));
        changed();if(focus&&!stopped)restore();}
    // Largest usable rectangle on one side of the stationary trigger. Both the
    // grid fallback and the feature page reserve this space, including a gap.
    function pageArea(){
        const right=window.innerWidth-16,bottom=window.innerHeight-16;
        return [
            {x:16,y:48,w:point.x-28,h:bottom-48},
            {x:point.x+68,y:48,w:right-point.x-68,h:bottom-48},
            {x:16,y:48,w:right-16,h:point.y-60},
            {x:16,y:point.y+68,w:right-16,h:bottom-point.y-68},
        ].filter(r=>r.w>0&&r.h>0).sort((a,b)=>Math.min(330,b.w)*Math.min(260,b.h)-Math.min(330,a.w)*Math.min(260,a.h))[0];
    }
    function layout(){
        const radius=148;
        const fanY=clamp(point.y,48+radius,window.innerHeight-78-radius);
        const direction=point.x+28<window.innerWidth/2?1:-1;
        // A full semicircle gives the rectangular hit areas room at its ends.
        // Offsetting its origin inward also keeps every category off the ball.
        const positions=petals.map((_,index)=>{const angle=(-90+index*36)*Math.PI/180;
            return {x:point.x+direction*(72+radius*Math.cos(angle)),y:fanY+radius*Math.sin(angle)};});
        const fits=window.innerWidth>=500&&window.innerHeight>=420&&positions.every(p=>p.x>=16&&p.x+62<=window.innerWidth-16&&p.y>=48&&p.y+62<=window.innerHeight-16);
        root.dataset.toolsLayout=fits?'fan':'compact';
        petals.forEach((petal,index)=>{
            petal.style.setProperty('--petal-x',`${positions[index].x-point.x}px`);
            petal.style.setProperty('--petal-y',`${positions[index].y-point.y}px`);});
        const area=pageArea();
        if(!area){close();return;} // No usable content space in a collapsed host.
        const width=Math.min(330,area.w),height=Math.min(260,area.h);
        const x=clamp(point.x,area.x,area.x+area.w-width),y=clamp(point.y-height/2,area.y,area.y+area.h-height);
        for(const [key,value] of Object.entries({x,y,width,height}))fan.style.setProperty(`--grid-${key}`,`${value}px`);
        panel.style.left=`${x}px`;panel.style.top=`${y}px`;
        panel.style.width=`${width}px`;panel.style.maxHeight=`${height}px`;
        fan.hidden=selected!==null;panel.hidden=selected===null;
        updateTrigger();
    }
    function toggle(event){if(stopped)return;
        // WebViews may give the drag's generated click detail=0. Do not let it
        // open tools and halt the just-started coast. New taps/keyboard input
        // clear this bounded guard; a missing click cannot swallow a later AX action.
        const suppressed=now()<=suppressClickUntil;suppressClickUntil=-1;if(suppressed)return;
        halt();if(opened){if(selected!==null)back();else close(true);return;}prepare(event);opened=true;layout();
        if(!opened)return;root.classList.add('open');ball.focus();changed();}
    function showAt(event){
        if(stopped||drag||!event.isTrusted||!Number.isFinite(event.clientX)||!Number.isFinite(event.clientY))return false;
        halt();close();prepare(event);suppressClickUntil=-1;
        parked={...point};point={x:event.clientX-28,y:event.clientY-28};place();opened=true;layout();
        if(opened){root.classList.add('open');ball.focus();changed();}
        return opened;
    }
    function setVisible(value){
        if(typeof value!=="boolean")throw new Error("Invalid tool visibility");
        visible=value;updateTrigger();
    }
    function choose(event){if(stopped||!opened)return;
        selected=event.currentTarget.dataset.toolsTarget;
        groups.forEach(group=>group.hidden=group.dataset.toolsGroup!==selected);
        petals.forEach(p=>p.setAttribute('aria-expanded',String(p.dataset.toolsTarget===selected)));
        layout();
        panel.querySelector('section:not([hidden]) button:not(:disabled):not([hidden]),section:not([hidden]) select')?.focus();}
    function back(){if(!opened||selected===null)return;
        const previous=petals.find(p=>p.dataset.toolsTarget===selected);selected=null;
        groups.forEach(group=>group.hidden=true);petals.forEach(p=>p.setAttribute('aria-expanded','false'));
        layout();previous?.focus();changed();}
    function down(event){if(stopped||event.button!==0||!event.isPrimary||drag)return;
        count('presses');
        halt();suppressClickUntil=-1;prepare(event);
        drag={id:event.pointerId,x:event.clientX,y:event.clientY,start:{...point},history:[{...point,time:now()}],direction:null,moved:false};
        changed();
        ball.setPointerCapture(event.pointerId);event.preventDefault();event.stopPropagation();}
    function move(event){if(!drag||event.pointerId!==drag.id)return;
        if(!(event.buttons&1)){cancel();return;}
        count('moves');
        track(event);event.preventDefault();event.stopPropagation();}
    function track(event){
        const time=now(),dx=event.clientX-drag.x,dy=event.clientY-drag.y;
        if(!drag.moved&&Math.hypot(dx,dy)>6){drag.moved=true;close();}
        if(!drag.moved)return;
        const previous=point;
        point={x:drag.start.x+dx,y:drag.start.y+dy};place();
        const travel={x:point.x-previous.x,y:point.y-previous.y},distance=Math.hypot(travel.x,travel.y);
        if(distance>0)drag.direction={x:travel.x/distance,y:travel.y/distance};
        // Keep recent geometry, including the pointerup endpoint. Equal-time
        // samples replace their predecessor instead of inventing a 1ms velocity.
        const history=drag.history;
        if(history.at(-1).time===time)history.pop();
        history.push({...point,time});
        drag.history=history.filter(sample=>time-sample.time<=120).slice(-16);
    }
    function launchVelocity(history){
        if(history.length<2)return {x:0,y:0};
        const origin=history[0].time,n=history.length;
        const mean=history.reduce((sum,s)=>({t:sum.t+s.time-origin,x:sum.x+s.x,y:sum.y+s.y}),{t:0,x:0,y:0});
        mean.t/=n;mean.x/=n;mean.y/=n;
        let variance=0,x=0,y=0;
        for(const sample of history){const dt=sample.time-origin-mean.t;variance+=dt*dt;x+=dt*(sample.x-mean.x);y+=dt*(sample.y-mean.y);}
        return variance>0?{x:clamp(x/variance,-2,2),y:clamp(y/variance,-2,2)}:{x:0,y:0};
    }
    function releaseCapture(id){if(ball.hasPointerCapture(id))ball.releasePointerCapture(id);}
    function releaseVelocity(held){
        const measured=launchVelocity(held.history),minimum=.1;
        // A slow or paused drag still settles about 15px in its last actual
        // direction. The short coast uses the same decay and boundary handling
        // as a flick; it never revives the old high speed after a pause.
        return held.direction&&Math.hypot(measured.x,measured.y)<minimum
            ?{x:held.direction.x*minimum,y:held.direction.y*minimum}:measured;
    }
    function up(event){if(!drag||event.pointerId!==drag.id||event.button!==0)return;
        track(event);const held=drag;drag=null;releaseCapture(held.id);suppressClickUntil=held.moved?now()+300:-1;
        if(held.moved){velocity=releaseVelocity(held);const reduceMotion=reduced();
            count('drag_releases');observations.coast_frames=0;observations.max_coast_distance=0;
            observations.last_release={...point,vx:velocity.x,vy:velocity.y,reduced:reduceMotion,samples:held.history.length};
            if(reduceMotion){dock();}else{lastFrame=now();coastUntil=lastFrame+900;animation=raf(frame);}}
        changed();event.preventDefault();event.stopPropagation();}
    function cancel(){if(drag){const id=drag.id;drag=null;suppressClickUntil=now()+300;releaseCapture(id);}halt();close();place();}
    function lost(event){if(drag?.id===event.pointerId)cancel();}
    function outside(event){if(opened&&!root.contains(event.target)){
        // A modal can open asynchronously after a toolbar action. Its first
        // pointer event belongs to that dialog, including native input focus.
        if(document.querySelector('dialog[open]')){close();return;}
        event.preventDefault();event.stopPropagation();close(true);}}
    function key(event){
        if(event.target===ball&&(event.key==='Enter'||event.key===' '))suppressClickUntil=-1;
        if(!opened)return;
        if(document.querySelector('dialog[open]')){close();return;}
        if(event.key==='Escape'){event.preventDefault();event.stopPropagation();close();ball.focus();}
        else if(event.key==='ArrowDown'||event.key==='ArrowUp'){
            if(fan.hidden)return;
            if(!fan.contains(document.activeElement)&&document.activeElement!==ball)return;
            event.preventDefault();event.stopPropagation();const i=petals.indexOf(document.activeElement),step=event.key==='ArrowDown'?1:-1;
            petals[i<0?(step===1?0:petals.length-1):(i+step+petals.length)%petals.length].focus();}
    }
    function resize(){const left=point.x+28<viewportWidth/2;viewportWidth=window.innerWidth;
        halt();if(drag)cancel();place();if(opened)layout();else{const b=bounds();point.x=left?b.left:b.right;place();}}
    function visibility(){if(document.hidden)cancel();}
    function panelAction(event){
        const button=event.target.closest?.('button');
        if(opened&&!panel.hidden&&button&&!document.querySelector('dialog[open]'))button.focus();
    }
    const bindings=[[ball,'pointerdown',down],[ball,'pointermove',move],[ball,'pointerup',up],
        [ball,'pointercancel',cancel],[ball,'lostpointercapture',lost],[ball,'click',toggle],
        ...petals.map(p=>[p,'click',choose]),[document,'pointerdown',outside,true],
        [panel,'click',panelAction],
        [document,'keydown',key,true],[window,'resize',resize],[window,'blur',cancel],[document,'visibilitychange',visibility]];
    place();close();
    return {bindings,close,setOpacity,setVisible,showAt,open:()=>opened,busy:()=>opened||!!drag,destroy(){stopped=true;cancel();},
        observations:()=>({...observations,last_release:observations.last_release?{...observations.last_release}:null}),
        clean:()=>stopped&&!animation&&!drag&&!opened,
        // Read-only geometry for lifecycle/physics regressions.
        snapshot:()=>({point:{...point},animation:!!animation,dragging:!!drag,opened,selected})};
}
