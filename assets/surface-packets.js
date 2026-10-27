// The tagged envelope has one destination. Unknown kinds fail visibly instead
// of falling through to paint and partially mutating an unrelated feature.
function decodeSurfaceEnvelope(packet){
    if(typeof packet!=="string")return packet;
    if(packet.length>64*1024*1024)throw new Error("Surface packet exceeds byte limit");
    return JSON.parse(packet);
}
function revokeSurfaceSelection(send,cache,busy) {
    const selected=cache??busy;
    if(selected)send({kind:"selection_cleared",presentation:selected.presentation,generation:selected.generation,
        revision:selected.revision,request:selected.request});
}
function expandSurfaceFrame(frame) {
    if(!frame||frame.glyph_encoding===undefined)return frame;
    if(frame.glyph_encoding!=="runs_v1"||Object.hasOwn(frame,"glyphs")||!Array.isArray(frame.runs)
        ||!Number.isSafeInteger(frame.rows)||frame.rows<1||frame.rows>8192
        ||!Number.isSafeInteger(frame.cols)||frame.cols<1||frame.cols>8192)throw new Error("Invalid compact frame");
    const glyphs=[],styles=new Map();
    function unpack(value){
        if(!Number.isInteger(value)||value<0||value>262143)throw new Error("Invalid compact style");
        if(!styles.has(value))styles.set(value,{fg:value&255,bg:(value>>>8)&255,bright:!!(value&65536),inverse:!!(value&131072)});
        return styles.get(value);
    }
    for(const run of frame.runs){
        if(!Array.isArray(run)||run.length!==7)throw new Error("Invalid glyph run");
        const [row,col,width,a,b,unmapped,texts]=run;
        if(!Number.isSafeInteger(row)||row<0||row>=frame.rows||!Number.isSafeInteger(col)||col<0
            ||![1,2].includes(width)||typeof unmapped!=="boolean"||!Array.isArray(texts)||!texts.length
            ||col+texts.length*width>frame.cols||glyphs.length+texts.length>1048576)throw new Error("Invalid glyph run bounds");
        const left=unpack(a),right=unpack(b);
        for(let index=0;index<texts.length;index++){
            const text=texts[index];if(typeof text!=="string")throw new Error("Invalid glyph text");
            glyphs.push({row,col:col+index*width,width,text,left,right,unmapped});
        }
    }
    const {glyph_encoding,runs,...metadata}=frame;
    return {...metadata,glyphs};
}
function expandSurfacePaint(paint){
    if(!paint)return paint;
    // Decode fully before invoking any handler; failed/partial packets never
    // publish a presentation or produce an ACK. Rust retains export ownership.
    return {...paint,frame:expandSurfaceFrame(paint.frame),source_frame:expandSurfaceFrame(paint.source_frame)};
}
function createSurfaceRouter(handlers) {
    const kinds=["localization","diagnostics","station_mouse_rejected","paint","notice","surface_probe","filters_open","filters_result","login_result","keyboard_state",
        "hover_settings","tools_opacity","tools_access","sites_open","sites_result","media_hover","hover_reset","hover_rejected","media_open","media_image","media_error",
        "media_external","media_reset","reading_move","inline_open","inline_image","inline_error","inline_reset","settings_visibility","settings_close","settings_open","font_catalog","font_preview","profile_result","settings_saved",
        "selection","paste_result","input_rejected","navigation_result","destination_result","recent_boards","bookmarks_open","bookmarks_result","link_result","export_result"];
    const table=new Map(kinds.map(kind=>{
        if(typeof handlers[kind]!=="function")throw new Error("Missing surface handler: "+kind);
        return [kind,handlers[kind]];
    }));
    return packet=>{
        packet=decodeSurfaceEnvelope(packet);
        if(!packet||typeof packet!=="object"||Array.isArray(packet)||!table.has(packet.kind)
            ||Object.keys(packet).some(key=>key!=="kind"&&key!=="payload"))throw new Error("Unknown surface packet");
        if(packet.kind!=="media_reset"&&!Object.hasOwn(packet,"payload"))throw new Error("Missing surface payload: "+packet.kind);
        let data=packet.payload;
        if(packet.kind==="paint")data=expandSurfacePaint(data);
        else if(packet.kind==="settings_open")data={...data,sample:expandSurfacePaint(data.sample)};
        else if(packet.kind==="font_preview"&&data.packet)data={...data,packet:expandSurfacePaint(data.packet)};
        else if(packet.kind==="profile_result"&&data.profile)data={...data,profile:{...data.profile,sample:expandSurfacePaint(data.profile.sample)}};
        return table.get(packet.kind)(data);
    };
}
