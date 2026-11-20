// Chrome evidence for ADR 1148 (the change-delivery model): when does the browser deliver `change` to a listener, and from where?
// Launches the pinned headless shell (the runtime oracle's browser), runs each scenario in a fresh page, records every click/input/change/focus/blur event
// at the element with { type, trusted, sync (inside a script call of the scenario), winEvent (window.event === the event) }.
// usage: node scripts/fuzz/change-delivery-probe.mjs [out.json]   (the table in docs/decisions.md, ADR 1148, is this script's output)
import { writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
const root = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const require = createRequire(`${root}/scripts/runtime-oracle-install/package.json`);
const { chromium } = require("playwright");
const { pinnedBrowser } = await import(`${root}/scripts/playwright-browser.mjs`);
const pin = pinnedBrowser();
const browser = await chromium.launch({ executablePath: pin.executable, headless: true });

const RECORDER = `
  window.__log = [];
  window.__sync = false;
  window.__watch = (el, label) => {
    for (const type of ["click", "input", "change", "focus", "blur", "focusin", "focusout"]) {
      el.addEventListener(type, (e) => window.__log.push({ el: label, type, trusted: e.isTrusted, sync: window.__sync, winEvent: window.event === e }));
    }
  };
  window.__call = (fn) => { window.__sync = true; try { return fn(); } finally { window.__sync = false; } };
`;

const results = [];
async function scenario(group, name, html, steps) {
  const page = await browser.newPage();
  const errors = [];
  page.on("pageerror", (error) => errors.push(String(error)));
  await page.setContent(`<!doctype html><html><body>${html}</body></html>`);
  await page.evaluate(RECORDER);
  try {
    await steps(page);
  } catch (error) {
    errors.push(`scenario error: ${error.message.split("\n")[0]}`);
  }
  const log = await page.evaluate(() => window.__log);
  const summary = log.map((e) => `${e.el}:${e.type}${e.trusted ? "" : "(untrusted)"}${e.sync ? "[in-call]" : ""}${e.winEvent ? "" : "[no window.event]"}`);
  results.push({ group, name, events: summary, errors });
  await page.close();
}
const watch = (page, selector, label) => page.evaluate(([s, l]) => window.__watch(document.querySelector(s), l), [selector, label]);
const call = (page, source) => page.evaluate(`window.__call(() => { ${source} })`);

// ---- checkbox
const CHECKBOX = `<form id="f"><input id="c" type="checkbox"></form><label id="l" for="c">label</label><button id="other">other</button>`;
const setup = async (page) => watch(page, "#c", "c");
await scenario("checkbox", "user click", CHECKBOX, async (page) => { await setup(page); await page.click("#c"); });
await scenario("checkbox", "keyboard Space", CHECKBOX, async (page) => { await setup(page); await page.focus("#c"); await page.keyboard.press("Space"); });
await scenario("checkbox", "user click on the label", CHECKBOX, async (page) => { await setup(page); await page.click("#l"); });
await scenario("checkbox", "el.click()", CHECKBOX, async (page) => { await setup(page); await call(page, "document.getElementById('c').click()"); });
await scenario("checkbox", "label.click()", CHECKBOX, async (page) => { await setup(page); await call(page, "document.getElementById('l').click()"); });
await scenario("checkbox", "dispatchEvent(MouseEvent click)", CHECKBOX, async (page) => { await setup(page); await call(page, "document.getElementById('c').dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true }))"); });
await scenario("checkbox", "dispatchEvent(Event change)", CHECKBOX, async (page) => { await setup(page); await call(page, "document.getElementById('c').dispatchEvent(new Event('change', { bubbles: true }))"); });
await scenario("checkbox", "dispatchEvent(Event input)", CHECKBOX, async (page) => { await setup(page); await call(page, "document.getElementById('c').dispatchEvent(new Event('input', { bubbles: true }))"); });
await scenario("checkbox", "checked = true", CHECKBOX, async (page) => { await setup(page); await call(page, "document.getElementById('c').checked = true"); });
await scenario("checkbox", "form.reset() after a user click", CHECKBOX, async (page) => { await setup(page); await page.click("#c"); await call(page, "document.getElementById('f').reset()"); });
await scenario("checkbox", "user click, then blur() while checked", CHECKBOX, async (page) => { await setup(page); await page.click("#c"); await call(page, "document.getElementById('c').blur()"); });
await scenario("checkbox", "user click, then focus() elsewhere", CHECKBOX, async (page) => { await setup(page); await page.click("#c"); await call(page, "document.getElementById('other').focus()"); });
await scenario("checkbox", "user click, then remove() while focused", CHECKBOX, async (page) => { await setup(page); await page.click("#c"); await call(page, "document.getElementById('c').remove()"); });
await scenario("checkbox", "user click, then disabled = true while focused", CHECKBOX, async (page) => { await setup(page); await page.click("#c"); await call(page, "document.getElementById('c').disabled = true"); });
await scenario("checkbox", "disabled: el.click()", CHECKBOX.replace('type="checkbox"', 'type="checkbox" disabled'), async (page) => { await setup(page); await call(page, "document.getElementById('c').click()"); });
await scenario("checkbox", "disabled: dispatchEvent(Event change)", CHECKBOX.replace('type="checkbox"', 'type="checkbox" disabled'), async (page) => { await setup(page); await call(page, "document.getElementById('c').dispatchEvent(new Event('change', { bubbles: true }))"); });
await scenario("checkbox", "ancestor form listener sees the change (bubbling)", CHECKBOX, async (page) => {
  await page.evaluate(() => window.__watch(document.getElementById("f"), "form"));
  await page.click("#c");
});

// ---- retyping: the hazard a type write opens
await scenario("retyped", "type = 'text' by a write, then a user edit and Tab", CHECKBOX, async (page) => {
  await setup(page);
  await call(page, "document.getElementById('c').type = 'text'");
  await page.click("#c");
  await page.keyboard.type("abc");
  await page.keyboard.press("Tab");
});
await scenario("retyped", "setAttribute('type','text'), a user edit, then remove() while focused", CHECKBOX, async (page) => {
  await setup(page);
  await call(page, "document.getElementById('c').setAttribute('type', 'text')");
  await page.click("#c");
  await page.keyboard.type("abc");
  await call(page, "document.getElementById('c').remove()");
});
await scenario("retyped", "removeAttribute('type') (the default is text), a user edit, then focus() elsewhere", CHECKBOX, async (page) => {
  await setup(page);
  await call(page, "document.getElementById('c').removeAttribute('type')");
  await page.click("#c");
  await page.keyboard.type("abc");
  await call(page, "document.getElementById('other').focus()");
});

// ---- radio
const RADIO = `<form id="f"><input id="r1" type="radio" name="g" checked><input id="r2" type="radio" name="g"></form>`;
const radios = async (page) => { await watch(page, "#r1", "r1"); await watch(page, "#r2", "r2"); };
await scenario("radio", "user click on the unchecked radio", RADIO, async (page) => { await radios(page); await page.click("#r2"); });
await scenario("radio", "ArrowRight from the checked radio", RADIO, async (page) => { await radios(page); await page.focus("#r1"); await page.keyboard.press("ArrowRight"); });
await scenario("radio", "r2.click()", RADIO, async (page) => { await radios(page); await call(page, "document.getElementById('r2').click()"); });
await scenario("radio", "r1.click() (already checked)", RADIO, async (page) => { await radios(page); await call(page, "document.getElementById('r1').click()"); });
await scenario("radio", "dispatchEvent(MouseEvent click) on r2", RADIO, async (page) => { await radios(page); await call(page, "document.getElementById('r2').dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true }))"); });
await scenario("radio", "r2.checked = true", RADIO, async (page) => { await radios(page); await call(page, "document.getElementById('r2').checked = true"); });
await scenario("radio", "form.reset() after a user click", RADIO, async (page) => { await radios(page); await page.click("#r2"); await call(page, "document.getElementById('f').reset()"); });

// ---- text input: the delivery by state change that no call names
const TEXT = `<form id="f"><input id="t" type="text"></form><button id="other">other</button>`;
const text = async (page) => watch(page, "#t", "t");
await scenario("text", "user types, then Tab", TEXT, async (page) => { await text(page); await page.click("#t"); await page.keyboard.type("abc"); await page.keyboard.press("Tab"); });
await scenario("text", "user types, then Enter", TEXT, async (page) => { await text(page); await page.click("#t"); await page.keyboard.type("abc"); await page.keyboard.press("Enter"); });
await scenario("text", "value = 'x' by a write, then blur()", TEXT, async (page) => { await text(page); await page.click("#t"); await call(page, "document.getElementById('t').value = 'x'"); await call(page, "document.getElementById('t').blur()"); });
await scenario("text", "user types, then blur()", TEXT, async (page) => { await text(page); await page.click("#t"); await page.keyboard.type("abc"); await call(page, "document.getElementById('t').blur()"); });
await scenario("text", "user types, then focus() elsewhere", TEXT, async (page) => { await text(page); await page.click("#t"); await page.keyboard.type("abc"); await call(page, "document.getElementById('other').focus()"); });
await scenario("text", "user types, then remove() while focused", TEXT, async (page) => { await text(page); await page.click("#t"); await page.keyboard.type("abc"); await call(page, "document.getElementById('t').remove()"); });
await scenario("text", "user types, then disabled = true while focused", TEXT, async (page) => { await text(page); await page.click("#t"); await page.keyboard.type("abc"); await call(page, "document.getElementById('t').disabled = true"); });
await scenario("text", "user types, then replaceWith() while focused", TEXT, async (page) => { await text(page); await page.click("#t"); await page.keyboard.type("abc"); await call(page, "document.getElementById('t').replaceWith(document.createElement('span'))"); });
await scenario("text", "user types, then the parent is removed while focused", TEXT, async (page) => { await text(page); await page.click("#t"); await page.keyboard.type("abc"); await call(page, "document.getElementById('f').remove()"); });
await scenario("text", "dispatchEvent(Event change)", TEXT, async (page) => { await text(page); await call(page, "document.getElementById('t').dispatchEvent(new Event('change', { bubbles: true }))"); });
await scenario("text", "form.reset() after a user edit", TEXT, async (page) => { await text(page); await page.click("#t"); await page.keyboard.type("abc"); await call(page, "document.getElementById('f').reset()"); });

// ---- select (the other control whose change a user makes)
const SELECT = `<select id="s"><option value="a">a</option><option value="b">b</option></select>`;
await scenario("select", "ArrowDown on the focused select", SELECT, async (page) => { await watch(page, "#s", "s"); await page.focus("#s"); await page.keyboard.press("ArrowDown"); });
await scenario("select", "value = 'b' by a write", SELECT, async (page) => { await watch(page, "#s", "s"); await call(page, "document.getElementById('s').value = 'b'"); });

const report = { chrome: pin.browserVersion, headlessShell: pin.revision, playwright: pin.playwright, results };
await browser.close();
const out = process.argv[2];
if (out) writeFileSync(out, JSON.stringify(report, null, 1));
console.log(`Chrome ${pin.browserVersion} (headless shell ${pin.revision}), Playwright ${pin.playwright}`);
let group = "";
for (const row of results) {
  if (row.group !== group) { group = row.group; console.log(`\n## ${group}`); }
  console.log(`- ${row.name}: ${row.events.length ? row.events.join(" ") : "(no event)"}${row.errors.length ? `  !! ${row.errors.join("; ")}` : ""}`);
}
