import {readFileSync} from 'node:fs';
import vm from 'node:vm';
import test from 'node:test';
import assert from 'node:assert/strict';

const create=vm.runInNewContext(readFileSync(new URL('../assets/surface-tools.js',import.meta.url),'utf8')+'\ncreateFloatingTools');
function fixture({width=900,height=700,reduced=false}={}){
    let clock=0,next=0,prepared=0,restored=0;
    const frames=new Map(),classes=new Set();
    const element=()=>({hidden:true,attrs:{},dataset:{},style:{setProperty(k,v){this[k]=v;}},
        setAttribute(k,v){this.attrs[k]=v;},focus(){document.activeElement=this;},
        contains(target){return target===this;}});
    const root=element(),ball=element(),fan=element(),panel=element(),back=element();
    root.classList={add:v=>classes.add(v),remove:v=>classes.delete(v)};
    const petals=['navigate','display','capture','input','session','filters'].map(group=>{const p=element();p.dataset.toolsTarget=group;return p;});
    const groups=petals.map(p=>{const group=element();group.dataset.toolsGroup=p.dataset.toolsTarget;return group;});
    const document={activeElement:null,hidden:false,querySelector:()=>null},window={innerWidth:width,innerHeight:height};
    const captures=new Set();ball.setPointerCapture=id=>captures.add(id);ball.hasPointerCapture=id=>captures.has(id);ball.releasePointerCapture=id=>captures.delete(id);
    fan.querySelectorAll=()=>petals;fan.contains=target=>petals.includes(target);
    panel.querySelectorAll=()=>groups;panel.querySelector=selector=>selector==='#tools-back'?back:null;
    root.contains=target=>[root,ball,fan,panel,back,...petals,...groups].includes(target);
    const menu=create({root,ball,fan,panel,window,document,prepare:()=>prepared++,restore:()=>restored++,
        raf:callback=>{frames.set(++next,callback);return next;},caf:id=>frames.delete(id),now:()=>clock,reduced:()=>reduced});
    function fire(target,type,values={}){const e={pointerId:1,button:0,buttons:1,isPrimary:true,clientX:820,clientY:455,detail:1,preventDefault(){this.prevented=true;},stopPropagation(){this.stopped=true;},...values};
        for(const [node,event,callback] of menu.bindings)if(node===target&&event===type)callback({...e,currentTarget:target});return e;}
    function advance(ms=16){clock+=ms;const queued=[...frames.values()];frames.clear();queued.forEach(fn=>fn(clock));}
    return {menu,fire,advance,root,ball,fan,panel,back,petals,groups,window,document,captures,frames,counts:()=>({prepared,restored})};
}
const overlaps=(a,b)=>a.x<b.x+b.w&&b.x<a.x+a.w&&a.y<b.y+b.h&&b.y<a.y+a.h;
function openAt(f,x,y){
    const start=f.menu.snapshot().point;
    f.fire(f.ball,'pointerdown',{clientX:0,clientY:0});f.advance(40);
    f.fire(f.ball,'pointerup',{clientX:x-start.x,clientY:y-start.y,buttons:0});f.fire(f.ball,'click');
    f.fire(f.ball,'pointerdown');f.fire(f.ball,'pointerup');f.fire(f.ball,'click');
}
test('all six visible fan hit areas have a real gap rather than overlapping corner buttons',()=>{
    const f=fixture();f.fire(f.ball,'click');
    const rects=f.petals.map(p=>({x:Number.parseFloat(p.style['--petal-x']),y:Number.parseFloat(p.style['--petal-y']),w:62,h:62}));
    for(let i=0;i<rects.length;i++)for(let j=i+1;j<rects.length;j++){
        const padded={...rects[i],x:rects[i].x-4,y:rects[i].y-4,w:70,h:70};
        assert.equal(overlaps(padded,rects[j]),false,`categories ${i} and ${j} need a gap`);
    }
});
test('category panels replace the fan and return restores the selected category focus across resizing',()=>{
    const f=fixture();f.fire(f.ball,'click');f.fire(f.petals[2],'click');
    assert.equal(f.fan.hidden,true);assert.equal(f.panel.hidden,false);
    f.ball.focus();f.fire(f.document,'keydown',{key:'ArrowDown'});assert.equal(f.document.activeElement,f.ball);
    f.window.innerWidth=360;f.window.innerHeight=300;f.fire(f.window,'resize');
    assert.equal(f.fan.hidden,true);assert.equal(f.panel.hidden,false);
    f.fire(f.back,'click');assert.equal(f.panel.hidden,true);assert.equal(f.fan.hidden,false);
    f.fire(f.panel,'click',{target:{closest:()=>f.back}}); // real click also bubbles through the old panel
    assert.equal(f.document.activeElement,f.petals[2]);assert.ok(f.groups.every(g=>g.hidden));
    f.fire(f.petals[1],'click');f.window.innerWidth=1040;f.window.innerHeight=740;f.fire(f.window,'resize');
    assert.equal(f.fan.hidden,true);assert.equal(f.panel.hidden,false);
    f.fire(f.back,'click');assert.equal(f.fan.hidden,false);assert.equal(f.panel.hidden,true);
});
test('visible fan/grid and reserved panel bounds never cover the stationary trigger or leave the viewport',()=>{
    for(const [width,height] of [[320,240],[360,300],[500,420],[900,700],[1920,1080]]){
        for(const x of [16,width/2-28,width-72])for(const y of [56,height/2,height-72]){
            const f=fixture({width,height});openAt(f,x,y);
            const point=f.menu.snapshot().point,ball={...point,w:56,h:56};
            function check(rect){
                assert.ok(rect.x>=16&&rect.y>=48&&rect.x+rect.w<=width-16+.001&&rect.y+rect.h<=height-16+.001,JSON.stringify({width,height,point,rect}));
                assert.equal(overlaps(ball,rect),false,JSON.stringify({ball,rect}));
            }
            if(f.root.dataset.toolsLayout==='fan')for(const p of f.petals)check({x:point.x+Number.parseFloat(p.style['--petal-x']),y:point.y+Number.parseFloat(p.style['--petal-y']),w:62,h:62});
            else check({x:Number.parseFloat(f.fan.style['--grid-x']),y:Number.parseFloat(f.fan.style['--grid-y']),w:Number.parseFloat(f.fan.style['--grid-width']),h:Number.parseFloat(f.fan.style['--grid-height'])});
            f.fire(f.petals[0],'click');assert.equal(f.fan.hidden,true);
            check({x:Number.parseFloat(f.panel.style.left),y:Number.parseFloat(f.panel.style.top),w:Number.parseFloat(f.panel.style.width),h:Number.parseFloat(f.panel.style.maxHeight)});
            assert.deepEqual(f.menu.snapshot().point,point);
            f.menu.destroy();assert.equal(f.menu.clean(),true);
        }
    }
});
test('click toggles a six-way fan, each category exposes its original controls, hover does nothing',()=>{
    const f=fixture();f.fire(f.root,'pointerenter');assert.equal(f.menu.open(),false);
    f.fire(f.ball,'click');assert.equal(f.fan.hidden,false);assert.equal(f.ball.attrs['aria-expanded'],'true');
    f.fire(f.petals[2],'click');assert.equal(f.panel.hidden,false);
    assert.deepEqual(f.groups.map(g=>g.hidden),[true,true,false,true,true,true]);
    f.fire(f.ball,'click');assert.equal(f.menu.open(),false);assert.equal(f.panel.hidden,true);
});
test('drag coasts then docks within bounds without opening tools or generating an action',()=>{
    const f=fixture();f.fire(f.ball,'pointerdown');f.advance(16);
    f.fire(f.ball,'pointermove',{clientX:610,clientY:310});f.fire(f.ball,'pointerup',{clientX:610,clientY:310});
    assert.equal(f.captures.size,0);assert.equal(f.menu.snapshot().animation,true);
    f.fire(f.ball,'click');assert.equal(f.menu.open(),false); // follow-up click belongs to this drag
    const released=f.menu.snapshot().point;f.advance();assert.notEqual(f.menu.snapshot().point.x,released.x);
    for(let i=0;i<140;i++){f.advance();const p=f.menu.snapshot().point;assert.ok(p.x>=16&&p.x<=828&&p.y>=56&&p.y<=628);}
    assert.equal(f.frames.size,0);assert.ok([16,828].includes(f.menu.snapshot().point.x));
    f.fire(f.ball,'pointerdown');f.fire(f.ball,'pointerup');f.fire(f.ball,'click');assert.equal(f.menu.open(),true);
});
test('zero-detail drag click cannot cancel momentum; subsequent physical taps open and close in place',()=>{
    const f=fixture();f.fire(f.ball,'pointerdown');f.advance(40);
    f.fire(f.ball,'pointerup',{clientX:640,clientY:80,buttons:0});
    f.fire(f.ball,'click',{detail:0});
    assert.equal(f.menu.open(),false);assert.equal(f.menu.snapshot().animation,true);
    const released=f.menu.snapshot().point;f.advance();assert.ok(f.menu.snapshot().point.x<released.x);
    for(const expected of [true,false]){
        const before=f.menu.snapshot().point;
        const position={clientX:before.x+28,clientY:before.y+28};
        f.fire(f.ball,'pointerdown',position);f.fire(f.ball,'pointerup',{...position,buttons:0});f.fire(f.ball,'click');
        assert.equal(f.menu.open(),expected);assert.deepEqual(f.menu.snapshot().point,before);
    }
});
test('opening near either vertical edge moves the fan instead of the ball',()=>{
    for(const y of [56,628]){
        const f=fixture({reduced:true});f.fire(f.ball,'pointerdown');f.advance(40);
        f.fire(f.ball,'pointerup',{clientY:y,buttons:0});f.fire(f.ball,'click');
        const before=f.menu.snapshot().point;
        f.fire(f.ball,'pointerdown',{clientY:y});f.fire(f.ball,'pointerup',{clientY:y,buttons:0});f.fire(f.ball,'click');
        assert.equal(f.menu.open(),true);assert.deepEqual(f.menu.snapshot().point,before);
        for(const p of f.petals){
            const top=before.y+Number.parseFloat(p.style['--petal-y']),dx=Number.parseFloat(p.style['--petal-x']);
            assert.ok(top>=48&&top+62<=684);assert.ok(dx>=72||dx<=-72,'petals cannot cover the close trigger');
        }
        f.fire(f.ball,'click');assert.deepEqual(f.menu.snapshot().point,before);assert.equal(f.menu.open(),false);
    }
});
test('keyboard activation and later accessibility clicks are not swallowed by a previous drag',()=>{
    for(const keyboard of [true,false]){
        const f=fixture();f.fire(f.ball,'pointerdown');f.advance(40);f.fire(f.ball,'pointerup',{clientX:650,buttons:0});
        if(keyboard)f.fire(f.document,'keydown',{key:'Enter',target:f.ball});else f.advance(400);
        f.fire(f.ball,'click',{detail:0});assert.equal(f.menu.open(),true);
    }
});
test('foreign pointers, missing physical button, cancellation and shutdown cannot leave capture or animation',()=>{
    for(const kind of ['pointercancel','blur','visibilitychange','missing-button','destroy']){
        const f=fixture();f.fire(f.ball,'pointerdown');f.advance();
        f.fire(f.ball,'pointermove',{pointerId:2,clientX:100});assert.equal(f.menu.snapshot().point.x,820);
        f.fire(f.ball,'pointermove',{clientX:600});
        if(kind==='blur')f.fire(f.window,kind);
        else if(kind==='visibilitychange'){f.document.hidden=true;f.fire(f.document,kind);}
        else if(kind==='missing-button')f.fire(f.ball,'pointermove',{buttons:0});
        else if(kind==='destroy')f.menu.destroy();else f.fire(f.ball,kind);
        assert.equal(f.captures.size,0);assert.equal(f.frames.size,0);assert.equal(f.menu.snapshot().dragging,false);
        f.menu.destroy();assert.equal(f.menu.clean(),true);f.fire(f.ball,'click');assert.equal(f.menu.open(),false);
    }
});
test('blur during momentum cancels the frame; reduced motion docks immediately',()=>{
    for(const reduced of [true,false]){
        const f=fixture({reduced});f.fire(f.ball,'pointerdown');f.advance();f.fire(f.ball,'pointermove',{clientX:400});f.fire(f.ball,'pointerup',{clientX:400});
        assert.equal(f.menu.snapshot().animation,!reduced);f.fire(f.window,'blur');assert.equal(f.frames.size,0);
    }
});
test('keyboard navigation and Escape stay within local chrome; resize keeps tools reachable',()=>{
    const f=fixture();f.fire(f.ball,'click',{detail:0});f.ball.focus();
    f.fire(f.document,'keydown',{key:'ArrowUp'});assert.equal(f.document.activeElement,f.petals[5]);
    f.fire(f.document,'keydown',{key:'ArrowDown'});assert.equal(f.document.activeElement,f.petals[0]);
    f.fire(f.document,'keydown',{key:'Escape'});assert.equal(f.document.activeElement,f.ball);assert.equal(f.menu.open(),false);
    f.window.innerWidth=360;f.window.innerHeight=300;f.fire(f.window,'resize');f.fire(f.ball,'click');
    assert.equal(f.root.dataset.toolsLayout,'compact');f.fire(f.petals[1],'click');assert.equal(f.fan.hidden,true);assert.equal(f.panel.hidden,false);
    const p=f.menu.snapshot().point;assert.ok(p.x>=16&&p.x<=288&&p.y>=56&&p.y<=228);
});

