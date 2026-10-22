import {test} from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import vm from 'node:vm';
const source=readFileSync(new URL('../assets/surface.js',import.meta.url),'utf8');
function environment(){
    const node=()=>({hidden:false,dataset:{},textContent:''}),sent=[];
    const c={articleLoad:node(),articleLoadStatus:node(),articleLoadCancel:node(),articleLoadResume:node(),latest:{article:{id:7,first_line:1,last_line:441,load:'loading'}},generation:4,stopped:false,send:p=>sent.push(p),focus(){},tr:(key,args)=>key+':'+(args?.lines??'')};
    vm.runInNewContext(source.slice(source.indexOf('function refreshArticleLoad()'),source.indexOf('function invalidatePresentation()')),c);
    return {c,sent};
}
test('article loading controls expose progress, cancel and resume only for the matching lifecycle states',()=>{
    const {c}=environment();c.refreshArticleLoad();
    assert.equal(c.articleLoad.hidden,false);assert.equal(c.articleLoadStatus.textContent,'article-load-loading:441');
    assert.equal(c.articleLoadCancel.hidden,false);assert.equal(c.articleLoadResume.hidden,true);
    for(const state of ['cancelled','stopped']){
        c.latest.article.load=state;c.refreshArticleLoad();assert.equal(c.articleLoadCancel.hidden,true);assert.equal(c.articleLoadResume.hidden,false);
    }
    for(const state of ['limit','changed']){
        c.latest.article.load=state;c.refreshArticleLoad();assert.equal(c.articleLoad.hidden,false);assert.equal(c.articleLoadResume.hidden,true);
    }
    c.latest.article.load='complete';c.refreshArticleLoad();assert.equal(c.articleLoad.hidden,true);
    c.latest.article=null;c.refreshArticleLoad();assert.equal(c.articleLoad.hidden,true);
});
test('cancel remains usable during station updates but does not enqueue station navigation or accept synthetic actions',()=>{
    const {c,sent}=environment();c.latest.input_pending=true;c.latest.output_pending=true;
    c.changeArticleLoad({isTrusted:false});assert.equal(sent.length,0);
    c.changeArticleLoad({isTrusted:true});c.changeArticleLoad({isTrusted:true},true);
    assert.deepEqual(JSON.parse(JSON.stringify(sent)),[
        {kind:'article_load',generation:4,article:7,resume:false,trusted:true},
        {kind:'article_load',generation:4,article:7,resume:true,trusted:true}
    ]);
    c.stopped=true;c.changeArticleLoad({isTrusted:true});assert.equal(sent.length,2);
});
test('loading edges wait locally while ordinary cached movement and a cancelled load retain manual boundary navigation',()=>{
    const sent=[],c={latest:{article:{first_line:1,last_line:46,end:false,load:'loading'},connected:true},viewport:{scrollTop:0,scrollHeight:1000,clientHeight:400},grid:{ratio:1,y:r=>r*24,bottom:r=>(r+1)*24,footerHeight:24},blocked:()=>false,dragging:null,clearSelection(){},cancelHover(){},repaint(){},navigate:(intent,...args)=>sent.push(intent)};
    vm.runInNewContext(source.slice(source.indexOf('function readingMove('),source.indexOf('function navigate(')),c);
    c.readingMove({action:'next_page'},{isTrusted:true});assert.equal(c.viewport.scrollTop,338.40000000000003);assert.equal(sent.length,0);
    c.viewport.scrollTop=600;c.readingMove({action:'next_page'},{isTrusted:true});assert.equal(sent.length,0);
    c.latest.article.load='cancelled';c.readingMove({action:'next_page'},{isTrusted:true});assert.deepEqual(JSON.parse(JSON.stringify(sent)),[{action:'next_page'}]);
});
test('Escape cancels a load without station input while selection, dialogs and IME retain their existing owner',()=>{
    let cancelled=0,selected=true,cleared=0,modal=false;
    const c={document:{querySelector:()=>modal?{}:null},stationMouse:{revoke(){}},cancelHover(){},pastePending:false,
        latest:{article:{load:'loading'},navigation:{kind:'article_reading'}},composing:false,blocked:()=>false,hasSelection:()=>selected,dragging:null,
        cancelArticleLoad:()=>cancelled++,keyboardState:{enabled:false,pending:false},cancelDrag(){},clearSelection(){cleared++;},commandKeys:new Set(['Escape'])};
    vm.runInNewContext(source.slice(source.indexOf('function keydown('),source.indexOf('function input(')),c);
    const event={key:'Escape',code:'Escape',isTrusted:true,preventDefault(){}};
    c.keydown(event);assert.equal(cancelled,0);assert.equal(cleared,1);
    selected=false;c.keydown({...event,isComposing:true});assert.equal(cancelled,0);
    modal=true;c.keydown(event);assert.equal(cancelled,0);
    modal=false;c.keydown(event);assert.equal(cancelled,1);
});

