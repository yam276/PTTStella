// Execute the production pointer transaction and movement handlers. These DOM
// doubles are independent of native WebView input and do not prove live access.
import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import vm from 'node:vm';

const source=readFileSync(new URL('../assets/surface.js',import.meta.url),'utf8');
function fixture({cached=true}={}){
    const sent=[],selections=[],capture=new Set();
    const f={sent,selections,stopped:false,dragging:null,released:null,navigationLocked:false,navigationRequest:false,
        generation:7,revision:4,composing:false,hoverCandidate:null,
        latest:{presentation:2,connected:true,input_pending:false,output_pending:false,
            frame:{cols:80,rows:cached?81:24},navigation:{kind:'article_reading',first_line:1,percent:50},
            article:cached?{id:1,first_line:1,last_line:80,end:false}:null},
        viewport:{clientHeight:480,clientWidth:960,scrollTop:0,scrollLeft:0,scrollHeight:1944,
            getBoundingClientRect:()=>({top:40,left:0}),setPointerCapture:id=>capture.add(id),
            hasPointerCapture:id=>capture.has(id),releasePointerCapture:id=>capture.delete(id)},
        surface:{dataset:{},offsetTop:0,getBoundingClientRect:()=>({top:40-f.viewport.scrollTop,left:0,width:960,height:cached?1944:576})},
        grid:{ratio:1,x:col=>col*12,y:row=>row*24,bottom:row=>(row+1)*24},
        stationMouse:{local:()=>true,down:()=>false,up:()=>false,move:()=>false,revoke(){}},
        blocked:()=>false,linkAt:()=>-1,send:p=>sent.push(JSON.parse(JSON.stringify(p))),
        canNavigate:()=>!f.latest.input_pending&&!f.navigationLocked&&!f.navigationRequest&&f.latest.connected&&f.latest.navigation.kind!=='unknown',
        stamp:()=>({generation:f.generation,revision:f.revision,input_epoch:0}),sameStamp:(a,b)=>JSON.stringify(a)===JSON.stringify(b),
        policy(){},focus(){},clearSelection(){},cancelHover(){},refreshKeepAlive(){},refreshNavigation(){},setHover(){},repaint(){},
        requestSelection:value=>selections.push(value),media:{locate(){}},hoverPreference:'off',inlineImages:{linkAtPoint:()=>-1}};
    vm.createContext(f);
    for(const [first,last] of [['navigationBackRegion','setHover'],['pointerdown','cancelDrag'],['readingMove',null]]){
        const end=last?source.indexOf(`function ${last}(`):source.indexOf('const browseReturn=');
        vm.runInContext(source.slice(source.indexOf(`function ${first}(`),end),f);
    }
    // Use the same cell geometry as production; image gaps are tested separately.
    f.surface.getBoundingClientRect=()=>({top:40-f.viewport.scrollTop,left:0,right:960,bottom:40-f.viewport.scrollTop+(cached?1944:576),width:960,height:cached?1944:576});
    f.event=(col,y,extra={})=>({button:0,detail:1,isTrusted:true,isPrimary:true,pointerId:1,
        clientX:col*12+6,clientY:40+y,shiftKey:false,preventDefault(){},...extra});
    f.tap=(col,y,extra={})=>{const e=f.event(col,y,extra);f.pointerdown(e);f.pointerup(e);f.click(e);};
    return f;
}

test('reading halves use the visible viewport after scrolling, not the full cached document',()=>{
    const f=fixture();f.viewport.scrollTop=960;
    f.tap(40,100);assert.equal(f.viewport.scrollTop,528);assert.deepEqual(f.sent,[]);
    f.tap(40,380);assert.equal(f.viewport.scrollTop,960);assert.deepEqual(f.sent,[]);
    const narrow=fixture();narrow.viewport.clientHeight=240;narrow.viewport.scrollTop=480;
    narrow.tap(40,150);assert.equal(narrow.viewport.scrollTop,696);
});

