// Production media controller with DOM/network doubles. This checks localization
// and cleanup boundaries, not browser image decoding or codec support.
import test from "node:test";
import assert from "node:assert/strict";
import {readFileSync} from "node:fs";
import vm from "node:vm";
import {translator} from "./i18n_test_support.mjs";

function environment(language){
    const nodes=new Map(),created=[],sent=[],urls=new Set();
    const element=tag=>({tag,style:{},dataset:{},children:[],attributes:{},open:false,hidden:false,
        clientWidth:800,clientHeight:400,naturalWidth:32,naturalHeight:16,paused:true,
        append(...children){this.children.push(...children);},replaceChildren(){this.children=[];},
        get childElementCount(){return this.children.length;},
        setAttribute(name,value){this.attributes[name]=value;},removeAttribute(name){delete this.attributes[name];},
        showModal(){this.open=true;},close(){this.open=false;},focus(){},
        addEventListener(){},removeEventListener(){},pause(){this.paused=true;},load(){},
        getBoundingClientRect(){return {left:0,top:0,right:800,bottom:400,width:800,height:400};},
    });
    const document={getElementById(id){if(!nodes.has(id))nodes.set(id,element(id));return nodes.get(id);},
        querySelector:()=>element("workspace"),createElement(tag){const node=element(tag);created.push(node);return node;}};
    const install=vm.runInNewContext(readFileSync(new URL("../assets/media.js",import.meta.url),"utf8")+"\ninstallMedia",{
        document,AbortController,fetch:async()=>({blob:async()=>({size:1})}),
        URL:{createObjectURL(){const url="blob:"+urls.size;urls.add(url);return url;},revokeObjectURL:url=>urls.delete(url)},
        ResizeObserver:class{observe(){}disconnect(){}},requestAnimationFrame:()=>1,cancelAnimationFrame(){},
    });
    const ui=install({tr:translator(language),send:packet=>sent.push(packet),prepare(){},changed(){},restore(){}});
    return {ui,nodes,sent,urls,created,node:document.getElementById};
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
