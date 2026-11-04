import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import vm from 'node:vm';
const source=readFileSync(new URL('../assets/surface-renderer.js',import.meta.url),'utf8');
const render=vm.runInNewContext(source+'\nrender',{fontDescription:()=>''});
const bg={fg:6,bg:6,bright:false,inverse:false};
// Independent integer-pixel rectangle consumer: a fully cyan 80-column strip
// has no black pixels, regardless of which article row rounded up or down.
function raster(width,height,rows=1,blocks=false){
    const pixels=new Uint8Array(width*height);
    let ink=false;
    const ctx={set fillStyle(v){ink=v==='#008080';},setTransform(){},
        fillRect(x,y,w,h){for(let row=y;row<y+h;row++)pixels.fill(ink?1:0,row*width+x,row*width+x+w);},
        measureText(){return {width:12};},save(){},restore(){},beginPath(){},rect(){},clip(){}};
    const glyphs=Array.from({length:rows*80},(_,i)=>({row:Math.floor(i/80),col:i%80,width:1,text:blocks?'█':' ',left:blocks?{...bg,bg:0}:bg,right:blocks?{...bg,bg:0}:bg}));
    const geometry=render({width,height,getContext:()=>ctx},{frame:{cols:80,rows,glyphs,cursor_visible:false},settings:{baseline:20},links:[]},{cursor:false});
    assert.equal(geometry.x(80),width);assert.equal(geometry.y(rows),height);
    assert.equal(pixels.includes(0),false,`${width}x${height}/${rows} rows: black seam`);
}
test('rounded article strips fill every last pixel at fractional zoom and DPR',()=>{
    for(const width of [961,1247,1561,1887,1919,2401]){
        for(const height of [Math.floor(width/40),Math.ceil(width/40)]){
            raster(width,height);raster(width,height,1,true);
        }
        raster(width,Math.floor(width*24/40),24);
    }
});

test('text width changes ink within fixed columns while height, dual clips and ANSI geometry remain stable',()=>{
 const style={fg:7,bg:0,bright:false,inverse:false};
 function inspect(text,width,percent){
  const calls={scales:[],origins:[],clips:[],fills:[]};
  const ctx={setTransform(){},fillRect(...v){calls.fills.push(v);},measureText:()=>({width:20}),save(){},restore(){},beginPath(){},rect(...v){calls.clips.push(v);},clip(){},moveTo(){},lineTo(){},closePath(){},fill(){},translate(...v){calls.origins.push(v);},scale(...v){calls.scales.push(v);},fillText(){}};
  const geometry=render({width:960,height:24,getContext:()=>ctx},{frame:{cols:80,rows:1,cursor_visible:false,glyphs:[{text,width,row:0,col:2,left:style,right:{...style,fg:1}}]},settings:{baseline:20,...(percent===undefined?{}:{font_width:percent})},links:[]},{cursor:false});
  return {...calls,last:geometry.x(80)};
 }
 const normal=inspect('中',2,100),legacy=inspect('中',2,undefined),wide=inspect('中',2,120),narrow=inspect('中',2,80),limited=inspect('中',2,150);
 assert.deepEqual(normal,legacy);assert.deepEqual(normal.scales,[[1,1],[1,1]]);
 assert.deepEqual(wide.scales,[[1.2,1],[1.2,1]]);assert.deepEqual(wide.origins,[[24,20],[24,20]]);
 assert.deepEqual(narrow.scales,[[.8,1],[.8,1]]);assert.deepEqual(narrow.origins,[[28,20],[28,20]]);
 assert.deepEqual(limited.scales,wide.scales);assert.deepEqual(wide.clips,[[24,0,12,24],[36,0,12,24]]);
 assert.equal(wide.last,960);assert.deepEqual(wide.fills,normal.fills);
 for(const text of ['─','█','◢'])assert.deepEqual(inspect(text,2,150),inspect(text,2,100));
});