test('left seventeen columns return across cached rows while live links have priority',()=>{
    const f=fixture();f.viewport.scrollTop=960;f.tap(16,380);
    assert.deepEqual(f.sent.map(p=>p.intent),[{action:'back'}]);assert.equal(f.viewport.scrollTop,960);
    const link=fixture();link.linkAt=()=>3;link.tap(4,100);
    assert.deepEqual(link.sent,[{kind:'open_link',presentation:2,generation:7,revision:4,index:3,trusted:true}]);
    assert.equal(link.viewport.scrollTop,0);
    const body=fixture();body.tap(17,380);assert.equal(body.viewport.scrollTop,432);assert.deepEqual(body.sent,[]);
});

test('the visible top and bottom row jump Home and End while the central halves still page',()=>{
    const f=fixture();f.latest.article.end=true;f.viewport.scrollTop=960;
    f.tap(40,5);assert.equal(f.viewport.scrollTop,0);assert.deepEqual(f.sent,[]);
    f.tap(40,475);assert.equal(f.viewport.scrollTop,1464);assert.deepEqual(f.sent,[]);
    f.tap(40,100);assert.equal(f.viewport.scrollTop,1032);
    f.tap(40,380);assert.equal(f.viewport.scrollTop,1464);
    // Same geometry at half-height, without making the entire viewport an edge.
    const small=fixture();small.viewport.clientHeight=240;small.viewport.scrollTop=480;
    small.tap(40,5);assert.equal(small.viewport.scrollTop,0);
    small.tap(40,235);assert.equal(small.viewport.scrollTop,1704);
    for(const [y,action] of [[25,'first'],[475,'last']]){
        const raw=fixture({cached:false});raw.latest.navigation.first_line=24;
        raw.tap(40,y);assert.equal(raw.sent[0].intent.action,action);
    }
    const back=fixture();back.tap(4,475);assert.equal(back.sent[0].intent.action,'back');
    const link=fixture();link.linkAt=()=>1;link.tap(40,5);assert.equal(link.sent[0].kind,'open_link');
});
function imageFixture(){
    const f=fixture();f.viewport.scrollTop=960;f.grid.rowAt=()=>null;
    f.inlineImages.linkAtPoint=e=>e.clientX>=0&&e.clientX<960&&e.clientY>=40&&e.clientY<520?0:-1;
    return f;
}
test('expanded image gaps support Back, page, Home and End without becoming selectable terminal cells',()=>{
    for(const [col,y,expected] of [[40,100,528],[40,380,1392],[40,5,0],[40,475,1464]]){
        const f=imageFixture();f.tap(col,y);assert.equal(f.viewport.scrollTop,expected);assert.deepEqual(f.sent,[]);assert.deepEqual(f.selections,[]);
    }
    const back=imageFixture();back.tap(4,380);assert.equal(back.sent[0].intent.action,'back');
    const hover=imageFixture();assert.deepEqual(JSON.parse(JSON.stringify(hover.pointerHit(null,hover.event(40,5)))),{kind:'page',action:'first',image:0});
    assert.equal(hover.cellAt(hover.event(40,5),false),null);
});
test('image navigation rejects Shift, drag, stale geometry, wrong releases, remote route and non-image gaps',()=>{
    for(const reason of ['shift','drag','stale','scroll','resize','other-image','released-stale','blocked','remote','unknown','untrusted','gap']){
        const f=imageFixture(),e=f.event(40,380,{shiftKey:reason==='shift',isTrusted:reason!=='untrusted'});
        if(reason==='blocked')f.blocked=()=>true;
        if(reason==='remote')f.stationMouse.local=()=>false;
        if(reason==='unknown')f.latest.navigation.kind='unknown';
        if(reason==='gap')f.inlineImages.linkAtPoint=()=>-1;
        f.pointerdown(e);
        if(reason==='stale')f.latest.presentation++;
        if(reason==='scroll')f.viewport.scrollTop=800;
        if(reason==='resize')f.viewport.clientHeight=400;
        if(reason==='other-image')f.inlineImages.linkAtPoint=()=>1;
        f.pointerup(reason==='drag'?f.event(42,380):e);
        if(reason==='released-stale')f.latest.presentation++;
        const top=f.viewport.scrollTop;f.click(e);
        assert.equal(f.viewport.scrollTop,top,reason);assert.deepEqual(f.sent,[],reason);assert.deepEqual(f.selections,[],reason);
    }
});

