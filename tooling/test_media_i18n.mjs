// Production media controller with DOM/network doubles. This checks localization
// and cleanup boundaries, not browser image decoding or codec support.
import test from "node:test";
import assert from "node:assert/strict";
import {readFileSync} from "node:fs";
import vm from "node:vm";
import {translator} from "./i18n_test_support.mjs";

function environment(language,geometry={}){
    const nodes=new Map(),created=[],sent=[],urls=new Set(),frames=new Map(),observers=[];
    let nextFrame=0;
    const events=()=>({listeners:new Map(),callbacks:new Map(),options:new Map(),
        addEventListener(name,fn,options){const set=this.callbacks.get(name)??new Set();set.add(fn);this.callbacks.set(name,set);this.options.set(name,options);this.listeners.set(name,e=>{for(const callback of [...set])callback(e);});},
        removeEventListener(name,fn){const set=this.callbacks.get(name);set?.delete(fn);if(!set?.size){this.listeners.delete(name);this.callbacks.delete(name);this.options.delete(name);}}});
    const element=tag=>({...events(),tag,scrollLeft:0,scrollTop:0,captured:new Set(),
        setPointerCapture(id){this.captured.add(id);},hasPointerCapture(id){return this.captured.has(id);},releasePointerCapture(id){this.captured.delete(id);},style:{},dataset:{},children:[],attributes:{},open:false,hidden:false,
        clientWidth:800,clientHeight:400,naturalWidth:32,naturalHeight:16,paused:true,
        append(...children){this.children.push(...children);},replaceChildren(){this.children=[];},
        get childElementCount(){return this.children.length;},
        setAttribute(name,value){this.attributes[name]=value;},removeAttribute(name){delete this.attributes[name];delete this[name];},
        showModal(){this.open=true;},close(){this.open=false;},focus(){document.activeElement=this;},
        contains(target){return this===target||this.children.some(child=>child.contains(target));},closest(){return this.dataset.contextAction?this:null;},
        pause(){this.paused=true;},load(){},
        getBoundingClientRect(){return {left:0,top:0,right:800,bottom:400,width:800,height:400};},
    });
    const workspace=element("workspace"),bounds={left:40,top:30,width:1280,height:900,...geometry};
    bounds.right=bounds.left+bounds.width;bounds.bottom=bounds.top+bounds.height;
    workspace.getBoundingClientRect=()=>bounds;
    const document={...events(),getElementById(id){if(!nodes.has(id))nodes.set(id,element(id));return nodes.get(id);},
        querySelector:()=>workspace,createElement(tag){const node=element(tag);created.push(node);return node;}};
    document.getElementById("chrome").getBoundingClientRect=()=>bounds;
    document.getElementById("media-hover-status").getBoundingClientRect=()=>({height:18});
    document.getElementById("media-hover").getBoundingClientRect=()=>{
        const stage=document.getElementById("media-hover-viewport").children[0];
        return {width:(Number.parseFloat(stage?.style.width)||100)+10,height:(Number.parseFloat(stage?.style.height)||0)+28};
    };
    const window={...events(),innerWidth:1000,innerHeight:700};
    const menuButtons=["copy-picture","save-picture","copy-link","open-link","settings","next"].map(action=>{const button=element("button");button.dataset.contextAction=action;return button;});
    const menu=document.getElementById("media-context-menu");menu.hidden=true;menu.append(...menuButtons);menu.querySelectorAll=()=>menuButtons;
    const install=vm.runInNewContext(readFileSync(new URL("../assets/surface-context-menu.js",import.meta.url),"utf8")+"\n"+readFileSync(new URL("../assets/gif-playback.js",import.meta.url),"utf8")+"\n"+readFileSync(new URL("../assets/media.js",import.meta.url),"utf8")+"\ninstallMedia",{
        document,window,AbortController,fetch:async()=>({blob:async()=>({size:1})}),
        URL:{createObjectURL(){const url="blob:"+urls.size;urls.add(url);return url;},revokeObjectURL:url=>urls.delete(url)},
        ResizeObserver:class{constructor(callback){this.callback=callback;observers.push(this);}observe(){}disconnect(){}},requestAnimationFrame:callback=>{frames.set(++nextFrame,callback);return nextFrame;},cancelAnimationFrame:id=>frames.delete(id),
    });
    const ui=install({tr:translator(language),send:packet=>sent.push(packet),prepare(){},changed(){},restore(){}});
    return {ui,nodes,sent,urls,created,bounds,document,window,menuButtons,resize:()=>observers.forEach(o=>o.callback()),node:document.getElementById,flush(){for(const [id,callback] of [...frames]){frames.delete(id);callback();}}};
}
const settle=()=>new Promise(resolve=>setImmediate(resolve));

