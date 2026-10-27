import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import vm from 'node:vm';

const source=readFileSync(new URL('../assets/surface.js',import.meta.url),'utf8');
const cases=JSON.parse(readFileSync(new URL('../tests/fixtures/navigation-requests.json',import.meta.url),'utf8'));
function environment(){
    const sent=[],notices=[],c={navigationSequence:0,navigationRequest:false,navigationLocked:false,pendingReadingMove:null,stopped:false,
        latest:{generation:7,presentation:1,input_epoch:2,frame:{revision:4},connected:true,article:{id:1,first_line:1,last_line:23,end:false,load:'cancelled'},navigation:{kind:'article_reading'}},
        viewport:{scrollTop:0,clientHeight:624},surface:{offsetTop:0,dataset:{}},grid:{height:624,ratio:1,y:r=>r*24,bottom:r=>(r+1)*24},
        blocked:()=>false,dragging:null,stationMouse:{local:()=>true,revoke(){}},policy(){},clearSelection(){},cancelHover(){},cancelComposition(){},cancelDrag(){},setHover(){},
        contextMenu:{close(){}},refreshNavigation(){},focus(){},repaint(){},showNotice:value=>{if(value)notices.push(value);},send:value=>sent.push(JSON.parse(JSON.stringify(value)))};
    c.stamp=()=>({generation:c.latest.generation,revision:c.latest.frame.revision,input_epoch:c.latest.input_epoch});
    c.canNavigate=()=>c.latest.connected&&!c.latest.input_pending&&!c.navigationRequest&&!c.navigationLocked&&!c.blocked();
    c.grid.packet=c.latest;
    vm.createContext(c);
    vm.runInContext(source.slice(source.indexOf('function readingMove('),source.indexOf('const browseReturn=')),c);
    vm.runInContext(source.slice(source.indexOf('function invalidatePresentation('),source.indexOf('const ctx=')),c);
    const createRouter=vm.runInContext(readFileSync(new URL('../assets/surface-packets.js',import.meta.url),'utf8')+'\ncreateSurfaceRouter',c);
    const handlers={navigation_result:c.receiveNavigation};
    const route=createRouter(new Proxy(handlers,{get:(target,key)=>(...args)=>target[key]?.(...args)}));
    const frame=(patch={})=>{
        const packet={...c.latest,...patch,frame:{revision:c.latest.frame.revision+1}};
        c.observeNavigation(packet,true);c.latest=packet;c.grid={...c.grid,packet,height:packet.article?packet.article.last_line*24+24:624};c.resumeReadingMove();
    };
    return {c,sent,notices,route,frame,handlers};
}
const trusted={isTrusted:true};
test('an arrow at an unknown edge emits the exact ordinary request validated by Rust, then completes once',()=>{
    const {c,sent,route,frame}=environment();
    c.readingMove({action:'lines',down:true,count:1},trusted,'keyboard-arrow');
    assert.deepEqual(sent,[cases[0].event]);
    route(cases[0].result);assert.equal(c.navigationRequest,false);
    frame({article:{...c.latest.article,last_line:45}});
    assert.equal(c.viewport.scrollTop,24);assert.equal(sent.length,1);
    c.resumeReadingMove();assert.equal(c.viewport.scrollTop,24);
});
test('a rejected request releases the wait and a late result cannot release a newer request',()=>{
    const {c,sent,route}=environment();
    c.navigationSequence=1;c.navigate({action:'next_page'},trusted,'keyboard',false);
    assert.deepEqual(sent[0],cases[1].event);
    route(cases[1].result);assert.equal(c.navigationLocked,false);assert.equal(c.navigationRequest,false);
    c.navigate({action:'next_page'},trusted,'button',false);
    assert.equal(c.navigationRequest.request,3);
    route(cases[1].result);assert.equal(c.navigationRequest.request,3);assert.equal(c.navigationLocked,true);
    route({kind:'navigation_result',payload:{request:3,generation:7,status:'rejected',notice:'deadline expired'}});
    assert.equal(c.navigationRequest,false);assert.equal(c.navigationLocked,false);assert.equal(sent.length,2,'no automatic retry');
});
test('quick-go preparation revokes the old wait; switching boards accepts a new page request despite late replies',()=>{
    const {c,sent,route,frame,handlers}=environment();
    c.navigate({action:'back'},trusted,'button',false);
    const old={...c.navigationRequest};
    const nodes=new Map();
    const document={getElementById(id){if(!nodes.has(id))nodes.set(id,{value:'',open:false,focus(){},showModal(){this.open=true;},close(){this.open=false;},querySelectorAll(){return [];}});return nodes.get(id);},querySelector(){return null;}};
    c.document=document;
    const createDialog=vm.runInContext(readFileSync(new URL('../assets/surface-navigation.js',import.meta.url),'utf8')+'\ncreateNavigationDialog',c);
    const dialog=createDialog({tr:key=>key,send:c.send,stamp:c.stamp,prepare:()=>c.invalidatePresentation(),restore(){},changed(){},current:()=>c.latest,isEditing:()=>false});
    handlers.destination_result=dialog.receive;
    dialog.bindings.find(([node,event])=>node===document&&event==='keydown')[2]({...trusted,key:'k',code:'KeyK',metaKey:true,preventDefault(){},stopPropagation(){}});
    assert.equal(nodes.get('destination-dialog').open,true);
    assert.deepEqual(sent.at(-1),{kind:'navigation_cancel',request:old.request,generation:7});
    nodes.get('destination-name').value='Steam';
    dialog.bindings.find(([node,event])=>node===nodes.get('destination-submit')&&event==='click')[2]({...trusted,preventDefault(){}});
    assert.equal(sent.at(-1).kind,'destination_submit');assert.equal(sent.at(-1).value,'Steam');
    route({kind:'destination_result',payload:{request:1,feedback:{status:'started',notice:null}}});
    assert.equal(nodes.get('destination-dialog').open,false);
    frame({article:null,navigation:{kind:'board_list'}});
    frame({article:{id:2,first_line:1,last_line:23,end:false,load:'cancelled'},navigation:{kind:'article_reading'}});
    c.readingMove({action:'next_page'},trusted);
    const next={...c.navigationRequest};assert.equal(next.request,2);assert.equal(next.article,2);
    route({kind:'navigation_result',payload:{request:old.request,generation:7,status:'cancelled',notice:null}});
    assert.equal(c.navigationRequest.request,next.request);
    route({kind:'navigation_result',payload:{request:next.request,generation:7,status:'sent',notice:null}});
    frame({article:{...c.latest.article,last_line:45}});
    assert.ok(c.viewport.scrollTop>0);assert.equal(c.navigationRequest,false);
    assert.equal(sent.filter(p=>p.kind==='navigate').length,2);
});
test('article changes, reconnects and input cleanup revoke only scoped waits',()=>{
    for(const mode of ['article','generation','input']){
        const {c,sent,route,frame}=environment();c.navigate({action:'next_page'},trusted,'button',false);
        if(mode==='input')c.invalidatePresentation();
        else frame(mode==='article'?{article:{...c.latest.article,id:2}}:{generation:8});
        assert.equal(c.navigationRequest,false);assert.equal(c.pendingReadingMove,null);
        assert.equal(sent.at(-1).kind,'navigation_cancel');
        route(cases[0].result);assert.equal(c.navigationRequest,false);
        assert.equal(c.viewport.scrollTop,0);
    }
});

