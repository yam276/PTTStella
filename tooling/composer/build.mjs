// Runtime is checked in so Cargo builds need no npm or sibling checkout.
import {build} from 'esbuild';
import {readFileSync,writeFileSync,readdirSync,mkdirSync} from 'node:fs';
import {createHash} from 'node:crypto';
import {fileURLToPath} from 'node:url';
import path from 'node:path';
const root=path.dirname(fileURLToPath(import.meta.url)),out=path.resolve(root,'../../assets/vendor/composer');
mkdirSync(out,{recursive:true});
await build({entryPoints:[path.join(root,'editor.js')],bundle:true,format:'iife',globalName:'StellaComposer',minify:true,target:'es2020',legalComments:'eof',outfile:path.join(out,'editor.js')});
const lock=JSON.parse(readFileSync(path.join(root,'package-lock.json'),'utf8')),packages=[],licenses=[];
for(const [name,pkg]of Object.entries(lock.packages)){
 if(!name.startsWith('node_modules/')||pkg.dev)continue;
 const dir=path.join(root,name),file=readdirSync(dir).filter(n=>/^license/i.test(n)).sort()[0];if(!file)throw new Error('Missing license: '+name);
 const id=name.replace(/^node_modules\//,'');packages.push({name:id,version:pkg.version,url:pkg.resolved,integrity:pkg.integrity,license:pkg.license});licenses.push(`\n=== ${id} ${pkg.version} (${pkg.license}) ===\n`+readFileSync(path.join(dir,file),'utf8'));
}
const hash=p=>createHash('sha256').update(readFileSync(p)).digest('hex');
writeFileSync(path.join(out,'LICENSES.txt'),'PTTStella local composer — third-party licenses\n'+licenses.join(''));
writeFileSync(path.join(out,'source.json'),JSON.stringify({source:'tooling/composer/editor.js',build:'npm ci --ignore-scripts --prefix tooling/composer && npm run build --prefix tooling/composer',source_sha256:hash(path.join(root,'editor.js')),lock_sha256:hash(path.join(root,'package-lock.json')),bundle_sha256:hash(path.join(out,'editor.js')),packages},null,2)+'\n');