test("image feedback uses the selected language, preserves URL data and releases resources",async()=>{
    for(const [language,title,ready] of [["en-US","Image preview","Image loaded · memory cache"],["zh-TW","圖片預覽","圖片已載入 · 記憶體快取"]]){
        const env=environment(language),url="https://example.com/<text>?x=1&y=2";
        env.ui.open({request:7,target:{kind:"image",url}});
        assert.equal(env.node("media-title").textContent,title);
        assert.equal(env.node("media-url").textContent,url);
        env.ui.receiveImage({request:7,mime:"image/png",data:"AA==",width:32,height:16,animated:false,cached:true});
        await settle();env.created.find(node=>node.tag==="img").onload();
        assert.equal(env.node("media-status").textContent,ready);
        assert.equal(env.sent.at(-1).success,true);
        env.ui.receiveImage({request:6}); // Stale results do not replace current text.
        assert.equal(env.node("media-status").textContent,ready);
        assert.equal(await env.ui.destroy(),true);assert.equal(env.urls.size,0);
    }
});

test("video player events retain English labels and explicit failure feedback",async()=>{
    const env=environment("en-US");env.ui.open({request:8,target:{kind:"video",url:"https://example.com/video.webm"}});
    const video=env.created.find(node=>node.tag==="video");
    assert.equal(env.node("media-title").textContent,"Video preview");
    assert.equal(video.attributes["aria-label"],"Video player");
    video.onplaying();assert.equal(env.node("media-motion").textContent,"Pause video");
    video.onpause();assert.equal(env.node("media-motion").textContent,"Play video");
    video.onwaiting();assert.equal(env.node("media-status").textContent,"Buffering video…");
    video.onerror();assert.equal(env.node("media-status").textContent,"Cannot play this video or format; retry or open in a browser");
    assert.equal(await env.ui.destroy(),true);
});

async function loadHover(env,width,height,position="top_right"){
    env.ui.hover({request:9,position,target:{kind:"image",url:"https://example.com/large.png"}});
    env.ui.receiveImage({request:9,mime:"image/png",data:"AA==",width,height,animated:false,cached:false});
    await settle();
    const image=env.created.find(node=>node.tag==="img");image.naturalWidth=width;image.naturalHeight=height;image.onload();
    return env.node("media-hover-viewport").children[0];
}

test("large hover images use more space while fitting the whole card in the workspace",async()=>{
    for(const geometry of [{},{width:360,height:240},{width:800,height:300}]){
        const env=environment("zh-TW",geometry),stage=await loadHover(env,1200,800);
        const width=Number.parseFloat(stage.style.width),height=Number.parseFloat(stage.style.height);
        assert.ok(Math.abs(width/height-1.5)<1e-9);
        if(!geometry.width)assert.ok(width>=500,"large previews should be visibly wider than the old 360px cap");
        assert.equal(Number.parseFloat(env.node("media-hover").style.top),env.bounds.top+12,"the full-window floating chrome must not push top-right previews to the bottom");
        env.ui.locate({x:env.bounds.right-1,y:env.bounds.bottom-1},"follow_cursor");env.flush();
        const hover=env.node("media-hover"),box=hover.getBoundingClientRect(),x=Number.parseFloat(hover.style.left),y=Number.parseFloat(hover.style.top);
        assert.ok(x>=env.bounds.left&&x+box.width<=env.bounds.right);
        assert.ok(y>=env.bounds.top&&y+box.height<=env.bounds.bottom);
        assert.equal(await env.ui.destroy(),true);
    }
});

