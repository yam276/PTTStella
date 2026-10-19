// The tagged envelope has one destination. Unknown kinds fail visibly instead
// of falling through to paint and partially mutating an unrelated feature.
function revokeSurfaceSelection(send,cache,busy) {
    const selected=cache??busy;
    if(selected)send({kind:"selection_cleared",presentation:selected.presentation,generation:selected.generation,
        revision:selected.revision,request:selected.request});
}
function createSurfaceRouter(handlers) {
    const kinds=["station_mouse_rejected","paint","notice","surface_probe","filters_open","filters_result","login_result","keyboard_state",
        "hover_settings","tools_opacity","sites_open","sites_result","media_hover","hover_reset","hover_rejected","media_open","media_image","media_error",
        "media_external","media_reset","settings_open","font_catalog","font_preview","profile_result","settings_saved",
        "selection","paste_result","input_rejected","navigation_result","destination_result","recent_boards","bookmarks_open","bookmarks_result","link_result","export_result"];
    const table=new Map(kinds.map(kind=>{
        if(typeof handlers[kind]!=="function")throw new Error("Missing surface handler: "+kind);
        return [kind,handlers[kind]];
    }));
    return packet=>{
        if(!packet||typeof packet!=="object"||Array.isArray(packet)||!table.has(packet.kind)
            ||Object.keys(packet).some(key=>key!=="kind"&&key!=="payload"))throw new Error("Unknown surface packet");
        if(packet.kind!=="media_reset"&&!Object.hasOwn(packet,"payload"))throw new Error("Missing surface payload: "+packet.kind);
        return table.get(packet.kind)(packet.payload);
    };
}
