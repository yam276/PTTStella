// Owns only tool chrome geometry and pointer capture; never sends station input.
function createFloatingTools({root,ball,fan,panel,window,document,prepare,restore,changed=()=>{},
    raf=window.requestAnimationFrame.bind(window),caf=window.cancelAnimationFrame.bind(window),
    now=()=>performance.now(),reduced=()=>window.matchMedia('(prefers-reduced-motion: reduce)').matches}) {
    const petals=[...fan.querySelectorAll('[data-tools-target]')];
    const groups=[...panel.querySelectorAll('[data-tools-group]')];
    let point={x:window.innerWidth-80,y:window.innerHeight*.65},drag=null,animation=0,stopped=false,opened=false,selected=null;
    let suppressClick=false,lastFrame=0,coastUntil=0,velocity={x:0,y:0};
    let viewportWidth=window.innerWidth;
    const observations={presses:0,moves:0,drag_releases:0,coast_frames:0,max_coast_distance:0,last_release:null};
    const count=key=>observations[key]=Math.min(65535,observations[key]+1);
    const bounds=()=>({left:16,right:Math.max(16,window.innerWidth-72),top:Math.min(56,Math.max(0,window.innerHeight-72)),bottom:Math.max(16,window.innerHeight-72)});
    const clamp=(value,min,max)=>Math.max(min,Math.min(Math.max(min,max),value));
    function place(){const b=bounds();point.x=clamp(point.x,b.left,b.right);point.y=clamp(point.y,b.top,b.bottom);
        root.style.setProperty('--tools-x',`${point.x}px`);root.style.setProperty('--tools-y',`${point.y}px`);}
    function halt(){if(animation)caf(animation);animation=0;velocity={x:0,y:0};}
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
        ball.setAttribute('aria-expanded','false');petals.forEach(p=>p.setAttribute('aria-expanded','false'));
        changed();if(focus&&!stopped)restore();}
    function layout(){
        const compact=window.innerWidth<500||window.innerHeight<420;
        root.dataset.toolsLayout=compact?'compact':'fan';
        if(compact){panel.style.left='16px';panel.style.top='80px';panel.style.width=`${Math.max(1,window.innerWidth-32)}px`;panel.style.maxHeight=`${Math.max(32,window.innerHeight-96)}px`;return;}
        const radius=Math.min(148,Math.max(92,(window.innerHeight-100)/2));
        point.y=clamp(point.y,radius+8,window.innerHeight-radius-64);place();
        const direction=point.x+28<window.innerWidth/2?1:-1;
        petals.forEach((petal,index)=>{const angle=(-75+index*30)*Math.PI/180;
            petal.style.setProperty('--petal-x',`${direction*radius*Math.cos(angle)}px`);
            petal.style.setProperty('--petal-y',`${radius*Math.sin(angle)}px`);});
        const width=Math.min(330,window.innerWidth-32),height=Math.min(260,window.innerHeight-80);
        const x=direction===1?point.x+radius+70:point.x-radius-width-14;
        panel.style.left=`${clamp(x,16,window.innerWidth-width-16)}px`;
        panel.style.top=`${clamp(point.y-height/2,48,window.innerHeight-height-16)}px`;
        panel.style.width=`${width}px`;panel.style.maxHeight=`${height}px`;
    }
    function toggle(event){if(stopped)return;if(suppressClick&&event.detail!==0){suppressClick=false;return;}suppressClick=false;
        halt();if(opened){close(true);return;}prepare(event);opened=true;layout();
        root.classList.add('open');fan.hidden=false;ball.setAttribute('aria-expanded','true');ball.focus();changed();}
    function choose(event){if(stopped||!opened)return;
        selected=event.currentTarget.dataset.toolsTarget;
        groups.forEach(group=>group.hidden=group.dataset.toolsGroup!==selected);
        petals.forEach(p=>p.setAttribute('aria-expanded',String(p.dataset.toolsTarget===selected)));
        panel.hidden=false;layout();if(root.dataset.toolsLayout==='compact')fan.hidden=true;
        panel.querySelector('section:not([hidden]) button:not(:disabled):not([hidden]),section:not([hidden]) select')?.focus();}
    function down(event){if(stopped||event.button!==0||!event.isPrimary||drag)return;
        count('presses');
        halt();suppressClick=false;prepare(event);
        drag={id:event.pointerId,x:event.clientX,y:event.clientY,start:{...point},history:[{...point,time:now()}],moved:false};
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
        point={x:drag.start.x+dx,y:drag.start.y+dy};place();
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
    function up(event){if(!drag||event.pointerId!==drag.id||event.button!==0)return;
        track(event);const held=drag;drag=null;releaseCapture(held.id);suppressClick=held.moved;
        if(held.moved){velocity=launchVelocity(held.history);const reduceMotion=reduced();
            count('drag_releases');observations.coast_frames=0;observations.max_coast_distance=0;
            observations.last_release={...point,vx:velocity.x,vy:velocity.y,reduced:reduceMotion,samples:held.history.length};
            if(reduceMotion){dock();}else{lastFrame=now();coastUntil=lastFrame+900;animation=raf(frame);}}
        changed();event.preventDefault();event.stopPropagation();}
    function cancel(){if(drag){const id=drag.id;drag=null;suppressClick=true;releaseCapture(id);}halt();close();place();}
    function lost(event){if(drag?.id===event.pointerId)cancel();}
    function outside(event){if(opened&&!root.contains(event.target)){event.preventDefault();event.stopPropagation();close(true);}}
    function key(event){if(!opened)return;
        if(event.key==='Escape'){event.preventDefault();event.stopPropagation();close();ball.focus();}
        else if(event.key==='ArrowDown'||event.key==='ArrowUp'){
            if(!fan.contains(document.activeElement)&&document.activeElement!==ball)return;
            event.preventDefault();event.stopPropagation();const i=petals.indexOf(document.activeElement),step=event.key==='ArrowDown'?1:-1;
            petals[i<0?(step===1?0:petals.length-1):(i+step+petals.length)%petals.length].focus();}
    }
    function resize(){const left=point.x+28<viewportWidth/2;viewportWidth=window.innerWidth;
        halt();if(drag)cancel();place();if(opened)layout();else{const b=bounds();point.x=left?b.left:b.right;place();}}
    function visibility(){if(document.hidden)cancel();}
    function panelAction(event){
        const button=event.target.closest?.('button');
        if(opened&&button&&!document.querySelector('dialog[open]'))button.focus();
    }
    const bindings=[[ball,'pointerdown',down],[ball,'pointermove',move],[ball,'pointerup',up],
        [ball,'pointercancel',cancel],[ball,'lostpointercapture',lost],[ball,'click',toggle],
        ...petals.map(p=>[p,'click',choose]),[document,'pointerdown',outside,true],
        [panel,'click',panelAction],
        [document,'keydown',key,true],[window,'resize',resize],[window,'blur',cancel],[document,'visibilitychange',visibility]];
    place();close();
    return {bindings,close,open:()=>opened,busy:()=>opened||!!drag,destroy(){stopped=true;cancel();},
        observations:()=>({...observations,last_release:observations.last_release?{...observations.last_release}:null}),
        clean:()=>stopped&&!animation&&!drag&&!opened,
        // Read-only geometry for lifecycle/physics regressions.
        snapshot:()=>({point:{...point},animation:!!animation,dragging:!!drag,opened,selected})};
}
