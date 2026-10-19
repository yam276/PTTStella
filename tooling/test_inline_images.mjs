import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import vm from 'node:vm';
const source=readFileSync(new URL('../assets/inline-images.js',import.meta.url),'utf8');
const {inlineRowLayout,inlineShortcut}=vm.runInNewContext(source+'\n({inlineRowLayout,inlineShortcut})');
test('inserted gaps never acquire terminal cells and selection height excludes pictures',()=>{
    for(const scale of [.5,1,1.5,2]) {
        const layout=inlineRowLayout(4,row=>row*24*scale,new Map([[0,100*scale],[2,60*scale]]));
        assert.deepEqual(Array.from({length:4},(_,r)=>layout.y(r)),[0,124,148,232].map(n=>n*scale));
        assert.equal(layout.height,256*scale);
        for(let row=0;row<4;row++) {
            assert.equal(layout.bottom(row)-layout.y(row),24*scale);
            assert.equal(layout.rowAt(layout.y(row)+12*scale),row);
        }
        assert.equal(layout.rowAt(40*scale),null);assert.equal(layout.rowAt(190*scale),null);
        assert.equal(layout.rowAt(40*scale,true),0);assert.equal(layout.rowAt(190*scale,true),2);
        assert.equal(layout.rowAt(-1),null);assert.equal(layout.rowAt(9999),null);
    }
});
test('Enter expansion is only offered on an idle local reading page',()=>{
    const key={key:'Enter'}, state={enabled:true,mode:'normal',pending:false};
    const packet={ptt_features:true,connected:true,navigation:{kind:'article_reading'},image_links:[0],mouse:{route:'local'}};
    assert.ok(inlineShortcut(key,state,packet));assert.ok(inlineShortcut(key,{...state,enabled:false,mode:'input'},packet));
    for(const patch of [{ctrlKey:true},{altKey:true},{shiftKey:true},{key:'ArrowRight'}])assert.ok(!inlineShortcut({...key,...patch},state,packet));
    for(const patch of [{mode:'input'},{mode:'once'},{pending:true}])assert.ok(!inlineShortcut(key,{...state,...patch},packet));
    for(const patch of [{ptt_features:false},{connected:false},{input_pending:true},{output_pending:true},{navigation:{kind:'continue_prompt'}},{image_links:[]},{mouse:{route:'station'}}])assert.ok(!inlineShortcut(key,state,{...packet,...patch}));
});
test('production key handler consumes repeated Enter without sending station bytes',()=>{
    const surface=readFileSync(new URL('../assets/surface.js',import.meta.url),'utf8');const sent=[];
    const context={inlineShortcut,keyboardState:{enabled:true,mode:'normal',pending:false},latest:{presentation:3,ptt_features:true,connected:true,navigation:{kind:'article_reading'},image_links:[0],mouse:{route:'local'}},
        stationMouse:{revoke(){}},cancelHover(){},pastePending:false,composing:false,hasSelection:()=>false,clearSelection(){},cancelDrag(){},stamp:()=>({generation:7,revision:4,input_epoch:2}),send:v=>sent.push(JSON.parse(JSON.stringify(v)))};
    vm.runInNewContext(surface.slice(surface.indexOf('function keydown('),surface.indexOf('function input(')),context);
    const event={key:'Enter',code:'Enter',isTrusted:true,ctrlKey:false,altKey:false,shiftKey:false,repeat:false,preventDefault(){}};
    context.keydown(event);context.keydown({...event,repeat:true});context.keydown(event);
    assert.equal(sent.length,2);assert.ok(sent.every(p=>p.kind==='inline_expand'&&p.trusted&&p.presentation===3&&p.stamp.revision===4));
});

function ownerEnvironment(){
    const urls=new Set(),nodes=[];let id=0;
    const element=tag=>{const node={tag,style:{},dataset:{},children:[],width:0,height:0,append(n){this.children.push(n);},insertBefore(n){this.children.push(n);},replaceChildren(){this.children=[];},remove(){},removeAttribute(){},setAttribute(){},getContext:()=>({drawImage(){}})};nodes.push(node);return node;};
    const context={document:{createElement:element},AbortController,setTimeout,clearTimeout,
        fetch:async()=>({blob:async()=>({})}),URL:{createObjectURL(){const url='blob:'+ ++id;urls.add(url);return url;},revokeObjectURL(url){urls.delete(url);}}};
    const create=vm.runInNewContext(source+'\ninstallInlineImages',context);
    const surface=element('div'),canvas=element('canvas');canvas.width=480;canvas.height=96;
    const ui=create({surface,canvas,tr:k=>k,redraw(){},changed(){}});
    const packet={presentation:3,generation:7,frame:{revision:4,rows:4}};
    const open={batch:1,presentation:3,generation:7,revision:4,entries:[{index:0,row:0}]};
    return {ui,urls,nodes,canvas,surface,packet,open};
}
test('reset aborts a pending decoder, releases its URL and ignores late batch delivery',async()=>{
    const e=ownerEnvironment();e.ui.open(e.open,e.packet);
    e.ui.receive({batch:1,index:0,image:{mime:'image/png',data:'AA==',width:2,height:1}});
    await new Promise(resolve=>setImmediate(resolve));assert.equal(e.urls.size,1);
    const image=e.nodes.find(n=>n.tag==='img');assert.equal(typeof image.onload,'function');
    e.ui.reset();assert.equal(e.urls.size,0);assert.equal(image.onload,null);
    e.ui.receive({batch:1,index:0,image:{}});assert.equal(e.ui.active(),false);
    assert.equal(await e.ui.destroy(),true);assert.equal(e.urls.size,0);
});
test('decoded stills survive repeated expansion and are released on a new revision',async()=>{
    const e=ownerEnvironment();e.ui.open(e.open,e.packet);
    e.ui.receive({batch:1,index:0,image:{mime:'image/png',data:'AA==',width:2,height:1}});
    await new Promise(resolve=>setImmediate(resolve));const image=e.nodes.find(n=>n.tag==='img');
    image.naturalWidth=2;image.naturalHeight=1;image.onload();await new Promise(resolve=>setImmediate(resolve));
    const still=e.nodes.find(n=>n.tag==='canvas'&&n!==e.canvas);assert.equal(still.width,2);assert.equal(e.urls.size,0);
    e.ui.open(e.open,e.packet);assert.equal(still.width,2);
    e.ui.layout({...e.packet,frame:{rows:4,revision:5}},{x:c=>c*12,y:r=>r*24},1);
    assert.equal(e.ui.active(),false);assert.equal(still.width,0);assert.equal(e.canvas.style.visibility,'');
    assert.equal(await e.ui.destroy(),true);
});

test('context hit testing uses inserted image boxes and drops stale indices on reset',async()=>{
    const e=ownerEnvironment();e.ui.open({...e.open,entries:[{index:3,row:0}]},e.packet);
    const box=e.nodes.find(n=>n.className==='inline-image');box.getBoundingClientRect=()=>({left:20,right:220,top:80,bottom:140});
    assert.equal(e.ui.linkAtPoint({clientX:100,clientY:100}),3);
    assert.equal(e.ui.linkAtPoint({clientX:220,clientY:100}),-1);
    assert.equal(e.ui.linkAtPoint({clientX:100,clientY:79}),-1);
    e.ui.reset();assert.equal(e.ui.linkAtPoint({clientX:100,clientY:100}),-1);await e.ui.destroy();
});
