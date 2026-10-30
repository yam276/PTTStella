import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import vm from 'node:vm';
import {translator} from './i18n_test_support.mjs';
const {settingsCatalog,searchSettings}=vm.runInNewContext(readFileSync(new URL('../assets/settings-catalog.js',import.meta.url),'utf8')+'\n({settingsCatalog,searchSettings})');
test('both UI languages find independently expected Chinese, English, case and multiword aliases',()=>{
 for(const language of ['zh-TW','en-US']){
  const index=settingsCatalog(translator(language));
  for(const [query,id]of [['中文字體','font-cjk'],['CJK FONT','font-cjk'],['透明度','tools-opacity'],['FLOATING OPACITY','tools-opacity'],['文章模式','article-mode'],['Article mode','article-mode'],['語言','interface-language'],['language','interface-language'],['right click','tools-right-click'],['白名單',null],['縮放','zoom-fit'],['experimental keyboard','vim-toggle'],['站端 滑鼠','station-mouse'],['keepalive','keep-alive']]){
   assert.ok(searchSettings(index,query).some(entry=>entry.id===id),language+' '+query);
  }
  assert.equal(searchSettings(index,'').length,0);assert.equal(searchSettings(index,'PTTStellaPassword123').length,0);
 }
});
test('settings search contains only the fixed product index and closed navigation targets',()=>{
 const entries=settingsCatalog(translator('en-US'));assert.equal(entries.length,21);
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

test('management categories stay embedded, open automatically and clean drafts on leaving',async()=>{
 const source=readFileSync(new URL('../assets/surface-settings-host.js',import.meta.url),'utf8');
 const nodes=Object.fromEntries(['font-dialog','sites-dialog','filters-dialog','login-dialog'].map(id=>[id,{open:false,close(){this.open=false;}}]));
 let options,nav,current='general',requested=0,password='';const sent=[],visits=[];
 const features=Object.fromEntries([['sites','sites-dialog'],['filters','filters-dialog'],['account','login-dialog']].map(([page,id])=>[page,{
  request(){visits.push(page);nodes[id].open=true;},close(){nodes[id].close();if(page==='account')password='';}
 }]));
 const factory=vm.runInNewContext(source+'\ncreateSurfaceSettings',{
  createSettingsDialog:o=>{options=o;return {bindings:[],request(){requested++;},open(){nodes['font-dialog'].open=true;},close(){nodes['font-dialog'].close();o.onClose();},destroy(){}};},
  createSettingsNavigation:o=>{nav={bindings:[],current:()=>current,show(page,id,event){const previous=current;current=page;o.changed(page,previous,event);},destroy(){}};return nav;}
 });
 const ui=factory({document:{getElementById:id=>nodes[id]},features:()=>features,send:v=>sent.push(JSON.parse(JSON.stringify(v))),tr:()=>''});
 assert.equal(ui.manage('filters',{isTrusted:false}),false);assert.equal(requested,0);
 assert.equal(ui.manage('filters',{isTrusted:true}),false);assert.equal(requested,1);assert.deepEqual(visits,[]);
 await ui.open({});assert.deepEqual(visits,['filters']);assert.equal(ui.manage('filters',{isTrusted:true}),true);
 nav.show('account',null,{isTrusted:true});password='private draft';assert.equal(nodes['filters-dialog'].open,false);
 nav.show('sites',null,{isTrusted:true});assert.equal(password,'');assert.equal(nodes['login-dialog'].open,false);
 assert.equal(nodes['font-dialog'].open,true);assert.equal(sent.length,0);
 nav.show('about');assert.equal(nodes['sites-dialog'].open,false);assert.equal(ui.visible('about'),true);
 nav.show('account');password='another draft';ui.close({isTrusted:true});assert.equal(password,'');
 assert.ok(Object.values(nodes).every(node=>!node.open));assert.deepEqual(sent,[{kind:'settings_visibility',open:false}]);
 ui.destroy();assert.equal(ui.manage('sites',{isTrusted:true}),false);
});

test('search leaves the visible management page and reopening it restores its controls',()=>{
 const source=readFileSync(new URL('../assets/settings-catalog.js',import.meta.url),'utf8');
 const nodes=new Map(),changes=[];
 const node=id=>{if(!nodes.has(id))nodes.set(id,{id,value:'',hidden:true,children:[],replaceChildren(){this.children=[];},append(child){this.children.push(child);},focus(){}});return nodes.get(id);};
 const sections=['general','account','sites','filters','about'].map(page=>({dataset:{settingsPage:page},hidden:true}));
 const categories=sections.map(section=>({dataset:{settingsCategory:section.dataset.settingsPage},setAttribute(){}}));
 const document={getElementById:node,querySelectorAll:selector=>selector==='[data-settings-page]'?sections:selector==='[data-settings-category]'?categories:[],createElement:()=>({addEventListener(){}})};
 const create=vm.runInNewContext(source+'\ncreateSettingsNavigation');
 const ui=create({document,tr:translator('zh-TW'),changed:(page,previous)=>changes.push([page,previous])});
 ui.show('account');assert.equal(ui.visible(),'account');assert.equal(node('settings-page-title').textContent,'帳號與隱私');node('settings-search').value='站點';
 ui.bindings.find(([target,type])=>target===node('settings-search')&&type==='input')[2]();
 assert.equal(ui.visible(),null);assert.equal(node('settings-page-title').textContent,'搜尋設定');assert.deepEqual(changes.at(-1),[null,'account']);assert.ok(sections.every(s=>s.hidden));
 ui.show('sites');assert.equal(ui.visible(),'sites');assert.equal(node('settings-page-title').textContent,'站點與連線');assert.equal(sections.find(s=>s.dataset.settingsPage==='sites').hidden,false);
 ui.destroy();
});

test('closing a nonmodal account panel restores the visible search field after browser focus restoration',()=>{
 const source=readFileSync(new URL('../assets/surface-settings-host.js',import.meta.url),'utf8');
 let navigation,focused=0,closed=0;const search={closest:()=>null,focus(){focused++;}};
 const dialog={open:true,contains:node=>node===search},account={open:true};
 const document={activeElement:search,getElementById:id=>id==='font-dialog'?dialog:id==='login-dialog'?account:null};
 const factory=vm.runInNewContext(source+'\ncreateSurfaceSettings',{
  createSettingsDialog:()=>({bindings:[],destroy(){}}),
  createSettingsNavigation:o=>{navigation=o;return {bindings:[],current:()=>"account",destroy(){}};}
 });
 const ui=factory({document,send(){},features:()=>({account:{close(){closed++;account.open=false;document.activeElement=dialog;}}}),tr:()=>''});
 navigation.changed(null,'account');assert.equal(closed,1);assert.equal(focused,1);
 search.closest=()=>({hidden:true});account.open=true;navigation.changed('sites','account');assert.equal(focused,1);
 ui.destroy();
});