test("portrait hover accounts for its status and promotion preserves image resources and original-size control",async()=>{
    const env=environment("en-US"),stage=await loadHover(env,800,1600);
    assert.ok(env.node("media-hover").getBoundingClientRect().height<=env.bounds.height*.6);
    assert.equal(Number.parseFloat(stage.style.width)/Number.parseFloat(stage.style.height),.5);
    const resource=[...env.urls][0];
    env.node("media-viewport").clientWidth=1500;env.node("media-viewport").clientHeight=850;
    env.ui.open({request:9,target:{kind:"image",url:"https://example.com/large.png"}});
    assert.equal(env.urls.size,1);assert.ok(env.urls.has(resource));
    assert.equal(stage.style.height,"850px");assert.equal(stage.style.width,"425px");
    env.node("media-original").listeners.get("click")();
    assert.equal(stage.style.width,"800px");assert.equal(stage.style.height,"1600px");
    assert.equal(await env.ui.destroy(),true);
});

test('explicit GIF previews autoplay, hover stays still, and pause/focus lifecycle releases the native source',async()=>{
 const env=environment('en-US');
 env.ui.hover({request:9,position:'top_right',target:{kind:'image',url:'https://example.com/motion.gif'}});
 env.ui.receiveImage({request:9,mime:'image/gif',data:'AA==',width:32,height:16,animated:true,cached:false});
 await settle();const image=env.created.find(n=>n.tag==='img');
 const context={clearRect(){},drawImage(){}};env.created.filter(n=>n.tag==='canvas').forEach(c=>c.getContext=()=>context);
 image.onload();assert.equal(image.hidden,true);assert.equal(image.src,undefined);
 env.ui.open({request:9,target:{kind:'image',url:'https://example.com/motion.gif'}});
 assert.equal(image.hidden,false);assert.ok(image.src.startsWith('blob:'));
 assert.equal(env.node('media-motion').textContent,'Stop and return to first frame');
 env.node('media-motion').listeners.get('click')({isTrusted:true});assert.equal(image.hidden,true);assert.equal(image.src,undefined);
 env.node('media-motion').listeners.get('click')({isTrusted:true});assert.equal(image.hidden,false);
 env.document.visibilityState='hidden';env.document.listeners.get('visibilitychange')();assert.equal(image.src,undefined);
 env.document.visibilityState='visible';env.document.listeners.get('visibilitychange')();assert.equal(image.hidden,false);
 assert.equal(await env.ui.destroy(),true);assert.equal(env.urls.size,0);assert.equal(env.document.listeners.size,0);
});

