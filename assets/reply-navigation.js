// Rust supplies filtered reply rows and roles; this owner only places UI and scrolls locally.
function createReplyNavigation({document,tr,current,allowed,prepare,restore,jump,changed,anchor=()=>({first:0,last:Infinity})}) {
    const root=document.getElementById("reply-navigation"),group=document.getElementById("reply-navigation-group");
    const count=document.getElementById("reply-navigation-count"),previous=document.getElementById("reply-navigation-prev"),next=document.getElementById("reply-navigation-next"),closeButton=document.getElementById("reply-navigation-close");
    const layer=document.getElementById("reply-markers"),openButton=document.getElementById("reply-navigation-open");
    const choices=[...group.querySelectorAll("[data-reply-group]")];
    let dead=false,opened=false,scope=null,packet=null,selected="author",activeRow=null,marks=[];
    const identity=p=>p?.article&&p.ptt_features&&p.navigation?.kind==="article_reading"?JSON.stringify([p.generation,p.article.id]):null;
    const rows=()=>packet?.reply_navigation?.[selected+"_rows"]??[];
    function labels(){
        root.setAttribute("aria-label",tr("reply-navigation-title"));root.dataset.group=selected;
        for(const option of choices){const value=option.dataset.replyGroup;option.textContent=tr("reply-navigation-"+value);option.disabled=!(packet?.reply_navigation?.[value+"_rows"]?.length);option.setAttribute("aria-pressed",String(value===selected));option.title=tr(value==="mine"&&!packet?.reply_navigation?.mine?"reply-navigation-unconfirmed":"reply-navigation-"+value);}
        const list=rows(),at=list.indexOf(activeRow);count.textContent=tr("reply-navigation-count",{current:at<0?0:at+1,total:list.length});
        previous.disabled=next.disabled=!list.length;previous.setAttribute("aria-label",tr("reply-navigation-prev"));next.setAttribute("aria-label",tr("reply-navigation-next"));closeButton.setAttribute("aria-label",tr("close"));previous.title=tr("reply-navigation-prev");next.title=tr("reply-navigation-next");closeButton.title=tr("close");
    }
    function update(p){
        if(dead)return;
        const id=identity(p),changedScope=id!==scope;scope=id;packet=p;
        if(changedScope){close(false);activeRow=null;selected="author";}
        if(!rows().length){selected=["author","mine","focused"].find(k=>p?.reply_navigation?.[k+"_rows"]?.length)??"author";activeRow=null;}
        labels();
        if(openButton)openButton.hidden=!id||!["author_rows","mine_rows","focused_rows"].some(k=>p?.reply_navigation?.[k]?.length);
        if(!id){close(false);clearMarks();}
    }
    function open(e){
        if(dead||!e.isTrusted||!identity(current())||!allowed(true))return false;
        prepare(e);update(current());opened=true;root.hidden=false;labels();choices.find(c=>!c.disabled)?.focus({preventScroll:true});changed();return true;
    }
    function close(focus=true){const was=opened;opened=false;root.hidden=true;if(was){if(focus)restore();changed();}}
    function step(e,back=false){
        if(dead||!opened||!e.isTrusted||!allowed()||packet!==current()||identity(current())!==scope)return;
        e.preventDefault();const list=rows();if(!list.length)return;
        const visible=anchor(),index=activeRow!==null&&activeRow>=visible.first&&activeRow<=visible.last?list.indexOf(activeRow):-1;
        let at=index<0?(back?list.findLastIndex(row=>row<visible.first):list.findIndex(row=>row>=visible.first)):(index+(back?-1:1)+list.length)%list.length;
        if(at<0)at=back?list.length-1:0;
        activeRow=list[at];
        labels();jump({row:activeRow});
    }
    function choose(e){if(dead||!opened||!e.isTrusted)return;if(e.currentTarget.disabled)return;selected=e.currentTarget.dataset.replyGroup;activeRow=null;labels();}
    function clearMarks(){for(const m of marks)m.remove();marks=[];}
    function layout(p,grid,surface,viewport){
        if(dead)return;update(p);
        if(!scope||!grid?.reading){clearMarks();return;}
        const host=layer.parentElement.getBoundingClientRect(),bounds=surface.getBoundingClientRect(),view=viewport.getBoundingClientRect(),ratio=grid.ratio;
        const footerTop=view.bottom-(grid.footerHeight??0),entries=p.reply_navigation?.marks??[];let used=0,low=0,high=entries.length;
        // Metadata is row-sorted by Rust. A long article must not walk all its
        // replies for every wheel frame; only visit the visible range.
        while(low<high){const mid=(low+high)>>>1;if(bounds.top+grid.reading.rowTop(entries[mid].row)/ratio<view.top-.01)low=mid+1;else high=mid;}
        for(let at=low;at<entries.length;at++){
            const mark=entries[at],top=bounds.top+grid.reading.rowTop(mark.row)/ratio,bottom=bounds.top+grid.bottom(mark.row)/ratio;
            if(top>=footerTop)break;
            if(top<view.top-.01||bottom>footerTop+.01||bottom<=top||grid.reading.clippedAt?.(grid.reading.rowTop(mark.row))===mark.row)continue;
            if(used===64)break;
            let node=marks[used];if(!node){node=document.createElement("div");node.className="reply-side-mark";layer.append(node);marks.push(node);}
            node.dataset.role=mark.author&&mark.mine?"both":mark.author?"author":"mine";
            // Standard replies have an independently verified blank separator
            // between the push symbol and ID. Use it when no outer gutter fits;
            // unlike the far right edge it remains visible while zoomed in.
            const outside=bounds.left-host.left>=54;
            const left=outside?bounds.left-host.left-5:bounds.left-host.left+grid.x(2)/ratio+1;
            Object.assign(node.style,{left:left+"px",top:(top-host.top)+"px",height:(bottom-top)+"px",width:Math.max(1,Math.min(mark.author&&mark.mine?5:3,(grid.x(3)-grid.x(2))/ratio-2))+"px"});
            node.textContent=outside&&mark.first?tr("reply-navigation-"+(mark.author&&mark.mine?"both":mark.author?"author":"mine")):"";
            node.setAttribute("aria-label",tr("reply-navigation-"+(mark.author&&mark.mine?"both":mark.author?"author":"mine")));
            used++;
        }
        while(marks.length>used)marks.pop().remove();
    }
    function keys(e){if(opened&&root.contains(e.target)&&e.key==="Escape"&&e.isTrusted){e.preventDefault();e.stopPropagation();close();}}
    function destroy(){if(dead)return;dead=true;close(false);clearMarks();packet=null;scope=null;activeRow=null;}
    return {open,close,update,layout,destroy,ownsFocus:()=>opened&&root.contains(document.activeElement),isOpen:()=>opened,clean:()=>dead&&!opened&&!packet&&!scope&&!marks.length,
        bindings:[[openButton,"click",open],...choices.map(c=>[c,"click",choose]),[previous,"click",e=>step(e,true)],[next,"click",e=>step(e)],[closeButton,"click",e=>{if(e.isTrusted)close();}],[root,"keydown",keys]]};
}