test('changed or capacity-stopped article cannot send End across a retained-history gap',()=>{
    const c={latest:{article:{first_line:1,last_line:661,end:false,load:'changed'},connected:true},viewport:{scrollTop:600,scrollHeight:1000,clientHeight:400},grid:{ratio:1,y:r=>r*24,bottom:r=>(r+1)*24,footerHeight:24},blocked:()=>false,dragging:null,clearSelection(){},cancelHover(){},repaint(){},navigate(){assert.fail('cannot fetch across a frozen cache');}};
    vm.runInNewContext(source.slice(source.indexOf('function readingMove('),source.indexOf('function navigate(')),c);
    for(const state of ['changed','limit']){c.latest.article.load=state;c.readingMove({action:'last'},{isTrusted:true});c.readingMove({action:'next_page'},{isTrusted:true});}
});
test('returning to the foreground refreshes the reading gate even without a new station frame',()=>{
    let focused=0,refreshed=0;const c={contextMenu:{isOpen:()=>false},document:{querySelector:()=>null,activeElement:{}},chrome:{contains:()=>false},focus(){focused++;},refreshKeepAlive(){refreshed++;}};
    vm.runInNewContext(source.slice(source.indexOf('function activate()'),source.indexOf('function paste(')),c);
    c.activate();assert.equal(focused,1);assert.equal(refreshed,1);
    assert.match(source,/\[document,"visibilitychange",refreshKeepAlive\]/);
});

test('article-scoped UI permission survives station page transitions but pauses for local interaction',()=>{
    const sent=[],c={latest:{connected:true,input_pending:true,output_pending:true,presentation:2,article:{id:7}},document:{hasFocus:()=>true,visibilityState:'visible'},settingsActive:false,stopped:false,
        blocked:()=>false,canNavigate:()=>false,inlineImages:{active:()=>false},tools:{busy:()=>false},stationMouse:{active:()=>false},dragging:null,released:null,
        hasSelection:()=>false,exportBusy:false,keyboardState:{pending:false},navigationLocked:false,navigationRequest:false,stamp:()=>({generation:1,revision:9,input_epoch:3}),keepAliveGate:null,keepAlive:{setAttribute(){}},tr:k=>k,send:p=>sent.push(p)};
    vm.runInNewContext(source.slice(source.indexOf('function refreshKeepAlive()'),source.indexOf('function toggleKeepAlive(')),c);
    c.refreshKeepAlive();assert.equal(sent.at(-1).reading,true);assert.equal(sent.at(-1).article,7);
    c.latest.article.id=8;c.refreshKeepAlive();assert.equal(sent.at(-1).article,8,'article changes renew permission even on the same stamp');
    c.dragging={};c.refreshKeepAlive();assert.equal(sent.at(-1).reading,false);
    c.dragging=null;c.document.hasFocus=()=>false;c.refreshKeepAlive();assert.equal(sent.at(-1).reading,false);
});