test('an outside pointer closes tools and is consumed before the terminal path',()=>{
    const f=fixture();f.fire(f.ball,'click');
    let prevented=false,stopped=false;
    f.fire(f.document,'pointerdown',{target:{},preventDefault(){prevented=true;},stopPropagation(){stopped=true;}});
    assert.equal(f.menu.open(),false);assert.equal(prevented,true);assert.equal(stopped,true);
});
test('actions keep keyboard focus in chrome, while opening tools never requests selection removal',()=>{
    const f=fixture();f.fire(f.ball,'click');f.fire(f.petals[0],'click');const button={focus(){f.document.activeElement=this;}};
    f.fire(f.panel,'click',{target:{closest:()=>button}});assert.equal(f.document.activeElement,button);
    assert.equal(f.counts().prepared,1); // one cancellation hook, no second feature action
});
test('returning to terminal editing closes floating tools before restoring keyboard focus',()=>{
    const source=readFileSync(new URL('../assets/surface.js',import.meta.url),'utf8');
    const body=source.match(/function restoreEditingFocus\(\)\{([^\n]+)\}/)[1];
    const calls=[];vm.runInNewContext(body,{tools:{close:()=>calls.push('close')},keyboard:{blur:()=>calls.push('blur')},focus:()=>calls.push('focus')});
    assert.deepEqual(calls,['close','blur','focus']);
});
test('recent drag momentum survives a tiny final movement rather than stopping at release',()=>{
    const f=fixture();f.fire(f.ball,'pointerdown');f.advance(16);
    f.fire(f.ball,'pointermove',{clientX:600,clientY:350});f.advance(16);
    f.fire(f.ball,'pointermove',{clientX:599,clientY:350});
    f.fire(f.ball,'pointerup',{clientX:599,clientY:350,buttons:0});
    const released=f.menu.snapshot().point;f.advance(16);
    assert.ok(f.menu.snapshot().point.x<released.x-10,'a flick must keep moving despite a small final sample');
    assert.equal(f.menu.observations().drag_releases,1);assert.ok(f.menu.observations().coast_frames>0);
    assert.ok(f.menu.observations().max_coast_distance>10);assert.equal(f.menu.observations().last_release.reduced,false);
});
test('coalesced release coordinates contribute to the drag endpoint and launch velocity',()=>{
    const f=fixture();f.fire(f.ball,'pointerdown');f.advance(40);
    f.fire(f.ball,'pointerup',{clientX:700,clientY:400,buttons:0});
    assert.equal(f.menu.snapshot().point.x,700);assert.equal(f.menu.snapshot().point.y,400);
    assert.equal(f.menu.snapshot().animation,true);f.advance();
    assert.ok(f.menu.snapshot().point.x<700&&f.menu.snapshot().point.y<400);
});
test('pausing before release gives only a gentle continuation while fresh reversal changes direction',()=>{
    const stopped=fixture();stopped.fire(stopped.ball,'pointerdown');stopped.advance(16);
    stopped.fire(stopped.ball,'pointermove',{clientX:600,clientY:350});stopped.advance(160);
    stopped.fire(stopped.ball,'pointerup',{clientX:600,clientY:350,buttons:0});stopped.advance();
    for(let i=0;i<10;i++)stopped.advance();
    const continuation=350-stopped.menu.snapshot().point.y;
    assert.ok(continuation>=4&&continuation<=24,'a pause keeps a small settle, never replays the old fast fling');
    const reversed=fixture();reversed.fire(reversed.ball,'pointerdown');reversed.advance(16);
    reversed.fire(reversed.ball,'pointermove',{clientX:500,clientY:300});reversed.advance(130);
    reversed.fire(reversed.ball,'pointermove',{clientX:550,clientY:330});reversed.advance(16);
    reversed.fire(reversed.ball,'pointerup',{clientX:570,clientY:340,buttons:0});reversed.advance();
    assert.ok(reversed.menu.snapshot().point.x>570&&reversed.menu.snapshot().point.y>340);
});
test('slow sparse drags get a small continuation on every release, independently of edge docking',()=>{
    const f=fixture();
    for(let attempt=0;attempt<3;attempt++){
        f.fire(f.ball,'pointerdown',{clientX:0,clientY:0});f.advance(600);
        f.fire(f.ball,'pointermove',{clientX:0,clientY:-8});f.advance(600);
        f.fire(f.ball,'pointerup',{clientX:0,clientY:-8,buttons:0});f.fire(f.ball,'click',{detail:0});
        const released=f.menu.snapshot().point;
        for(let i=0;i<10;i++)f.advance();
        const distance=released.y-f.menu.snapshot().point.y;
        assert.ok(distance>=8&&distance<=22,`slow drag ${attempt} needs a gentle vertical coast, got ${distance}`);
        assert.ok(f.menu.observations().coast_frames>0);assert.equal(f.menu.open(),false);
        assert.ok(f.frames.size<=1);
    }
    assert.equal(f.menu.observations().drag_releases,3);f.menu.destroy();assert.equal(f.menu.clean(),true);
});
test('the minimum glide follows the last actual movement, stays in bounds and does not turn taps into drags',()=>{
    const tap=fixture();tap.fire(tap.ball,'pointerdown');tap.advance(600);tap.fire(tap.ball,'pointerup');
    assert.equal(tap.menu.snapshot().animation,false);tap.fire(tap.ball,'click');assert.equal(tap.menu.open(),true);
    const f=fixture();f.fire(f.ball,'pointerdown');f.advance(600);
    f.fire(f.ball,'pointermove',{clientX:820,clientY:425});f.advance(600);
    f.fire(f.ball,'pointermove',{clientX:820,clientY:435});f.advance(600);
    f.fire(f.ball,'pointerup',{clientX:820,clientY:435,buttons:0});
    f.advance();assert.ok(f.menu.snapshot().point.y>435&&f.menu.snapshot().point.y<437,'gentle release must use the final downward direction');
    assert.ok(f.menu.observations().coast_frames>0);
    for(let i=0;i<160;i++){f.advance();const p=f.menu.snapshot().point;assert.ok(p.x>=16&&p.x<=828&&p.y>=56&&p.y<=628);}
    assert.equal(f.frames.size,0);
    const edge=fixture();edge.fire(edge.ball,'pointerdown');edge.advance(600);
    edge.fire(edge.ball,'pointerup',{clientX:1100,clientY:1000,buttons:0});
    for(let i=0;i<160;i++){edge.advance();const p=edge.menu.snapshot().point;assert.ok(p.x>=16&&p.x<=828&&p.y>=56&&p.y<=628);}
    assert.equal(edge.frames.size,0);
});
test('resizing keeps the closed floating button docked outside the content centre',()=>{
    const f=fixture();f.window.innerWidth=1800;f.fire(f.window,'resize');
    assert.equal(f.menu.snapshot().point.x,1728);
    assert.equal(f.menu.snapshot().animation,false);
});
test('probe observations retain release evidence after cleanup without exposing mutable state',()=>{
    const f=fixture({reduced:true});f.fire(f.ball,'pointerdown');f.advance(16);
    f.fire(f.ball,'pointerup',{clientX:700,clientY:400,buttons:0});
    const observed=f.menu.observations();assert.equal(observed.last_release.reduced,true);assert.equal(observed.coast_frames,0);
    observed.last_release.x=-100;f.menu.destroy();assert.equal(f.menu.observations().last_release.x,700);
    assert.equal(f.menu.observations().drag_releases,1);assert.equal(f.menu.clean(),true);
});
