// Owns only tool chrome geometry and pointer capture; never sends station input.
function createFloatingTools({root,ball,fan,panel,window,document,prepare,restore,changed=()=>{},
    raf=window.requestAnimationFrame.bind(window),caf=window.cancelAnimationFrame.bind(window),
    now=()=>performance.now(),reduced=()=>window.matchMedia('(prefers-reduced-motion: reduce)').matches}) {
    const petals=[...fan.querySelectorAll('[data-tools-target]')];
    const groups=[...panel.querySelectorAll('[data-tools-group]')];
    let point={x:window.innerWidth-80,y:window.innerHeight*.65},drag=null,animation=0,stopped=false,opened=false,selected=null;
    let suppressClick=false,lastFrame=0,coastUntil=0,velocity={x:0,y:0};
    const bounds=()=>({left:16,right:Math.max(16,window.innerWidth-72),top:Math.min(56,Math.max(0,window.innerHeight-72)),bottom:Math.max(16,window.innerHeight-72)});
    const clamp=(value,min,max)=>Math.max(min,Math.min(Math.max(min,max),value));
    function place(){const b=bounds();point.x=clamp(point.x,b.left,b.right);point.y=clamp(point.y,b.top,b.bottom);
        root.style.setProperty('--tools-x',`${point.x}px`);root.style.setProperty('--tools-y',`${point.y}px`);}
    function halt(){if(animation)caf(animation);animation=0;velocity={x:0,y:0};}
    function dock(){const b=bounds();point.x=point.x+28<window.innerWidth/2?b.left:b.right;place();}
    function frame(time){animation=0;if(stopped||drag)return;
        const dt=Math.min(32,Math.max(0,time-lastFrame));lastFrame=time;
        if(time<coastUntil&&Math.hypot(velocity.x,velocity.y)>.025){
            const b=bounds(),next={x:point.x+velocity.x*dt,y:point.y+velocity.y*dt};
            if(next.x<b.left||next.x>b.right)velocity.x=0;
            if(next.y<b.top||next.y>b.bottom)velocity.y=0;
            point=next;velocity.x*=Math.exp(-dt/140);velocity.y*=Math.exp(-dt/140);place();
        }else{
            const b=bounds(),goal=point.x+28<window.innerWidth/2?b.left:b.right;
            point.x+=(goal-point.x)*(1-Math.exp(-dt/65));place();
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
        halt();suppressClick=false;prepare(event);
        drag={id:event.pointerId,x:event.clientX,y:event.clientY,start:{...point},last:{x:event.clientX,y:event.clientY,time:now()},moved:false};
        changed();
        ball.setPointerCapture(event.pointerId);event.preventDefault();event.stopPropagation();}
    function move(event){if(!drag||event.pointerId!==drag.id)return;
        if(!(event.buttons&1)){cancel();return;}
        const time=now(),elapsed=Math.max(1,time-drag.last.time),dx=event.clientX-drag.x,dy=event.clientY-drag.y;
        drag.moved ||= Math.hypot(dx,dy)>6;
        if(drag.moved){close();point={x:drag.start.x+dx,y:drag.start.y+dy};place();
            velocity={x:clamp((event.clientX-drag.last.x)/elapsed,-1.5,1.5),y:clamp((event.clientY-drag.last.y)/elapsed,-1.5,1.5)};}
        drag.last={x:event.clientX,y:event.clientY,time};event.preventDefault();event.stopPropagation();}
    function releaseCapture(id){if(ball.hasPointerCapture(id))ball.releasePointerCapture(id);}
    function up(event){if(!drag||event.pointerId!==drag.id||event.button!==0)return;
        const held=drag;drag=null;releaseCapture(held.id);suppressClick=held.moved;
        if(held.moved){if(now()-held.last.time>100)velocity={x:0,y:0};
            if(reduced()){dock();}else{lastFrame=now();coastUntil=lastFrame+650;animation=raf(frame);}}
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
    function resize(){halt();if(drag)cancel();place();if(opened)layout();}
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
        clean:()=>stopped&&!animation&&!drag&&!opened,
        // Read-only geometry for lifecycle/physics regressions.
        snapshot:()=>({point:{...point},animation:!!animation,dragging:!!drag,opened,selected})};
}