test('a masked partial edge row still offers Home/End navigation without exposing its text or links',()=>{
    for(const [y,expected] of [[5,0],[475,1464]]){
        const f=fixture();f.viewport.scrollTop=960;f.grid.rowAt=()=>null;
        f.grid.reading={clippedAt:()=>59,free:()=>false,align:v=>v};f.linkAt=()=>3;
        f.tap(40,y);assert.equal(f.viewport.scrollTop,expected);assert.deepEqual(f.sent,[]);
        assert.deepEqual(f.selections,[]);
    }
    const outside=fixture();outside.grid.rowAt=()=>null;outside.grid.reading={clippedAt:()=>3,free:()=>false,align:v=>v};
    outside.tap(81,380);assert.deepEqual(outside.sent,[]);assert.equal(outside.viewport.scrollTop,0);
});

test('raw reading excludes headers/footer, computes its visible half, and sends the existing typed intent',()=>{
    for(const [row,action] of [[3,'previous_page'],[18,'next_page']]){
        const f=fixture({cached:false});f.latest.navigation.first_line=24;f.tap(40,row*24+12);
        assert.deepEqual(f.sent.map(p=>[p.kind,p.intent.action,p.source]),[['navigate',action,'pointer']]);
    }
    const f=fixture({cached:false});assert.equal(f.hitAt({row:0,col:40}),null);assert.equal(f.hitAt({row:23,col:40}),null);
    f.latest.navigation.kind='unknown';f.tap(40,380);assert.deepEqual(f.sent,[]);
});

test('only an unknown cache edge fetches once; confirmed first/end and repeated clicks send no bytes',()=>{
    const f=fixture();f.viewport.scrollTop=1464;f.tap(40,380);f.tap(40,380);
    assert.deepEqual(f.sent.map(p=>[p.intent.action,p.source]),[['next_page','pointer']]);
    const end=fixture();end.latest.article.end=true;end.viewport.scrollTop=1464;end.tap(40,380);assert.deepEqual(end.sent,[]);
    const first=fixture();first.tap(40,100);assert.equal(first.viewport.scrollTop,0);assert.deepEqual(first.sent,[]);
});

test('drag, Shift, cancelled capture and crossing the half boundary cannot activate a page',()=>{
    const drag=fixture(),down=drag.event(40,100);drag.pointerdown(down);drag.pointerup(drag.event(42,100));drag.click(down);
    assert.equal(drag.selections.length,1);assert.deepEqual(drag.sent,[]);assert.equal(drag.viewport.scrollTop,0);
    const shift=fixture();shift.tap(40,380,{shiftKey:true});assert.equal(shift.viewport.scrollTop,0);
    const cross=fixture();cross.pointerdown(cross.event(40,239));cross.pointerup(cross.event(40,241));cross.click(cross.event(40,241));
    assert.equal(cross.viewport.scrollTop,0);assert.deepEqual(cross.sent,[]);
    const cancelled=fixture();cancelled.pointerdown(cancelled.event(40,380));cancelled.dragging=null;cancelled.pointerup(cancelled.event(40,380));cancelled.click(cancelled.event(40,380));
    assert.equal(cancelled.viewport.scrollTop,0);
});

test('source, article, presentation, scroll and resize changes invalidate a held or released gesture',()=>{
    for(const mutate of [f=>f.revision++,f=>f.latest.article.id++,f=>f.latest.presentation++,
        f=>f.viewport.scrollTop=24,f=>f.viewport.clientHeight=400]){
        for(const stage of ['held','released']){
            const f=fixture(),e=f.event(40,380);f.pointerdown(e);
            if(stage==='held')mutate(f);f.pointerup(e);if(stage==='released')mutate(f);
            const top=f.viewport.scrollTop;f.click(e);assert.equal(f.viewport.scrollTop,top);assert.deepEqual(f.sent,[]);
        }
    }
});

