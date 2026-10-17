// Production Fluent runtime and shared catalogs; no browser or network required.
import test from "node:test";
import assert from "node:assert/strict";
import {readFileSync} from "node:fs";
import {translator,create,runtime as exports} from "./i18n_test_support.mjs";
test("official Fluent formats independent Chinese and English catalogs and plurals",()=>{
    const zh=translator("zh-TW"),en=translator("en-US");
    assert.equal(zh("connect"),"連線");assert.equal(en("connect"),"Connect");
    assert.equal(en("filters-count-hidden",{count:1}),"One reply hidden");
    assert.equal(en("filters-count-hidden",{count:2}),"2 replies hidden");
    assert.equal(zh("filters-count-hidden",{count:2}),"已遮黑 2 則推文");
    assert.equal(en("export-lines-hint",{start:3,end:5}),"Export lines 3–5 (all 80 columns)");
    const unsafe="<img src=x onerror=evil()> & example";
    assert.equal(en("link-hover-hint",{url:unsafe}),unsafe+" — click to open; right-click to select and copy");
    assert.equal(zh("connect"),"連線");
});
test("bootstrap, fallback and formatting errors have explicit boundaries",()=>{
    assert.throws(()=>create(null,exports),/bootstrap/);
    assert.throws(()=>create({kind:"localization",payload:{language:"unknown"}},exports),/bootstrap/);
    const tr=create({kind:"localization",payload:{language:"en-US",resource:"hello = Hello",fallback:"fallback = 後備"}},exports);
    assert.equal(tr("fallback"),"後備");
    assert.throws(()=>tr("missing"),/Missing Fluent/);
    assert.throws(()=>translator("en-US")("export-lines-hint",{start:1}),/formatting failed/);
});

test("the independent Rust/JS bootstrap fixture retains the tagged envelope",()=>{
    const packet=JSON.parse(readFileSync(new URL("../tests/fixtures/localization.json",import.meta.url),"utf8"));
    assert.equal(create(packet,exports)("connect"),"Connect");
    packet.payload.language="zh-TW";packet.payload.resource=packet.payload.fallback;
    assert.equal(create(packet,exports)("connect"),"連線");
});
