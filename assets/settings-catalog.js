// Bounded bilingual index: static product labels and aliases, never user data.
function settingsCatalog(tr){
    return SETTINGS_CATALOG.map(([id,page,label,aliases])=>({id,page,label:tr(label),search:(tr(label)+' '+tr(aliases)).normalize('NFKC').toLowerCase()}));
}
function searchSettings(entries,query){
    let bounded=query.slice(0,128);
    // Native strings contain complete Unicode scalars. Avoid creating a lone
    // high surrogate when the shared UTF-16 limit cuts an astral character.
    if(/[\uD800-\uDBFF]$/.test(bounded))bounded=bounded.slice(0,-1);
    const words=bounded.normalize('NFKC').toLowerCase().trim().split(/\s+/).filter(Boolean);
    return words.length?entries.filter(entry=>words.every(word=>entry.search.includes(word))):[];
}
function createSettingsNavigation({document,tr,changed=()=>{}}){
    const search=document.getElementById('settings-search'),results=document.getElementById('settings-results');
    const pages=[...document.querySelectorAll('[data-settings-page]')],categories=[...document.querySelectorAll('[data-settings-category]')];
    let entries=settingsCatalog(tr);let current='general';
    const title=document.getElementById('settings-page-title');
    function heading(searching=false){
        if(title)title.textContent=searching?tr('settings-search-label'):tr('settings-page-'+current);
    }
    function show(page,id,event){
        const previous=current;current=page;heading();document.getElementById("font-defaults").hidden=!["general","appearance","input","media"].includes(page);search.value='';results.hidden=true;results.replaceChildren();
        for(const section of pages)section.hidden=section.dataset.settingsPage!==page;
        for(const button of categories)button.setAttribute('aria-current',button.dataset.settingsCategory===page?'page':'false');
        document.querySelectorAll('.settings-hit').forEach(node=>node.classList.remove('settings-hit'));
        changed(page,previous,event);
        const field=id?document.getElementById(id):null;
        if(field){field.classList.add('settings-hit');field.scrollIntoView({block:'center'});field.focus();}
    }
    function find(){
        if(!search.value.trim()){show(current);search.focus();return;}
        changed(null,current);heading(true);
        for(const section of pages)section.hidden=true;
        results.hidden=false;results.replaceChildren();
        const found=searchSettings(entries,search.value);
        if(!found.length){results.textContent=tr('settings-search-empty');return;}
        for(const entry of found){
            const button=document.createElement('button');button.type='button';button.textContent=entry.label+' · '+tr('settings-page-'+entry.page);
            button.addEventListener('click',e=>{if(e.isTrusted)show(entry.page,entry.id,e);});results.append(button);
        }
    }
    const bindings=[[search,'input',find],...categories.map(button=>[button,'click',e=>{if(e.isTrusted)show(button.dataset.settingsCategory,null,e);}])];
    show('general');return {bindings,show,relocalize(){entries=settingsCatalog(tr);if(search.value.trim())find();else heading();},current:()=>current,visible:()=>results.hidden?current:null,destroy(){results.replaceChildren();}};
}
