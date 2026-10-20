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
test('in-app settings shortcut reuses the dialog and captures before BBS input',()=>{
 const source=readFileSync(new URL('../assets/surface-settings-host.js',import.meta.url),'utf8');
 const dialog={open:false},search={focus(){focused++;}},document={getElementById:id=>id==='font-dialog'?dialog:search};
 let requested=0,focused=0,closed=0,disposed=0;const sent=[];
 const factory=vm.runInNewContext(source+'\ncreateSurfaceSettings',{
  createSettingsDialog:options=>({bindings:[],request(){requested++;},close(){closed++;},open(){dialog.open=true;},destroy(){disposed++;}}),
  createSettingsNavigation:()=>({bindings:[],current:()=>"general",destroy(){}})
 });
 const ui=factory({document,send:v=>sent.push(v),tr:()=>''});
 const binding=ui.bindings.find(([node])=>node===document);assert.equal(binding[3],true);
 function shortcut(extra={}){let prevented=false,stopped=false;binding[2]({key:',',ctrlKey:true,isTrusted:true,preventDefault(){prevented=true;},stopImmediatePropagation(){stopped=true;},...extra});return prevented&&stopped;}
 assert.ok(shortcut());assert.equal(requested,1);
 dialog.open=true;assert.ok(shortcut());assert.equal(requested,1);assert.equal(focused,1);
 for(const extra of [{isComposing:true},{isTrusted:false},{repeat:true},{key:'a'}])shortcut(extra);
 assert.equal(requested,1);assert.equal(focused,1);assert.equal(sent.length,0);ui.destroy();assert.equal(disposed,1);
});

test('settings management handoff waits for the autosave-aware modal close',()=>{
 const source=readFileSync(new URL('../assets/surface-settings-host.js',import.meta.url),'utf8');
 let options,navigation,closeCalls=0;const sent=[];
 const factory=vm.runInNewContext(source+'\ncreateSurfaceSettings',{
  createSettingsDialog:o=>{options=o;return {bindings:[],close(){closeCalls++;},destroy(){}};},
  createSettingsNavigation:o=>{navigation=o;return {bindings:[],current:()=>"sites",destroy(){}};}
 });
 const ui=factory({document:{getElementById:()=>({open:true})},send:v=>sent.push(JSON.parse(JSON.stringify(v))),tr:()=>''});
 navigation.launch('sites',{isTrusted:true});assert.equal(closeCalls,1);assert.equal(sent.length,0);
 options.onClose();assert.deepEqual(sent,[{kind:'settings_visibility',open:false},{kind:'settings_launch',feature:'sites',trusted:true}]);
 options.onClose();assert.equal(sent.filter(x=>x.kind==='settings_launch').length,1);ui.destroy();
});
