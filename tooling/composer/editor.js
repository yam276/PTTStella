// Mature text/IME/history ownership; terminal ANSI interpretation stays in Rust.
import {EditorState,EditorSelection,StateEffect,StateField,Compartment} from '@codemirror/state';
import {EditorView,Decoration,WidgetType,drawSelection,rectangularSelection,keymap,highlightSpecialChars} from '@codemirror/view';
import {history,historyKeymap,defaultKeymap} from '@codemirror/commands';
const projection=StateEffect.define(), viewMode=StateEffect.define();
function color(s,palette){let fg=s.fg+(s.bright&&s.fg<8?8:0),bg=s.bg;if(s.inverse)[fg,bg]=[bg,fg];return [palette[fg],palette[bg]];}
export function create({parent,onChange,onContext,palette,label,graphicCode=()=>null,renderGlyph=()=>{}}){
    let mode='color',selectionMode='text',applying=false,view,disposed=false;
    const ownerWindow=parent.ownerDocument.defaultView;
    const columns=79,rows=23;
    let fontSize='',measurement=null;
    function fit(){
        if(disposed||!view||parent.clientWidth<=32||parent.clientHeight<=16)return;
        measurement??=parent.ownerDocument.createElement('canvas').getContext('2d');
        if(!measurement)return;
        measurement.font='20px '+ownerWindow.getComputedStyle(view.dom).fontFamily;
        const cell=measurement.measureText('0').width;
        if(!Number.isFinite(cell)||cell<=0)return;
        const gutter=view.scrollDOM.offsetWidth-view.scrollDOM.clientWidth;
        parent.style.setProperty('--draft-editor-gutter',gutter+'px');
        const scale=Math.min((parent.clientWidth-17-gutter)/(columns*cell),(parent.clientHeight-17)/(rows*cell*2));
        const next=Math.max(1,Math.floor(20*scale*100)/100)+'px';
        if(next!==fontSize){fontSize=next;parent.style.setProperty('--draft-editor-font-size',next);view.requestMeasure();}
    }
    class Graphic extends WidgetType {
        constructor(text,data){super();this.text=text;this.data=data;}
        eq(other){return this.text===other.text&&JSON.stringify(this.data.left)===JSON.stringify(other.data.left)&&JSON.stringify(this.data.right)===JSON.stringify(other.data.right);}
        toDOM(){const canvas=parent.ownerDocument.createElement('canvas');canvas.width=this.data.width*12;canvas.height=24;canvas.style.cssText='position:static;inset:auto;display:inline-block;width:'+this.data.width+'ch;height:2ch;vertical-align:bottom';canvas.setAttribute('aria-label',this.text);renderGlyph(canvas,this.text,this.data);return canvas;}
        ignoreEvent(){return false;}
    }
    const editable=new Compartment();
    const styles=StateField.define({
        create:()=>({mode,data:[],set:Decoration.none}),
        update(value,tr){
            let m=value.mode,data=value.data,set=value.set.map(tr.changes),replace=false;
            for(const e of tr.effects){if(e.is(projection)){data=e.value;replace=true;}if(e.is(viewMode)){m=e.value;replace=true;}}
            if(replace){const ranges=[];for(const d of data){if(d.to>tr.state.doc.length||d.from>=d.to)continue;
                if(d.hidden){if(m!=='source')ranges.push(Decoration.replace({stellaHidden:true}).range(d.from,d.to));}
                else {
                    const [fg,bg]=color(d.left,palette),[rf,rb]=color(d.right,palette);
                    // Rust supplies byte-cell styles, including the two sides of Chinese glyphs.
                    const css=fg===rf&&bg===rb?`color:${fg};background-color:${bg}`:`color:transparent;background-image:linear-gradient(90deg,${fg} 50%,${rf} 50%),linear-gradient(90deg,${bg} 50%,${rb} 50%);background-clip:text,border-box;-webkit-background-clip:text,border-box`;
                    if(m==='color'&&d.width!==2)ranges.push(Decoration.mark({attributes:{style:css}}).range(d.from,d.to));
                    let offset=d.from;for(const ch of tr.state.doc.sliceString(d.from,d.to)){
                        if(d.width===2)ranges.push(Decoration.mark({attributes:{style:(m==='color'?css+';':'')+'display:inline-block;width:2ch;text-align:center;vertical-align:bottom'}}).range(offset,offset+ch.length));
                        if(m==='color'&&graphicCode(ch)!==null)ranges.push(Decoration.replace({widget:new Graphic(ch,d)}).range(offset,offset+ch.length));offset+=ch.length;
                    }
                }
            }set=Decoration.set(ranges,true);}
            // Offsets in the last source projection are stale after an edit. The mapped
            // decoration set keeps typing stable; only a fresh revision can rebuild it.
            return {mode:m,data:tr.docChanged?[]:data,set};
        },
        provide:field=>[EditorView.decorations.from(field,v=>v.set),EditorView.atomicRanges.of(v=>v.state.field(field).set.update({filter:(_a,_b,d)=>d.spec.stellaHidden===true}))]
    });
    const lineSelection=EditorState.transactionFilter.of(tr=>{
        if(selectionMode!=='line'||!tr.selection||tr.docChanged||tr.selection.ranges.every(r=>r.empty))return tr;
        const ranges=tr.selection.ranges.map(r=>{if(r.empty)return r;
            const first=tr.newDoc.lineAt(r.from),last=tr.newDoc.lineAt(r.to>r.from&&tr.newDoc.lineAt(r.to).from===r.to?r.to-1:r.to);
            const end=Math.min(tr.newDoc.length,last.to+1);return r.anchor<=r.head?EditorSelection.range(first.from,end):EditorSelection.range(end,first.from);
        });
        const selection=EditorSelection.create(ranges,tr.selection.mainIndex);return selection.eq(tr.selection)?tr:[tr,{selection}];
    });
    const wholeLines=EditorView.mouseSelectionStyle.of((v,event)=>{
        if(selectionMode!=='line'||event.button!==0)return null;
        let anchor=v.posAtCoords({x:event.clientX,y:event.clientY});if(anchor===null)return null;
        return {get(cur){const pos=v.posAtCoords({x:cur.clientX,y:cur.clientY})??anchor;
            if(Math.abs(cur.clientX-event.clientX)<3&&Math.abs(cur.clientY-event.clientY)<3&&event.detail<2)return EditorSelection.single(anchor);
            const a=v.state.doc.lineAt(Math.min(anchor,pos)),b=v.state.doc.lineAt(Math.max(anchor,pos));
            const end=Math.min(v.state.doc.length,b.to+1);return EditorSelection.single(pos<anchor?end:a.from,pos<anchor?a.from:end);
        },update(u){anchor=u.changes.mapPos(anchor);}};
    });
    const extensions=[
        styles,EditorState.tabSize.of(8),editable.of([EditorView.editable.of(true),EditorState.readOnly.of(false)]),EditorState.allowMultipleSelections.of(true),
        lineSelection,wholeLines,rectangularSelection({eventFilter:e=>selectionMode==='rectangle'&&e.button===0}),drawSelection(),history(),
        keymap.of([...historyKeymap,...defaultKeymap]),highlightSpecialChars({specialChars:/[\u0000-\u001f\u007f]/g,render:(code,description,placeholder)=>{const span=parent.ownerDocument.createElement('span');span.textContent=code===27?'␛':placeholder;span.className='cm-specialChar';span.title=description??'U+'+code.toString(16);return span;}}),
        EditorView.contentAttributes.of({'aria-label':label,spellcheck:'false',autocapitalize:'off',autocorrect:'off'}),
        EditorView.updateListener.of(u=>{if(u.docChanged&&!applying&&!disposed)onChange();}),
        EditorView.domEventHandlers({contextmenu(e,v){if(!e.isTrusted)return false;e.preventDefault();onContext(e);return true;}}),
        EditorView.theme({'&':{width:`calc(${columns}ch + 16px + var(--draft-editor-gutter, 0px))`,height:`calc(${rows*2}ch + 16px)`,boxSizing:'border-box',fontFamily:'"PTTStella Cascadia Mono", "PTTStella Noto Sans CJK TC", monospace',fontSize:'var(--draft-editor-font-size, 20px)',color:'#c0c0c0',backgroundColor:'#000',outline:'1px solid #655633'},'.cm-scroller':{overflow:'auto',fontFamily:'inherit',fontSize:'inherit',lineHeight:'2ch',scrollbarGutter:'stable'},'.cm-content':{padding:'8px 0',minHeight:'100%',width:`calc(${columns}ch + 16px)`,flexGrow:'0',flexShrink:'0',boxSizing:'border-box'},'.cm-line':{padding:'0 8px'},'&.cm-focused':{outline:'1px solid #b99952'},'.cm-cursor':{borderLeftColor:'#f1cb70'},'.cm-selectionLayer':{zIndex:'3 !important',opacity:'.45',pointerEvents:'none'},'.cm-cursorLayer':{zIndex:'4'},'.cm-selectionBackground, &.cm-focused .cm-selectionBackground':{background:'#6795c0 !important'},'.cm-specialChar':{color:'#f1cb70'}})
    ];
    view=new EditorView({parent,state:EditorState.create({doc:'',extensions})});
    const observer=ownerWindow.ResizeObserver?new ownerWindow.ResizeObserver(fit):null;
    observer?.observe(parent);if(!observer)ownerWindow.addEventListener('resize',fit);
    fit();parent.ownerDocument.fonts?.ready.then(()=>{if(!disposed){fit();view.requestMeasure();}});
    return {
        value:()=>view.state.doc.toString(),
        load(value){applying=true;view.setState(EditorState.create({doc:value,extensions}));applying=false;},
        ranges:()=>view.state.selection.ranges.filter(r=>!r.empty).map(r=>({from:r.from,to:r.to})).sort((a,b)=>a.from-b.from),
        apply(changes){view.dispatch({changes,userEvent:'input.color'});},
        project(data){view.dispatch({effects:projection.of(data)});},
        mode(m){mode=m;view.dispatch({effects:viewMode.of(m)});},
        selection(m){selectionMode=m;view.dispatch({selection:view.state.selection});},
        readOnly(value){view.dispatch({effects:editable.reconfigure([EditorView.editable.of(!value),EditorState.readOnly.of(value)])});},
        composing:()=>view.composing,
        focus(){fit();view.requestMeasure();view.focus();},
        destroy(){disposed=true;observer?.disconnect();if(!observer)ownerWindow.removeEventListener('resize',fit);measurement=null;parent.style.removeProperty('--draft-editor-font-size');parent.style.removeProperty('--draft-editor-gutter');view.destroy();},
        get view(){return view;}
    };
}