const pointer=target=>({target,isTrusted:true,clientX:500,clientY:350,preventDefault(){this.prevented=true;},stopPropagation(){this.stopped=true;},stopImmediatePropagation(){this.stopped=true;}});
async function inspectImage(env,request=20){
 env.ui.open({request,target:{kind:'image',url:'https://example.com/source.png'}});
 env.ui.receiveImage({request,mime:'image/png',data:'AA==',width:32,height:16,animated:false,cached:false});
 await settle();const image=env.created.filter(node=>node.tag==='img').at(-1);image.onload();return image;
}
test('inspected images share the four-item contextual menu and send only the current request',async()=>{
 const env=environment('zh-TW'),image=await inspectImage(env),menu=env.node('media-context-menu');
 const right=()=>env.node('media-viewport').listeners.get('contextmenu')(pointer(image));
 for(const [action,expected] of [['copy-picture','copy_image'],['save-picture','save_image'],['copy-link','copy_url'],['open-link','external']]){
  right();assert.equal(menu.hidden,false);
  assert.deepEqual(env.menuButtons.filter(button=>!button.hidden).map(button=>button.dataset.contextAction),['copy-picture','save-picture','copy-link','open-link']);
  menu.listeners.get('click')(pointer(env.menuButtons.find(button=>button.dataset.contextAction===action)));
  assert.deepEqual(JSON.parse(JSON.stringify(env.sent.at(-1))),{kind:'media',request:20,action:expected,trusted:true});assert.equal(menu.hidden,true);
 }
 right();const escape={...pointer(menu),key:'Escape'};env.document.listeners.get('keydown')(escape);
 assert.equal(menu.hidden,true);assert.equal(env.node('media-dialog').open,true);assert.equal(escape.stopped,true);
 right();env.ui.open({request:21,target:{kind:'image',url:'https://example.com/next.png'}});assert.equal(menu.hidden,true);
 const count=env.sent.length;menu.listeners.get('click')(pointer(env.menuButtons[0]));assert.equal(env.sent.length,count);
 assert.equal(env.ui.notice('copied'),true);assert.equal(env.node('media-status').textContent,'copied');
 assert.equal(await env.ui.destroy(),true);assert.equal(env.document.listeners.size,0);assert.equal(env.window.listeners.size,0);assert.equal(menu.listeners.size,0);
});
test('image menus reject untrusted, background, hover, failed and video targets',async()=>{
 const env=environment('en-US'),image=await inspectImage(env),menu=env.node('media-context-menu'),fire=event=>env.node('media-viewport').listeners.get('contextmenu')(event);
 fire({...pointer(image),isTrusted:false});assert.equal(menu.hidden,true);
 fire(pointer(env.node('media-viewport')));assert.equal(menu.hidden,true);
 env.ui.fail({request:20,message:'decode failed'});fire(pointer(image));assert.equal(menu.hidden,true);
 env.ui.open({request:22,target:{kind:'video',url:'https://example.com/video.webm'}});fire(pointer(env.created.find(node=>node.tag==='video')));assert.equal(menu.hidden,true);
 env.ui.hover({request:23,position:'top_right',target:{kind:'image',url:'https://example.com/hover.png'}});fire(pointer(image));assert.equal(menu.hidden,true);
 assert.equal(await env.ui.destroy(),true);assert.equal(env.ui.notice('late'),false);
});

