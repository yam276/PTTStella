// Isolated Node GC probe for the production inline layout owner. The DOM and
// renderer deliberately retain no Paint references. Run with --expose-gc.
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import vm from 'node:vm';

assert.equal(typeof global.gc,'function','GC probe requires --expose-gc');
const source=readFileSync(new URL('../assets/gif-playback.js',import.meta.url),'utf8')+'\n'+
    readFileSync(new URL('../assets/inline-images.js',import.meta.url),'utf8');
const base={x:c=>c*6,y:r=>r*12};
function environment(){
    let footer=null;
    const make=tag=>({tag,style:{},dataset:{},children:[],width:0,height:0,
        append(node){this.children.push(node);node.parentNode=this;},
        insertBefore(node){this.append(node);},
        replaceChildren(){for(const n of this.children)n.parentNode=null;this.children=[];},
        remove(){if(this.parentNode)this.parentNode.children=this.parentNode.children.filter(n=>n!==this);this.parentNode=null;},
        removeAttribute(name){delete this[name];},setAttribute(){},
        toDataURL(){footer=this;return 'data:image/png;base64,AA==';},
        getContext:()=>({drawImage(){}}),
        getBoundingClientRect(){return {left:0,top:0,right:480,bottom:96,width:480,height:96};}});
    const create=vm.runInNewContext(source+'\ninstallInlineImages',{
        document:{createElement:make},AbortController,setTimeout,clearTimeout});
    const surface=make('div'),canvas=make('canvas'),viewport=make('div'),host=make('main');
    canvas.width=480;canvas.height=96;surface.offsetTop=0;viewport.scrollTop=0;viewport.clientHeight=96;host.append(viewport);
    const ui=create({surface,canvas,viewport,render(){},tr:k=>k,redraw(){},changed(){}});
    return {ui,footer:()=>footer};
}
function article(id=1){
    const style={fg:7,bg:0,bright:false,inverse:false};
    return {presentation:3,generation:7,article:{id,first_line:1,last_line:4000,end:true},
        source_frame:{rows:24},frame:{revision:4,rows:4001,cols:80,
        glyphs:Array.from({length:4001*80},(_,i)=>({row:Math.floor(i/80),col:i%80,width:1,text:'a',left:style,right:style}))},
        links:[],image_links:[]};
}
async function leave(e,mode){
    let packet=article();
    const refs={packet:new WeakRef(packet),glyphs:new WeakRef(packet.frame.glyphs),style:new WeakRef(packet.frame.glyphs[0].left)};
    e.ui.layout(packet,base,1);
    assert.ok(e.footer()?.width>0,'article footer was actually rendered');
    if(mode==='destroy')assert.equal(await e.ui.destroy(),true);
    else if(mode==='new_article')e.ui.layout(article(2),base,1);
    else e.ui.layout({presentation:4,generation:7,article:null,article_mode:mode==='terminal'?'terminal':'web',
        navigation:{kind:mode==='terminal'?'article_reading':'article_list'},
        frame:{revision:5,rows:24,cols:80,glyphs:[]},links:[],image_links:[]},base,1);
    if(mode!=='new_article')assert.equal(e.footer().width,0,'inactive bitmap is released');
    packet=null;
    return refs;
}
async function collect(){for(let i=0;i<8;i++){await new Promise(setImmediate);global.gc();}}
const results=[];
for(const mode of ['list','terminal','new_article','destroy']){
    const e=environment(),refs=await leave(e,mode);
    // Keep the owner and its reusable footer alive while the old article dies.
    await collect();
    const result={mode,packet_collected:refs.packet.deref()===undefined,glyphs_collected:refs.glyphs.deref()===undefined,style_collected:refs.style.deref()===undefined};
    assert.equal(result.packet_collected,true,mode+': old Paint remains reachable');
    assert.equal(result.glyphs_collected,true,mode+': old glyph array remains reachable');
    assert.equal(result.style_collected,true,mode+': indexed glyph copies retain the old styles');
    if(mode!=='destroy'){
        e.ui.layout(article(3),base,1);assert.ok(e.footer().width>0,'footer still works after reuse');
        assert.equal(await e.ui.destroy(),true);
    }
    results.push(result);
}
console.log(JSON.stringify({status:'passed',rows:4000,glyphs:320080,results}));
