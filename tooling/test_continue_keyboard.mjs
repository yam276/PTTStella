// Production bridge event routing; Rust tests own pause translation and repeats.
import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import vm from 'node:vm';
const source=readFileSync(new URL('../assets/surface.js',import.meta.url),'utf8');
function fixture(enabled=false){
    const f={revokeNavigation(){},sent:[],generation:7,composing:false,pastePending:false,selected:false,modal:false,
        keyboardState:{enabled,mode:enabled?'normal':'input'},latest:{navigation:{kind:'continue_prompt'}},
        document:{querySelector:()=>f.modal?{}:null},stationMouse:{revoke(){}},cancelHover(){},cancelDrag(){},clearSelection(){f.selected=false;},
        hasSelection:()=>f.selected,inlineShortcut:()=>false,inlineImages:{keyHeld:()=>false},zoom:1,zoomTo(){f.zoomed=true;},send:p=>f.sent.push(JSON.parse(JSON.stringify(p)))};
    vm.createContext(f);vm.runInContext(source.slice(source.indexOf('const commandKeys='),source.indexOf('function input(')),f);
    f.key=(key,extra={})=>{const e={key,code:'KeyJ',keyCode:0,isTrusted:true,isComposing:false,ctrlKey:false,altKey:false,metaKey:false,shiftKey:false,repeat:false,prevented:false,preventDefault(){this.prevented=true;},...extra};f.keydown(e);return e;};
    return f;
}
test('ordinary, navigation, Escape and function keys reach Rust in both modes without textarea edits',()=>{
    for(const enabled of [false,true])for(const key of ['j','i','f','3','Escape','Tab','Enter','F2','ArrowLeft','中']){
        const f=fixture(enabled),e=f.key(key);assert.equal(e.prevented,true);assert.equal(f.sent.length,1);
        assert.equal(f.sent[0].kind,'key');assert.equal(f.sent[0].key.key,key);assert.equal(f.sent[0].generation,7);
    }
});
test('pause keeps native IME, clipboard, local shortcuts and modal editing owners',()=>{
    for(const enabled of [false,true])for(const extra of [{isComposing:true},{keyCode:229},{key:'Process'},{metaKey:true}]){
        const f=fixture(enabled),e=f.key('Enter',{code:'Enter',...extra});assert.equal(e.prevented,false);assert.deepEqual(f.sent,[]);
    }
    for(const change of [f=>f.composing=true,f=>f.modal=true,f=>f.pastePending=true]){const f=fixture();change(f);f.key('j');assert.deepEqual(f.sent,[]);}
    for(const key of ['Dead','Shift','Control','Alt','Meta','CapsLock','NumLock','ScrollLock','Fn']){const f=fixture();f.selected=true;assert.equal(f.key(key).prevented,false);assert.deepEqual(f.sent,key==='Dead'?[{kind:'key',key:{key:'Dead',code:'KeyJ',ctrl:false,alt:false,shift:false,repeat:false},generation:7,trusted:true}]:[]);assert.equal(f.selected,true);}
    const copy=fixture();copy.selected=true;copy.key('c',{ctrlKey:true});assert.deepEqual(copy.sent,[]);
    const paste=fixture();paste.key('v',{ctrlKey:true});assert.deepEqual(paste.sent,[]);
    const escape=fixture();escape.selected=true;escape.key('Escape');assert.deepEqual(escape.sent,[]);assert.equal(escape.selected,false);
    const zoom=fixture();zoom.key('+',{ctrlKey:true});assert.equal(zoom.zoomed,true);assert.deepEqual(zoom.sent,[]);
    const unknown=fixture();unknown.latest.navigation.kind='unknown';assert.equal(unknown.key('F2').prevented,false);assert.deepEqual(unknown.sent,[]);
});