test('a wheel boundary remains gated until the real redraw, then reverse wheel and pointer Open work',()=>{
    const {c,sent,route,frame}=environment();
    c.latest.article=null;c.latest.navigation={kind:'article_list'};c.latest.input_pending=false;
    Object.assign(c,{contextMenu:{isOpen:()=>false},hasSelection:()=>false,tools:{open:()=>false},stationMouse:{local:()=>true,revoke(){},wheel:()=>false},zoomTo(){}});
    vm.runInContext(source.slice(source.indexOf('function wheel('),source.indexOf('const zoomIn=')),c);
    const wheel=deltaY=>c.wheel({...trusted,deltaY,preventDefault(){}});
    wheel(120);const request={...c.navigationRequest};
    route({kind:'navigation_result',payload:{request:request.request,generation:7,status:'sent',notice:null}});
    // The station accepts the key but sends nothing at its last page.
    frame({input_epoch:3,input_pending:true});
    wheel(120);wheel(-120);c.navigate({action:'open',row:4},trusted);
    assert.equal(sent.filter(p=>p.kind==='navigate').length,1,'no duplicate or premature Open');
    // A genuine redraw with the same visible rows settles the pending input.
    frame({input_epoch:4,input_pending:false});
    wheel(-120);assert.equal(sent.at(-1).intent.action,'previous_page');
    const reverse={...c.navigationRequest};
    route({kind:'navigation_result',payload:{request:reverse.request,generation:7,status:'sent',notice:null}});
    frame({input_epoch:5,input_pending:false});
    c.navigate({action:'open',row:4},trusted);
    assert.deepEqual(sent.at(-1).intent,{action:'open',row:4});
    assert.equal(sent.filter(p=>p.kind==='navigate').length,3);
});
