// Native image decoding, with a bounded visible set. No animation-frame draw loop.
function createGifPlayback({document,window=null,limit=2,pixelBudget=67108864}) {
    const entries=new Set();let disposed=false,focused=document.hasFocus?.()??true;
    function play(entry,value){
        if(entry.playing===value)return;
        entry.playing=value;
        if(value){entry.image.src=entry.url;entry.image.hidden=false;entry.still.hidden=true;}
        else {entry.image.removeAttribute("src");entry.image.hidden=true;entry.still.hidden=false;}
        entry.changed?.(value);
    }
    function refresh(){
        if(disposed)return;
        const eligible=focused&&document.visibilityState!=="hidden"?[...entries]
            .filter(e=>e.enabled).map(e=>({entry:e,score:e.score()})).filter(e=>Number.isFinite(e.score))
            .sort((a,b)=>a.score-b.score):[];
        const candidates=[];let pixels=0;
        for(const {entry} of eligible){if(candidates.length>=limit)break;
            if(entry.cost>pixelBudget-pixels)continue;pixels+=entry.cost;candidates.push(entry);}

        const playing=new Set(candidates);
        // Release old decoders before starting new ones.
        for(const entry of entries)if(!playing.has(entry))play(entry,false);
        for(const entry of candidates)play(entry,true);
    }
    function add({image,still,url,score,changed,cost=pixelBudget}){
        if(disposed)return {enabled:()=>false,playing:()=>false,setEnabled(){},remove(){}};
        const entry={image,still,url,score,changed,cost:Number.isSafeInteger(cost)&&cost>0?cost:pixelBudget,enabled:true,playing:false};
        entries.add(entry);image.removeAttribute("src");image.hidden=true;still.hidden=false;refresh();
        return {enabled:()=>entry.enabled,playing:()=>entry.playing,
            setEnabled(value){if(!entries.has(entry))return;entry.enabled=value;refresh();},
            remove(){play(entry,false);entries.delete(entry);refresh();}};
    }
    function blur(){focused=false;refresh();}
    function focus(){focused=true;refresh();}
    return {add,refresh,clean:()=>disposed&&entries.size===0,
        bindings:[...(window?[[window,"blur",blur],[window,"focus",focus]]:[]),[document,"visibilitychange",refresh]],
        destroy(){disposed=true;for(const entry of entries)play(entry,false);entries.clear();}};
}
