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
