// Contrast checks use the production palette; native rendering is verified separately.
import {readFileSync} from 'node:fs';
import test from 'node:test';
import assert from 'node:assert/strict';

const css=readFileSync(new URL('../assets/app.css',import.meta.url),'utf8');
const palette=Object.fromEntries([...css.matchAll(/(--ui-[\w-]+):\s*(#[\da-f]{6})\s*;/gi)].map(m=>[m[1],m[2]]));
function luminance(hex){
    const rgb=[1,3,5].map(i=>Number.parseInt(hex.slice(i,i+2),16)/255).map(v=>v<=.04045?v/12.92:((v+.055)/1.055)**2.4);
    return .2126*rgb[0]+.7152*rgb[1]+.0722*rgb[2];
}
function contrast(a,b){const [dark,light]=[luminance(a),luminance(b)].sort((a,b)=>a-b);return (light+.05)/(dark+.05);}

test('gold theme controls and primary actions retain readable normal text contrast',()=>{
    for(const [foreground,background] of [
        ['--ui-text','--ui-control'],['--ui-text','--ui-control-hover'],
        ['--ui-muted','--ui-panel'],['--ui-gold','--ui-control'],
        ['--ui-on-gold','--ui-gold'],['--ui-on-gold','--ui-gold-hover'],
    ])assert.ok(contrast(palette[foreground],palette[background])>=4.5,`${foreground} on ${background}`);
    assert.ok(contrast(palette['--ui-focus'],palette['--ui-panel'])>=3);
    for(const ref of css.matchAll(/var\((--ui-[\w-]+)\)/g))assert.ok(palette[ref[1]],`undefined palette token ${ref[1]}`);
});

test('the flat constellation artwork has visible contrast on the floating button',()=>{
    const svg=readFileSync(new URL('../assets/icons/constellation-flat.svg',import.meta.url),'utf8');
    const colors=[...svg.matchAll(/(?:fill|stroke)="(#[\da-f]{6})"/gi)].map(m=>m[1]);
    assert.ok(colors.length>0);
    for(const color of colors)for(const background of ['--ui-panel','--ui-control-hover'])assert.ok(contrast(color,palette[background])>=3);
});
