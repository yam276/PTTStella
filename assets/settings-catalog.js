// Bounded bilingual index: static product labels and aliases, never user data.
function settingsCatalog(tr){
    return [
        ['interface-language','general','language-label','settings-alias-language'],
        ['font-preset','appearance','font-preset','settings-alias-font'],
        ['font-ascii','appearance','font-ascii','settings-alias-ascii'],
        ['font-cjk','appearance','font-cjk','settings-alias-cjk'],
        ['font-size','appearance','font-size','settings-alias-size'],
        ['font-baseline','appearance','font-baseline','settings-alias-baseline'],
        ['font-profile-name','appearance','profile-name','settings-alias-profile'],
        ['tools-opacity','input','settings-opacity','settings-alias-opacity'],
        ['tools-right-click','input','settings-right-click','settings-alias-right-click'],
        ['tools-show-ball','input','settings-show-ball','settings-alias-ball'],
        ['settings-hover','media','hover-label','settings-alias-media'],
        [null,'sites','sites-title','settings-alias-sites'],
        [null,'filters','filters-title','settings-alias-filters'],
        [null,'account','settings-account','settings-alias-account'],
        [null,'about','settings-about','settings-alias-about'],
    ].map(([id,page,label,aliases])=>({id,page,label:tr(label),search:(tr(label)+' '+tr(aliases)).normalize('NFKC').toLowerCase()}));
}
function searchSettings(entries,query){
    const words=query.slice(0,128).normalize('NFKC').toLowerCase().trim().split(/\s+/).filter(Boolean);
    return words.length?entries.filter(entry=>words.every(word=>entry.search.includes(word))):[];
}
function createSettingsNavigation({document,tr,launch}){
    const search=document.getElementById('settings-search'),results=document.getElementById('settings-results');
    const pages=[...document.querySelectorAll('[data-settings-page]')],categories=[...document.querySelectorAll('[data-settings-category]')];
    const entries=settingsCatalog(tr);let current='general';
    function show(page,id){
        current=page;document.getElementById("font-defaults").hidden=!["general","appearance","input","media"].includes(page);search.value='';results.hidden=true;results.replaceChildren();
        for(const section of pages)section.hidden=section.dataset.settingsPage!==page;
        for(const button of categories)button.setAttribute('aria-current',button.dataset.settingsCategory===page?'page':'false');
        document.querySelectorAll('.settings-hit').forEach(node=>node.classList.remove('settings-hit'));
        const field=id?document.getElementById(id):null;
        if(field){field.classList.add('settings-hit');field.scrollIntoView({block:'center'});field.focus();}
    }
    function find(){
        if(!search.value.trim()){show(current);search.focus();return;}
        for(const section of pages)section.hidden=true;
        results.hidden=false;results.replaceChildren();
        const found=searchSettings(entries,search.value);
        if(!found.length){results.textContent=tr('settings-search-empty');return;}
        for(const entry of found){
            const button=document.createElement('button');button.type='button';button.textContent=entry.label+' · '+tr('settings-page-'+entry.page);
            button.addEventListener('click',e=>{if(e.isTrusted)show(entry.page,entry.id);});results.append(button);
        }
    }
    const bindings=[[search,'input',find],...categories.map(button=>[button,'click',e=>{if(e.isTrusted)show(button.dataset.settingsCategory);}]),
        ...[...document.querySelectorAll('[data-settings-launch]')].map(button=>[button,'click',e=>{if(e.isTrusted)launch(button.dataset.settingsLaunch,e);}])];
    show('general');return {bindings,show,current:()=>current,destroy(){results.replaceChildren();}};
}
