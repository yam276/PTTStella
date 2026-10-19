// Production media controller with DOM/network doubles. This checks localization
// and cleanup boundaries, not browser image decoding or codec support.
import test from "node:test";
import assert from "node:assert/strict";
import {readFileSync} from "node:fs";
import vm from "node:vm";
import {translator} from "./i18n_test_support.mjs";

function environment(language,geometry={}){
    const nodes=new Map(),created=[],sent=[],urls=new Set(),frames=new Map();
    let nextFrame=0;
    const element=tag=>({tag,style:{},dataset:{},children:[],attributes:{},open:false,hidden:false,
        clientWidth:800,clientHeight:400,naturalWidth:32,naturalHeight:16,paused:true,
        append(...children){this.children.push(...children);},replaceChildren(){this.children=[];},
        get childElementCount(){return this.children.length;},
        setAttribute(name,value){this.attributes[name]=value;},removeAttribute(name){delete this.attributes[name];},
        showModal(){this.open=true;},close(){this.open=false;},focus(){},
        listeners:new Map(),addEventListener(name,callback){this.listeners.set(name,callback);},removeEventListener(name){this.listeners.delete(name);},pause(){this.paused=true;},load(){},
        getBoundingClientRect(){return {left:0,top:0,right:800,bottom:400,width:800,height:400};},
    });
    const workspace=element("workspace"),bounds={left:40,top:30,width:1280,height:900,...geometry};
    bounds.right=bounds.left+bounds.width;bounds.bottom=bounds.top+bounds.height;
    workspace.getBoundingClientRect=()=>bounds;
    const document={getElementById(id){if(!nodes.has(id))nodes.set(id,element(id));return nodes.get(id);},
        querySelector:()=>workspace,createElement(tag){const node=element(tag);created.push(node);return node;}};
    document.getElementById("chrome").getBoundingClientRect=()=>bounds;
    document.getElementById("media-hover-status").getBoundingClientRect=()=>({height:18});
    document.getElementById("media-hover").getBoundingClientRect=()=>{
        const stage=document.getElementById("media-hover-viewport").children[0];
        return {width:(Number.parseFloat(stage?.style.width)||100)+10,height:(Number.parseFloat(stage?.style.height)||0)+28};
    };
    const install=vm.runInNewContext(readFileSync(new URL("../assets/media.js",import.meta.url),"utf8")+"\ninstallMedia",{
        document,AbortController,fetch:async()=>({blob:async()=>({size:1})}),
        URL:{createObjectURL(){const url="blob:"+urls.size;urls.add(url);return url;},revokeObjectURL:url=>urls.delete(url)},
        ResizeObserver:class{observe(){}disconnect(){}},requestAnimationFrame:callback=>{frames.set(++nextFrame,callback);return nextFrame;},cancelAnimationFrame:id=>frames.delete(id),
    });
    const ui=install({tr:translator(language),send:packet=>sent.push(packet),prepare(){},changed(){},restore(){}});
    return {ui,nodes,sent,urls,created,bounds,node:document.getElementById,flush(){for(const [id,callback] of [...frames]){frames.delete(id);callback();}}};
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
