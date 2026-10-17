import {readFileSync} from "node:fs";
import vm from "node:vm";
const read=path=>readFileSync(new URL("../"+path,import.meta.url),"utf8");
export const runtime={};
const exports=runtime;
vm.runInNewContext(read("assets/vendor/fluent/index.js"),{exports,module:{exports},Intl});
export const create=vm.runInNewContext(read("assets/i18n.js")+"\ncreateSurfaceI18n");
const resources=Object.fromEntries(["zh-TW","en-US"].map(language=>[language,read(`locales/${language}/pttstella.ftl`)]));
export const translator=language=>create({kind:"localization",payload:{language,resource:resources[language],fallback:resources["zh-TW"]}},exports);