test('pending output, modal, remote mouse, untrusted events and image gaps cannot trigger a local page',()=>{
    for(const mutate of [f=>f.latest.output_pending=true,f=>f.latest.input_pending=true,
        f=>f.blocked=()=>true,f=>f.stationMouse.local=()=>false,f=>f.latest.connected=false,
        f=>f.grid.rowAt=()=>null]){
        const f=fixture();mutate(f);f.tap(40,380);assert.equal(f.viewport.scrollTop,0);assert.deepEqual(f.sent,[]);
    }
    const f=fixture();f.tap(40,380,{isTrusted:false});assert.equal(f.viewport.scrollTop,0);assert.deepEqual(f.sent,[]);
});

test('reading hover chooses directional cursors without drawing a frame over text',()=>{
    const f=fixture();f.navigationMarker={style:{},dataset:{}};f.surface.style={};
    f.hoveredLink=-1;f.pointerClient=null;f.previewHover=()=>{};f.draw=()=>{};f.tr=key=>key;
    vm.runInContext(source.slice(source.indexOf('function setHover('),source.indexOf('function refreshNavigation(')),f);
    for(const [point,action] of [[{row:3,col:40},'previous_page'],[{row:18,col:40},'next_page'],[{row:3,col:0},'back']]){
        f.setHover(point,false,false);
        assert.equal(f.surface.dataset.pointerAction,action);
        assert.equal(f.surface.style.cursor,'');
        assert.equal(f.surface.title,'');
        assert.equal(f.navigationMarker.hidden,true);
    }
});

// A pause owns the whole grid, including links and the usual Back gutter.
function pauseFixture(){const f=fixture({cached:false});f.latest.navigation={kind:'continue_prompt'};f.viewport.clientHeight=576;return f;}
test('confirmed pause clicks anywhere send Continue once, ahead of links and Back',()=>{
    for(const [col,y] of [[0,1],[40,280],[79,575]]){
        const f=pauseFixture();f.linkAt=()=>0;f.tap(col,y);f.tap(col,y);
        assert.deepEqual(f.sent,[{kind:'navigate',stamp:{generation:7,revision:4,input_epoch:0},intent:{action:'continue'},source:'pointer',trusted:true}]);
    }
    const f=pauseFixture();f.tap(40,280);f.navigationRequest=false;f.navigationLocked=false;
    f.latest.navigation={kind:'board_list',first_row:3,last_row:22};f.revision++;
    f.tap(40,280,{detail:2});assert.equal(f.sent.length,1);
});
test('pause gestures reject drag, Shift, untrusted, outside, busy and remote input',()=>{
    for(const change of [f=>f.latest.output_pending=true,f=>f.latest.input_pending=true,f=>f.blocked=()=>true,
        f=>f.stationMouse.local=()=>false,f=>f.latest.connected=false,f=>f.latest.navigation.kind='unknown']){
        const f=pauseFixture();change(f);f.tap(40,280);assert.deepEqual(f.sent,[]);
    }
    for(const extra of [{shiftKey:true},{isTrusted:false},{button:2}]){const f=pauseFixture();f.tap(40,280,extra);assert.deepEqual(f.sent,[]);}
    const outside=pauseFixture();outside.tap(80,280);assert.deepEqual(outside.sent,[]);
    const drag=pauseFixture();drag.pointerdown(drag.event(40,280));drag.pointerup(drag.event(42,280));drag.click(drag.event(42,280));assert.deepEqual(drag.sent,[]);assert.equal(drag.selections.length,1);
});
test('pause source or geometry changes cancel both held and released clicks',()=>{
    for(const change of [f=>f.revision++,f=>f.generation++,f=>f.latest.presentation++,f=>f.viewport.scrollTop=24,f=>f.viewport.clientHeight=400]){
        for(const stage of ['held','released']){
            const f=pauseFixture(),e=f.event(40,280);f.pointerdown(e);
            if(stage==='held')change(f);f.pointerup(e);if(stage==='released')change(f);
            f.click(e);assert.deepEqual(f.sent,[]);
        }
    }
});

