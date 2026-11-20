// The measurement behind ADR 1154: does Chrome read an inline handler after a solidus, in upper case, after a quoted value, and after a comment ended by `--!>`
// as the handler it is after a space? Each page is loaded as the HTML text it is, the handler sets `window.hits`, and the page is driven (focus, click) so that the
// handler's event happens.
//
//   node fixtures/reactive-ir/product-realm-page-handler-solidus/html-tokenizer-probe.mjs      (the pinned headless shell; seconds)
//
// Measured with the headless shell 1223 (Chrome 148.0.7778.96): every spelling below runs its handler, the same as the white-space control `ctl`.
import { createRequire } from "node:module";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
const ROOT = join(dirname(fileURLToPath(import.meta.url)), "../../..");
const require = createRequire(`${ROOT}/scripts/runtime-oracle-install/`);
const { chromium } = require("playwright-core");
const { pinnedBrowser } = await import(`${ROOT}/scripts/playwright-browser.mjs`);
const record = `(window.hits = (window.hits || [])).push(`;
const page = (body) => `<!doctype html><html><head><meta charset="utf-8"></head>${body}</html>`;
const cases = {
  // The handler of `body`, on window load.
  ctl: page(`<body onload="${record}'load')"><div id="root"></div></body>`),
  "body-solidus": page(`<body/onload="${record}'load')"><div id="root"></div></body>`),
  "body-upper": page(`<BODY/ONLOAD="${record}'load')"><div id="root"></div></BODY>`),
  "body-spaced-solidus": page(`<body / onload="${record}'load')"><div id="root"></div></body>`),
  "body-comment-bang": page(`<!-- x --!><body onload="${record}'load')"><div id="root"></div></body> -->`),
  // The handler of an element, on a click.
  "click-ctl": page(`<body><div id="root" onclick="${record}'click')"></div></body>`),
  "click-after-value": page(`<body><div id="root"/onclick="${record}'click')"></div></body>`),
  "click-after-value-upper": page(`<body><div id="root"/ONCLICK="${record}'click')"></div></body>`),
  "click-after-value-adjacent": page(`<body><div id="root"onclick="${record}'click')"></div></body>`),
  // The handler of the focus move, `focusin`, on the body.
  "focusin-ctl": page(`<body onfocusin="${record}'focusin')"><input id="a"></body>`),
  "focusin-solidus": page(`<body/onfocusin="${record}'focusin')"><input id="a"></body>`),
  "focusin-upper": page(`<body/ONFOCUSIN="${record}'focusin')"><input id="a"></body>`),
};
const browser = await chromium.launch({ executablePath: pinnedBrowser().executable });
const out = {};
for (const [name, html] of Object.entries(cases)) {
  const tab = await browser.newPage();
  await tab.setContent(html);
  await tab.evaluate(() => {
    document.getElementById("root")?.click();
    document.getElementById("a")?.focus();
  });
  out[name] = await tab.evaluate(() => window.hits ?? []);
  await tab.close();
}
await browser.close();
console.log(JSON.stringify(out, null, 1));
const expected = { load: ["body-solidus", "body-upper", "body-spaced-solidus", "body-comment-bang"], click: ["click-after-value", "click-after-value-upper", "click-after-value-adjacent"], focusin: ["focusin-solidus", "focusin-upper"] };
const control = { load: "ctl", click: "click-ctl", focusin: "focusin-ctl" };
let ok = true;
for (const [event, names] of Object.entries(expected)) {
  for (const name of [control[event], ...names]) {
    const ran = out[name].includes(event);
    if (!ran) { ok = false; console.error(`${name}: the ${event} handler did not run`); }
  }
}
console.log(ok ? "every spelling runs its handler" : "a spelling's handler did not run");
process.exitCode = ok ? 0 : 1;