// Independent geometry: center an image when smaller, scroll its top/left when
// larger. Native browser clamping is deliberately modeled, not an unbounded div.
async function inspectLarge(env,{mime='image/png',animated=false}={}){
 const view=env.node('media-viewport');view.clientWidth=400;view.clientHeight=300;
 view.getBoundingClientRect=()=>({left:100,top:50,right:500,bottom:350,width:400,height:300});
 env.ui.open({request:31,target:{kind:'image',url:'https://example.com/large.png'}});
 env.ui.receiveImage({request:31,mime,data:'AA==',width:1000,height:800,animated,cached:false});
 await settle();const stage=view.children[0],image=stage.children[0];image.naturalWidth=1000;image.naturalHeight=800;
 stage.children.filter(n=>n.tag==='canvas').forEach(n=>n.getContext=()=>({clearRect(){},drawImage(){}}));
 for(const [axis,size,client] of [['scrollLeft','width','clientWidth'],['scrollTop','height','clientHeight']]){
  let offset=0;Object.defineProperty(view,axis,{configurable:true,get:()=>offset,set:value=>{offset=Math.max(0,Math.min(Math.max(0,Number.parseFloat(stage.style[size])-view[client]),value));}});
 }
 stage.getBoundingClientRect=()=>({left:100+Math.max(0,(400-Number.parseFloat(stage.style.width))/2)-view.scrollLeft,
  top:50+Math.max(0,(300-Number.parseFloat(stage.style.height))/2)-view.scrollTop,
  width:Number.parseFloat(stage.style.width),height:Number.parseFloat(stage.style.height)});
 image.onload();
 const event=(extra={})=>({...pointer(image),pointerId:4,isPrimary:true,button:0,buttons:1,clientX:320,clientY:230,...extra});
 const fire=(name,extra={})=>{const e=event(extra);view.listeners.get(name)?.(e);return e;};
 const sourcePoint=(x,y)=>{const b=stage.getBoundingClientRect();return [(x-b.left)/b.width,(y-b.top)/b.height];};
 return {view,stage,image,event,fire,sourcePoint};
}
test('wheel zoom anchors the image under the pointer, preserves aspect and normalizes pixel/line/page deltas',async()=>{
 for(const [deltaMode,deltaY] of [[0,-100],[1,-6.25],[2,-1/3]]){
  const env=environment('en-US'),f=await inspectLarge(env);env.node('media-original').listeners.get('click')();
  f.view.scrollLeft=250;f.view.scrollTop=200;const before=f.sourcePoint(320,230),requests=env.sent.length;
  const e=f.fire('wheel',{deltaMode,deltaY});
  assert.equal(e.prevented,true);assert.equal(e.stopped,true);assert.equal(f.view.options.get('wheel').passive,false);
  assert.ok(Math.abs(Number(env.node('media-dialog').dataset.scale)-Math.exp(.25))<1e-10);
  assert.ok(Math.abs(Number.parseFloat(f.stage.style.width)/Number.parseFloat(f.stage.style.height)-1.25)<1e-10);
  f.sourcePoint(320,230).forEach((v,i)=>assert.ok(Math.abs(v-before[i])<1e-10));
  f.fire('wheel',{deltaMode,deltaY:-deltaY});
  assert.ok(Math.abs(Number(env.node('media-dialog').dataset.scale)-1)<1e-10);
  assert.equal(env.sent.length,requests);assert.equal(env.urls.size,1);
  assert.equal(await env.ui.destroy(),true);
 }
});
test('wheel zoom clamps finite input, leaves horizontal scrolling alone and resets cleanly to Fit',async()=>{
 const env=environment('en-US'),f=await inspectLarge(env);
 for(const e of [{deltaY:0,deltaX:50},{deltaY:NaN},{deltaY:Infinity},{deltaY:-100,isTrusted:false}])assert.equal(f.fire('wheel',e).prevented,undefined);
 for(let i=0;i<80;i++)f.fire('wheel',{deltaY:-1e6});assert.equal(env.node('media-dialog').dataset.scale,'8');
 for(let i=0;i<80;i++)f.fire('wheel',{deltaY:1e6});assert.equal(env.node('media-dialog').dataset.scale,'0.01');
 env.node('media-fit').listeners.get('click')();assert.equal(env.node('media-dialog').dataset.scale,'0.375');
 assert.equal(f.view.scrollLeft,0);assert.equal(f.view.scrollTop,0);assert.equal(f.view.dataset.pannable,'false');
 assert.equal(await env.ui.destroy(),true);
});
test('left drag pans both axes with capture, while fit images and modified/right clicks keep their owners',async()=>{
 const env=environment('en-US'),f=await inspectLarge(env);
 assert.equal(f.fire('pointerdown').prevented,undefined);assert.equal(f.view.captured.size,0);
 env.node('media-original').listeners.get('click')();f.view.scrollLeft=200;f.view.scrollTop=150;
 for(const extra of [{button:2},{button:1},{shiftKey:true},{ctrlKey:true},{altKey:true},{metaKey:true},{isPrimary:false},{isTrusted:false},{clientX:501}]){
  assert.equal(f.fire('pointerdown',extra).prevented,undefined);assert.equal(f.view.captured.size,0);
 }
 assert.equal(f.fire('pointerdown').prevented,true);assert.equal(f.view.dataset.panning,'true');assert.ok(f.view.captured.has(4));
 f.fire('pointermove',{clientX:270,clientY:210,pointerId:99});assert.equal(f.view.scrollLeft,200);
 f.fire('pointermove',{clientX:270,clientY:210});assert.equal(f.view.scrollLeft,250);assert.equal(f.view.scrollTop,170);
 // Captured drag can leave the preview; scroll stays within the real extent.
 f.fire('pointermove',{clientX:-5000,clientY:-5000});assert.equal(f.view.scrollLeft,600);assert.equal(f.view.scrollTop,500);
 f.fire('pointerup');assert.equal(f.view.captured.size,0);assert.equal(f.view.dataset.panning,undefined);
 const offsets=[f.view.scrollLeft,f.view.scrollTop];f.fire('pointermove',{clientX:100,clientY:100});assert.deepEqual([f.view.scrollLeft,f.view.scrollTop],offsets);
 assert.equal(await env.ui.destroy(),true);
});
test('cancel, focus loss, context menu, zoom, switching and close revoke drag without stale movement',async()=>{
 for(const cancel of ['pointercancel','lostpointercapture','blur','hidden','contextmenu','zoom','switch','close','resize','buttons-up']){
  const env=environment('en-US'),f=await inspectLarge(env);env.node('media-original').listeners.get('click')();f.fire('pointerdown');
  if(['pointercancel','lostpointercapture'].includes(cancel))f.fire(cancel);
  else if(cancel==='blur')env.window.listeners.get('blur')();
  else if(cancel==='hidden'){env.document.visibilityState='hidden';env.document.listeners.get('visibilitychange')();}
  else if(cancel==='contextmenu')f.fire('contextmenu');
  else if(cancel==='zoom')f.fire('wheel',{deltaY:-100});
  else if(cancel==='switch')env.ui.open({request:99,target:{kind:'image',url:'https://example.com/new.png'}});
  else if(cancel==='close')env.node('media-close').listeners.get('click')(f.event());
  else if(cancel==='resize')env.resize();
  else f.fire('pointermove',{buttons:0});
  assert.equal(f.view.captured.size,0,cancel);assert.equal(f.view.dataset.panning,undefined,cancel);
  const offset=[f.view.scrollLeft,f.view.scrollTop];f.fire('pointermove',{clientX:200,clientY:150});assert.deepEqual([f.view.scrollLeft,f.view.scrollTop],offset,cancel);
  assert.equal(await env.ui.destroy(),true);assert.equal(f.view.listeners.size,0);assert.equal(env.window.listeners.size,0);assert.equal(env.document.listeners.size,0);
 }
});
test('GIF zoom and pan retain the decoder and resources, and hidden/video/loading/closed previews reject gestures',async()=>{
 const env=environment('en-US'),f=await inspectLarge(env,{mime:'image/gif',animated:true});
 const blob=f.image.src,packets=env.sent.length;env.node('media-original').listeners.get('click')();
 f.fire('wheel',{deltaY:-100});f.fire('pointerdown');f.fire('pointermove',{clientX:280});f.fire('pointerup');
 assert.equal(f.image.src,blob);assert.equal(f.image.hidden,false);assert.equal(env.sent.length,packets);assert.equal(env.urls.size,1);
 // Opening the image menu suspends wheel and pan until dismissed.
 f.fire('contextmenu');assert.equal(f.fire('wheel',{deltaY:-100}).prevented,undefined);assert.equal(f.fire('pointerdown').prevented,undefined);
 env.ui.hover({request:32,position:'top_right',target:{kind:'image',url:'https://example.com/hover.png'}});
 assert.equal(f.fire('wheel',{deltaY:-100}).prevented,undefined);
 env.ui.open({request:33,target:{kind:'video',url:'https://example.com/video.webm'}});
 assert.equal(f.fire('wheel',{deltaY:-100}).prevented,undefined);assert.equal(f.fire('pointerdown').prevented,undefined);
 env.ui.open({request:34,target:{kind:'image',url:'https://example.com/loading.png'}});
 assert.equal(f.fire('wheel',{deltaY:-100}).prevented,undefined);
 assert.equal(await env.ui.destroy(),true);assert.equal(f.fire('wheel',{deltaY:-100}).prevented,undefined);
});