test('home menu hover marker spans both edges of the terminal grid',()=>{
    const f=fixture({cached:false});f.latest.navigation={kind:'main_menu',first_row:3,last_row:10};
    f.hoveredLink=-1;f.pointerClient=null;f.previewHover=()=>{};f.draw=()=>{};
    f.tr=key=>key;f.surface.style={};f.navigationMarker={style:{},dataset:{}};
    vm.runInContext(source.slice(source.indexOf('function setHover('),source.indexOf('function refreshNavigation(')),f);
    f.setHover({row:4,col:25},false,false);
    assert.equal(f.navigationMarker.hidden,false);assert.equal(f.navigationMarker.style.left,'0px');
    assert.equal(f.navigationMarker.style.width,'960px');assert.equal(f.navigationMarker.style.height,'24px');
    assert.equal(f.surface.title,'');
});

test('only the visible author word toggles reply focus; content, time and adjacent spaces navigate',()=>{
    const setup=()=>{const f=fixture();f.latest.reply_rows=[5];f.latest.account_words=[{row:5,start:3,end:8,id:'Alice'},{row:5,start:30,end:35,id:'Alice'}];f.latest.reply_author_words=[0];return f;};
    for(const col of [3,4,7]){const f=setup();f.tap(col,5*24+12);
        assert.deepEqual(f.sent,[{kind:'reply_focus',presentation:2,stamp:{generation:7,revision:4,input_epoch:0},row:5,trusted:true}]);}
    for(const col of [0,2,8,16]){const f=setup();f.tap(col,5*24+12);assert.equal(f.sent[0].intent.action,'back');}
    for(const col of [17,30,40,70]){const f=setup();f.viewport.scrollTop=240;f.tap(col,5*24+12);assert.equal(f.viewport.scrollTop,0);assert.deepEqual(f.sent,[]);}
    const link=setup();link.linkAt=()=>2;link.tap(4,5*24+12);assert.equal(link.sent[0].kind,'open_link');
    const hidden=setup();hidden.latest.reply_author_words=[];hidden.tap(4,5*24+12);assert.equal(hidden.sent[0].intent.action,'back');
    for(const kind of ['shift','drag','stale','untrusted']){
        const x=setup();const e=x.event(4,5*24+12,{shiftKey:kind==='shift',isTrusted:kind!=='untrusted'});
        x.pointerdown(e);if(kind==='stale')x.latest.presentation++;x.pointerup(kind==='drag'?x.event(5,5*24+12):e);x.click(e);assert.deepEqual(x.sent,[],kind);
    }
});

test('basic reply and selection hover stays quiet while links retain their literal destination',()=>{
    const f=fixture();f.surface.style={};f.navigationMarker={style:{},dataset:{}};
    f.hoveredLink=-1;f.pointerClient=null;f.previewHover=()=>{};f.draw=()=>{};f.tr=key=>key;
    f.latest.reply_rows=[5];f.latest.account_words=[{row:5,start:3,end:8,id:'Alice'}];f.latest.reply_author_words=[0];f.latest.links=[{url:'https://example.com/a?b=1&c=2'}];
    vm.runInContext(source.slice(source.indexOf('function setHover('),source.indexOf('function refreshNavigation(')),f);
    f.setHover({row:5,col:40},false,false);assert.equal(f.surface.title,'');
    f.dragging={moved:true};f.setHover({row:5,col:40},false,false);assert.equal(f.surface.title,'');
    f.dragging=null;f.linkAt=()=>0;f.setHover({row:5,col:40},false,false);
    assert.equal(f.surface.title,'https://example.com/a?b=1&c=2');
});
