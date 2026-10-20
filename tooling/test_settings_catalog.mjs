import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import vm from 'node:vm';
import {translator} from './i18n_test_support.mjs';
const {settingsCatalog,searchSettings}=vm.runInNewContext(readFileSync(new URL('../assets/settings-catalog.js',import.meta.url),'utf8')+'\n({settingsCatalog,searchSettings})');
test('both UI languages find independently expected Chinese, English, case and multiword aliases',()=>{
 for(const language of ['zh-TW','en-US']){
  const index=settingsCatalog(translator(language));
  for(const [query,id]of [['中文字體','font-cjk'],['CJK FONT','font-cjk'],['透明度','tools-opacity'],['FLOATING OPACITY','tools-opacity'],['語言','interface-language'],['language','interface-language'],['right click','tools-right-click'],['白名單',null]]){
   assert.ok(searchSettings(index,query).some(entry=>entry.id===id),language+' '+query);
  }
  assert.equal(searchSettings(index,'').length,0);assert.equal(searchSettings(index,'PTTStellaPassword123').length,0);
 }
});
test('settings search contains only the fixed product index and closed navigation targets',()=>{
 const entries=settingsCatalog(translator('en-US'));assert.equal(entries.length,15);
 assert.ok(entries.every(entry=>['general','appearance','input','media','sites','filters','account','about'].includes(entry.page)));
 assert.equal(searchSettings(entries,'<img src=x onerror=alert(1)>').length,0);
 assert.equal(searchSettings(entries,'x'.repeat(10000)).length,0);
});
test('native settings shortcut captures before BBS input and ignores IME, repeats and untrusted events',()=>{
 const source=readFileSync(new URL('../assets/surface.js',import.meta.url),'utf8');
 const begin=source.indexOf('const settingsUi='),end=source.indexOf('const loginUi=',begin);
 const document={},sent=[];document.getElementById=()=>({});
 const ui=vm.runInNewContext(source.slice(begin,end)+'\nsettingsUi',{
  document,send:value=>sent.push(value),stationMouse:{revoke(){}},cancelHover(){},policy(){},
 });
 const binding=ui.bindings.find(([node])=>node===document);
 assert.equal(binding[3],true,'must intercept before the textarea target handler');
 for(const modifier of ['ctrlKey','metaKey']){
  let prevented=false,stopped=false;
  binding[2]({key:',',[modifier]:true,isTrusted:true,preventDefault(){prevented=true;},stopImmediatePropagation(){stopped=true;}});
  assert.ok(prevented&&stopped);assert.equal(sent.at(-1).kind,'settings_open');
 }
 const count=sent.length;
 for(const changes of [{isComposing:true},{isTrusted:false},{repeat:true},{key:'a'}]){
  binding[2]({key:',',ctrlKey:true,isTrusted:true,preventDefault(){},stopImmediatePropagation(){},...changes});
 }
 assert.equal(sent.length,count);
});
