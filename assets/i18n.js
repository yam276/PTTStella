// Official Fluent runtime, with atomic replacement of validated embedded bundles.
function createSurfaceI18n(packet, runtime) {
    function load(packet) {
        if(packet?.kind!=="localization"||!packet.payload||!["zh-TW","en-US"].includes(packet.payload.language))throw new Error("Invalid localization bootstrap");
        const {language,resource,fallback}=packet.payload;
        const make=(locale,source)=>{
            if(typeof source!=="string"||source.length>262144)throw new Error("Invalid Fluent resource");
            const bundle=new runtime.FluentBundle(locale,{useIsolating:false});
            const errors=bundle.addResource(new runtime.FluentResource(source));
            if(errors.length)throw new Error("Invalid embedded Fluent catalog: "+errors.join("; "));
            return bundle;
        };
        return [make(language,resource),make("zh-TW",fallback)];
    }
    let bundles=load(packet);
    const translate=(id,args)=>{
        for(const bundle of bundles){
            const message=bundle.getMessage(id);
            if(!message?.value)continue;
            const errors=[];const value=bundle.formatPattern(message.value,args,errors);
            if(errors.length)throw new Error("Fluent formatting failed: "+id+": "+errors.join("; "));
            return value;
        }
        throw new Error("Missing Fluent message: "+id);
    };
    translate.update=packet=>{const next=load(packet);bundles=next;};
    return translate;
}