test('normal readonly CJK physical commands reach Rust, while native composition keeps its owner',()=>{
    const normal=fixture(true),e=normal.key('Process',{keyCode:229});assert.equal(e.prevented,true);assert.equal(normal.sent[0].key.code,'KeyJ');
    for(const f of [fixture(false),Object.assign(fixture(true),{composing:true})]){assert.equal(f.key('Process',{keyCode:229}).prevented,false);assert.deepEqual(f.sent,[]);}
});
test('CJK Control shortcuts use the physical command path without admitting text or stealing clipboard/IME',()=>{
    for(const enabled of [false,true])for(const keyCode of [0,229]){
        const f=fixture(enabled);f.latest.navigation.kind='unknown';
        const e=f.key('ㄌ',{code:'KeyX',ctrlKey:true,keyCode});
        assert.equal(e.prevented,true);assert.equal(f.sent.length,1);
        assert.deepEqual(f.sent[0].key,{key:'ㄌ',code:'KeyX',ctrl:true,alt:false,shift:false,repeat:false});
    }
    for(const change of [f=>f.composing=true,f=>f.modal=true]){
        const f=fixture();change(f);f.key('ㄌ',{code:'KeyX',ctrlKey:true});assert.deepEqual(f.sent,[]);
    }
    const composing=fixture();composing.key('ㄌ',{code:'KeyX',ctrlKey:true,isComposing:true});assert.deepEqual(composing.sent,[]);
    const copy=fixture();copy.selected=true;copy.key('ㄏ',{code:'KeyC',ctrlKey:true});assert.deepEqual(copy.sent,[]);
    const paste=fixture();paste.key('ㄒ',{code:'KeyV',ctrlKey:true});assert.deepEqual(paste.sent,[]);
    const text=fixture();text.latest.navigation.kind='unknown';const e=text.key('ㄌ',{code:'KeyX'});assert.equal(e.prevented,false);
    assert.equal(text.sent[0].kind,'key'); // Rust still decides ordinary text admission.
});

test('PTT editor Ctrl+V sends one station key instead of pasting, including CJK key delivery',()=>{
    for(const extra of [{key:'v',code:'KeyV'},{key:'ㄒ',code:'KeyV',keyCode:229}]){
        const f=fixture();f.latest={editor:true,ptt_features:true,navigation:{kind:'unknown'}};
        const e=f.key('v',{ctrlKey:true,...extra});
        assert.equal(e.prevented,true);assert.equal(f.sent.length,1);assert.equal(f.sent[0].kind,'key');
        assert.equal(f.sent[0].key.code,'KeyV');assert.equal(f.sent[0].key.ctrl,true);
    }
    for(const extra of [{shiftKey:true},{metaKey:true},{isComposing:true}]){
        const f=fixture();f.latest={editor:true,ptt_features:true,navigation:{kind:'unknown'}};
        assert.equal(f.key('v',{ctrlKey:true,code:'KeyV',...extra}).prevented,false);assert.deepEqual(f.sent,[]);
    }
});

test('IME-delivered plain space in web reading stays a local page action',()=>{
    const f=fixture();f.latest={article:{id:1},navigation:{kind:'article_reading'}};
    Object.assign(f,{keyboard:{value:' '},composition:null,beforeEdit:{composition:false},sendInput:p=>f.sent.push(p),readingMove:(intent)=>f.movement=intent});
    vm.runInContext(source.slice(source.indexOf('function input('),source.indexOf('function compositionstart(')),f);
    f.input({data:' ',inputType:'insertText',isTrusted:true});
    assert.equal(f.movement?.action,'next_page');assert.deepEqual(f.sent,[]);
});

test('native composition, Vim input, terminal text and non-space edits keep their text owner',()=>{
    for(const reason of ['composition','trailing','vim','terminal','text']){
        const f=fixture(reason==='vim');f.latest={article:reason==='terminal'?null:{id:1},navigation:{kind:'article_reading'}};
        Object.assign(f,{keyboard:{value:' '},composition:{},positionKeyboard(){},beforeEdit:{composition:reason==='trailing'},sendInput:p=>f.sent.push(p),readingMove:()=>assert.fail('must retain text/IME ownership')});
        vm.runInContext(source.slice(source.indexOf('function input('),source.indexOf('function compositionstart(')),f);
        f.composing=reason==='composition';f.input({data:reason==='text'?'中':' ',inputType:'insertText',isTrusted:true});
        assert.equal(f.sent.length,['composition','trailing'].includes(reason)?0:1,reason);
    }
});

test('a trusted IME-confirmed lone space pages locally; cancellation and committed Chinese keep composition ownership',()=>{
    for(const value of [' ','','中']){
        const f=fixture();f.latest={article:{id:1},navigation:{kind:'article_reading'}};
        Object.assign(f,{composition:{generation:7,startTrusted:true,editTrusted:true,editText:value},keyboard:{value,classList:{remove(){}}},positionKeyboard(){},refreshNavigation(){},policy(){},sendInput:p=>f.sent.push(p),readingMove:intent=>f.movement=intent});
        vm.runInContext(source.slice(source.indexOf('function compositionend('),source.indexOf('function cancelComposition(')),f);
        f.compositionend({data:value,isTrusted:true});
        assert.equal(f.movement?.action,value===' '?'next_page':undefined);
        assert.equal(f.sent.length,value==='中'?1:0);
        if(value==='中')assert.equal(f.sent[0].composed,true);
    }
});
