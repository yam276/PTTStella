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
    const context={document:{querySelector:()=>null},inlineShortcut,keyboardState:{enabled:true,mode:'normal',pending:false},latest:{presentation:3,ptt_features:true,connected:true,navigation:{kind:'article_reading'},image_links:[0],mouse:{route:'local'}},
        stationMouse:{revoke(){}},cancelHover(){},pastePending:false,composing:false,hasSelection:()=>false,clearSelection(){},cancelDrag(){},stamp:()=>({generation:7,revision:4,input_epoch:2}),send:v=>sent.push(JSON.parse(JSON.stringify(v)))};
    vm.runInNewContext(surface.slice(surface.indexOf('function keydown('),surface.indexOf('function input(')),context);
    const event={key:'Enter',code:'Enter',isTrusted:true,ctrlKey:false,altKey:false,shiftKey:false,repeat:false,preventDefault(){}};
    context.keydown(event);context.keydown({...event,repeat:true});context.keydown(event);
    assert.equal(sent.length,2);assert.ok(sent.every(p=>p.kind==='inline_expand'&&p.trusted&&p.presentation===3&&p.stamp.revision===4));
});

function ownerEnvironment(cached=false){
    const urls=new Set(),nodes=[];let id=0;
    const element=tag=>{const node={tag,style:{},dataset:{},children:[],width:0,height:0,append(n){this.children.push(n);},insertBefore(n){this.children.push(n);},replaceChildren(){this.children=[];},remove(){},removeAttribute(){},setAttribute(){},getContext:()=>({drawImage(){}})};nodes.push(node);return node;};
    const context={document:{createElement:element},AbortController,setTimeout,clearTimeout,
        fetch:async()=>({blob:async()=>({})}),URL:{createObjectURL(){const url='blob:'+ ++id;urls.add(url);return url;},revokeObjectURL(url){urls.delete(url);}}};
    const create=vm.runInNewContext(source+'\ninstallInlineImages',context);
    const surface=element('div'),canvas=element('canvas');canvas.width=480;canvas.height=96;surface.offsetTop=0;
    const viewport=cached?{scrollTop:0,clientHeight:96}:undefined,rendered=[];
    const ui=create({surface,canvas,viewport,render:(target,packet)=>rendered.push(packet),tr:k=>k,redraw(){},changed(){}});
    const packet={presentation:3,generation:7,frame:{revision:4,rows:4}};
    const open={batch:1,presentation:3,generation:7,revision:4,entries:[{index:0,row:0,key:0}]};
    return {ui,urls,nodes,canvas,surface,packet,open,viewport,rendered};
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
test('decoded stills survive a page turn and remap to the current link index without decoding again',async()=>{
    const e=ownerEnvironment();e.ui.open(e.open,e.packet);
    e.ui.receive({batch:1,index:0,image:{mime:'image/png',data:'AA==',width:2,height:1}});
    await new Promise(resolve=>setImmediate(resolve));const image=e.nodes.find(n=>n.tag==='img');
    image.naturalWidth=2;image.naturalHeight=1;image.onload();await new Promise(resolve=>setImmediate(resolve));
    const still=e.nodes.find(n=>n.tag==='canvas'&&n!==e.canvas);assert.equal(still.width,2);assert.equal(e.urls.size,0);
    e.ui.open(e.open,e.packet);assert.equal(still.width,2);
    e.ui.layout({...e.packet,frame:{rows:4,revision:5}},{x:c=>c*12,y:r=>r*24},1);
    assert.equal(e.ui.active(),true);assert.equal(still.width,2);assert.equal(e.canvas.style.visibility,'');
    const next={...e.packet,frame:{rows:4,revision:5}};
    e.ui.open({...e.open,revision:5,entries:[{index:3,row:2,key:0}]},next);
    e.ui.layout(next,{x:c=>c*12,y:r=>r*24},1);
    assert.equal(still.width,2);assert.equal(e.nodes.filter(n=>n.tag==='img').length,1);
    e.ui.reset();assert.equal(still.width,0);
    assert.equal(await e.ui.destroy(),true);
});

test('context hit testing uses inserted image boxes and drops stale indices on reset',async()=>{
    const e=ownerEnvironment();e.ui.open({...e.open,entries:[{index:3,row:0,key:0}]},e.packet);
    const box=e.nodes.find(n=>n.className==='inline-image');box.getBoundingClientRect=()=>({left:20,right:220,top:80,bottom:140});
    assert.equal(e.ui.linkAtPoint({clientX:100,clientY:100}),3);
    assert.equal(e.ui.linkAtPoint({clientX:220,clientY:100}),-1);
    assert.equal(e.ui.linkAtPoint({clientX:100,clientY:79}),-1);
    e.ui.reset();assert.equal(e.ui.linkAtPoint({clientX:100,clientY:100}),-1);await e.ui.destroy();
});

test('an open modal rejects BBS key and text callbacks without forwarding to the station',()=>{
 const surface=readFileSync(new URL('../assets/surface.js',import.meta.url),'utf8'),sent=[];
 const context={document:{querySelector:()=>({open:true})},keyboard:{value:'private setting'},send:p=>sent.push(p)};
 const begin=surface.indexOf('function keydown('),end=surface.indexOf('function input(');
 vm.runInNewContext(surface.slice(begin,end),context);
 let prevented=false;context.keydown({key:'Enter',isTrusted:true,preventDefault(){prevented=true;}});
 const inputEnd=surface.indexOf('function ',end+10);vm.runInNewContext(surface.slice(end,inputEnd),context);
 context.input({data:'text',isTrusted:true});assert.ok(prevented);assert.equal(sent.length,0);assert.equal(context.keyboard.value,'');
});

test('cached rows remain visible across live revisions, virtualize offscreen rows and preserve an absolute-line anchor',async()=>{
    const e=ownerEnvironment(true),base={x:c=>c*6,y:r=>r*24};
    const packet={...e.packet,article:{id:1,first_line:50,last_line:149},frame:{rows:101,revision:4,glyphs:[]},links:[]};
    e.ui.layout(packet,base,1);
    assert.equal(e.canvas.style.visibility,'hidden');assert.equal(e.surface.style.height,'2424px');
    assert.ok(e.rendered.length<=7);assert.ok(e.rendered.every(p=>p.frame.rows===1));
    e.viewport.scrollTop=600;e.ui.layout(packet,base,1);
    assert.equal(e.viewport.scrollTop,600);
    // Twenty earlier rows arrive while the user reads absolute line 75.
    const prepended={...packet,article:{id:1,first_line:30,last_line:149},frame:{rows:121,revision:8,glyphs:[]}};
    e.ui.layout(prepended,base,1);assert.equal(e.viewport.scrollTop,1080);
    // A pending source repaint cannot remove the cached text layer.
    e.ui.layout({...prepended,output_pending:true,frame:{...prepended.frame,revision:9}},base,1);
    assert.equal(e.canvas.style.visibility,'hidden');assert.equal(e.viewport.scrollTop,1080);
    e.ui.layout({...packet,article:{id:2,first_line:1,last_line:100}},base,1);
    assert.equal(e.viewport.scrollTop,0);
    e.ui.layout(e.packet,base,1);assert.equal(e.canvas.style.visibility,'');
    assert.equal(await e.ui.destroy(),true);
});

test('an expanded image keeps its slot during a new source frame and late decoding anchors the text below it',async()=>{
    const e=ownerEnvironment(true),base={x:c=>c*6,y:r=>r*24};
    const packet={...e.packet,article:{id:1,first_line:1,last_line:23},frame:{rows:24,revision:4,glyphs:[]},links:[]};
    e.ui.open({...e.open,first_line:1,article_id:1},packet);
    e.ui.layout(packet,base,1);e.viewport.scrollTop=120;e.ui.layout(packet,base,1);
    e.ui.receive({batch:1,index:0,image:{mime:'image/png',data:'AA==',width:100,height:200}});
    await new Promise(resolve=>setImmediate(resolve));const image=e.nodes.find(n=>n.tag==='img');
    image.naturalWidth=100;image.naturalHeight=200;image.onload();await new Promise(resolve=>setImmediate(resolve));
    e.ui.layout({...packet,frame:{...packet.frame,revision:5}},base,1);
    assert.equal(e.viewport.scrollTop,296); // The same text anchor, +176px decoded height.
    assert.equal(e.canvas.style.visibility,'hidden');
    assert.equal(e.nodes.find(n=>n.className==='inline-image').hidden,false);
    assert.equal(e.nodes.filter(n=>n.tag==='img').length,1);
    await e.ui.destroy();
});

test('production cached movement scrolls locally, fetches only at an unknown edge and rejects duplicate or blocked sends',()=>{
    const surface=readFileSync(new URL('../assets/surface.js',import.meta.url),'utf8'),sent=[];
    const context={latest:{article:{first_line:1,last_line:46,end:false},connected:true,input_pending:false,output_pending:false},
        viewport:{scrollTop:0,scrollHeight:1000,clientHeight:400},grid:{ratio:1,y:r=>24*r,bottom:r=>24*(r+1)},
        blocked:()=>false,dragging:null,hasSelection:()=>false,clearSelection(){},cancelHover(){},repaint(){},
        stationMouse:{local:()=>true,revoke(){}},policy(){},canNavigate:()=>!context.navigationLocked,
        setHover(){},refreshNavigation(){},stamp:()=>({generation:7,revision:20,input_epoch:3}),send:p=>sent.push(p),focus(){},surface:{dataset:{}}};
    vm.runInNewContext(surface.slice(surface.indexOf('function readingMove('),surface.indexOf('const browseReturn=')),context);
    const event={isTrusted:true};
    context.navigate({action:'next_page'},event);assert.equal(context.viewport.scrollTop,360);assert.equal(sent.length,0);
    context.navigate({action:'next_page'},event);assert.equal(context.viewport.scrollTop,600);assert.equal(sent.length,0);
    context.navigate({action:'next_page'},event);assert.equal(sent.length,1);assert.equal(sent[0].stamp.revision,20);
    context.navigate({action:'next_page'},event);assert.equal(sent.length,1);
    context.navigationLocked=false;context.viewport.scrollTop=600;
    context.navigate({action:'previous_page'},event);assert.equal(context.viewport.scrollTop,240);assert.equal(sent.length,1);
    context.viewport.scrollTop=0;context.navigate({action:'previous_page'},event);assert.equal(sent.length,1);
    context.latest.article.end=true;context.viewport.scrollTop=600;context.navigate({action:'next_page'},event);assert.equal(sent.length,1);
    context.latest.input_pending=true;context.navigate({action:'previous_page'},event);assert.equal(context.viewport.scrollTop,240);
    context.latest.input_pending=false;context.blocked=()=>true;context.navigate({action:'previous_page'},event);assert.equal(context.viewport.scrollTop,240);
});

test('production draw never temporarily collapses a cached scroll container to the terminal page height',()=>{
    const surface=readFileSync(new URL('../assets/surface.js',import.meta.url),'utf8');
    const viewport={scrollTop:500,getBoundingClientRect:()=>({width:960,height:576})};
    let height=1600,layoutCalls=0;
    const style={set height(value){height=parseFloat(value);if(height<viewport.scrollTop+576)viewport.scrollTop=Math.max(0,height-576);},get height(){return height+'px';}};
    const context={devicePixelRatio:1,viewport,surface:{style},canvas:{width:960,height:576},level:{},zoom:1,draws:0,hoveredLink:-1,
        render:()=>({x:c=>c*12,y:r=>r*24}),inlineImages:{layout(packet,base){layoutCalls++;assert.equal(viewport.scrollTop,500);return base;}},
        positionKeyboard(){},drawSelection(){},drawNavigationMarker(){}};
    vm.runInNewContext(surface.slice(surface.indexOf('function draw(packet)'),surface.indexOf('function repaint()')),context);
    context.draw({article:{id:1},frame:{rows:47,cols:80},source_frame:{rows:24,cols:80}});
    assert.equal(viewport.scrollTop,500);assert.equal(height,1600);assert.equal(layoutCalls,1);
});

test('cached reading anchors its hidden physical-key input inside the viewport instead of the station footer',()=>{
    const source=readFileSync(new URL('../assets/surface.js',import.meta.url),'utf8');
    const context={grid:{},latest:{article:{id:1},frame:{cursor:[0,256]}},composing:false,
        viewport:{scrollTop:600,scrollLeft:40},surface:{offsetTop:0,offsetLeft:10},keyboard:{style:{}}};
    vm.runInNewContext(source.slice(source.indexOf('function positionKeyboard()'),source.indexOf('function focus()')),context);
    context.positionKeyboard();
    assert.equal(context.keyboard.style.top,'608px');assert.equal(context.keyboard.style.left,'38px');
    assert.equal(context.keyboard.style.height,'1px');
    context.viewport.scrollTop=0;context.positionKeyboard();assert.equal(context.keyboard.style.top,'8px');
});

test('production cached wheel normalizes pixel, line and page units while keeping modifiers and blocked input separate',()=>{
    const source=readFileSync(new URL('../assets/surface.js',import.meta.url),'utf8'),moves=[],zooms=[];
    const context={contextMenu:{isOpen:()=>false},cancelHover(){},tools:{open:()=>false},latest:{article:{id:1},connected:true},
        viewport:{clientHeight:400,scrollWidth:100,clientWidth:100,scrollHeight:1000},blocked:()=>false,dragging:null,hasSelection:()=>false,
        readingMove:(intent,event,origin)=>moves.push([intent.delta,origin]),stationMouse:{wheel:()=>false},zoom:1,zoomTo:value=>zooms.push(value)};
    vm.runInNewContext(source.slice(source.indexOf('function wheel(e)'),source.indexOf('const zoomIn=')),context);
    const event={deltaY:3,deltaMode:0,isTrusted:true,preventDefault(){this.prevented=true;}};
    for(const deltaMode of [0,1,2])context.wheel({...event,deltaMode});
    assert.deepEqual(moves,[[3,'pointer'],[72,'pointer'],[1200,'pointer']]);
    context.wheel({...event,ctrlKey:true});assert.deepEqual(zooms,[.75]);
    for(const patch of [{metaKey:true},{shiftKey:true},{altKey:true}])context.wheel({...event,...patch});
    context.blocked=()=>true;context.wheel({...event});assert.equal(moves.length,3);
});
