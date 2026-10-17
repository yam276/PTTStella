// Production font policy with explicit FontFace/FontFaceSet doubles, not native glyph proof.
import test from "node:test";
import assert from "node:assert/strict";
import vm from "node:vm";
import {readFileSync} from "node:fs";
import {translator} from "./i18n_test_support.mjs";
function environment(language="en-US"){
    const attempts=[],registered=[],available=new Set();
    class Face {constructor(family,source){this.family=family;this.source=source;}
        async load(){attempts.push([this.family,this.source]);if(this.source.startsWith("local(")&&!available.has(this.source))throw new Error("missing");return this;}}
    const fonts={add:face=>registered.push(face.family),load:async()=>[],ready:Promise.resolve()};
    const create=vm.runInNewContext(readFileSync(new URL("../assets/surface-fonts.js",import.meta.url),"utf8")+"\ncreateSurfaceFonts",{URL});
    return {font:create({document:{baseURI:"http://dioxus.local/",fonts},FontFace:Face,tr:translator(language)}),attempts,registered,available};
}
const windows={ascii_font:"Cascadia Mono",cjk_font:"Microsoft JhengHei",font_size:20};
test("CJK retains its chosen face then uses selected Latin symbols before OS fallback",()=>{
    const {font}=environment();
    assert.equal(font.description(windows),'20px "PTTStella Cascadia Mono", monospace');
    assert.equal(font.description(windows,true),'20px "Microsoft JhengHei", "PTTStella Cascadia Mono", monospace');
    assert.equal(font.description({...windows,ascii_font:"Menlo",cjk_font:"Songti TC"},true,2),'40px "Songti TC", "Menlo", monospace');
    assert.equal(font.family("Menlo","Menlo"),'"Menlo", monospace');
    assert.equal(font.family('A "quoted" font'),JSON.stringify('A "quoted" font')+", monospace");
});
test("bundled Latin is registered independently and missing CJK remains a warning in both languages",async()=>{
    for(const language of ["zh-TW","en-US"]){
        const {font,registered,attempts,available}=environment(language);
        const warning=await font.load(windows);assert.match(warning,/Microsoft JhengHei/);assert.doesNotMatch(warning,/Cascadia Mono/);
        assert.deepEqual(registered,["PTTStella Cascadia Mono"]);
        assert.equal(font.note(windows,warning),warning);
        available.add('local("Microsoft JhengHei")');assert.equal(await font.load(windows),"");
        const loaded=attempts.length;await font.load(windows);assert.equal(attempts.length,loaded);
        font.invalidate();await font.load(windows);assert.equal(attempts.length,loaded+1);
        assert.equal(registered.length,1);
    }
});
