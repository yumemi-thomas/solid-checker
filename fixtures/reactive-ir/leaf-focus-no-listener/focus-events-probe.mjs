// The measurement behind `focus_delivery.rs`'s event list (ADR 11xx-b): which event types does `el.focus()` deliver before it returns, over
// 18 moves? Every `on*` event type `window` has, and the legacy and composition names, are listened for in the capture phase on `window` and
// `document` (and in a child frame); the moves are from nothing, from an edited input, into a textarea, button, contenteditable, a shadow
// tree, a dialog, a frame and back, a closed `<details>`, a select, a popover (open, then focus outside), with `preventScroll`, and from a
// removed element. Each event is recorded twice (window and document); the last line lists the types.
//
//   node fixtures/reactive-ir/leaf-focus-no-listener/focus-events-probe.mjs      (the pinned headless shell; under a minute)
//
// Measured with the headless shell 1223 (Chrome 148.0.7778.96): DOMFocusIn, DOMFocusOut, blur, change, focus, focusin, focusout. The
// project's own loop (scripts/fuzz/surface-dom.json, `HTMLOrSVGElement.focus`: `focus`, `focusin` to a fresh frame's element) agrees on the
// receiver's half. An IME composition cannot be started in a headless page: the composition events are named after ADR 0642, not measured.
import { createRequire } from "node:module";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
const ROOT = join(dirname(fileURLToPath(import.meta.url)), "../../..");
const require = createRequire(`${ROOT}/scripts/runtime-oracle-install/`);
const { chromium } = require("playwright-core");
const { pinnedBrowser } = await import(`${ROOT}/scripts/playwright-browser.mjs`);
const browser = await chromium.launch({ executablePath: pinnedBrowser().executable });
const page = await browser.newPage();
await page.setContent(`<!doctype html><html><body>
<input id="a" value="x"><input id="b"><textarea id="t"></textarea><button id="btn">go</button>
<div id="ce" contenteditable="true">edit</div><div id="plain">plain</div>
<dialog id="dlg"><input id="din"></dialog>
<details id="det"><summary id="sum">s</summary><input id="inside"></details>
<div id="host"></div>
<iframe id="fr" srcdoc="<input id='fin'>"></iframe>
<div popover id="pop"><input id="pin"></div>
<select id="sel"><option>1</option><option>2</option></select>
</body></html>`);
await page.evaluate(() => {
  const host = document.getElementById("host");
  const root = host.attachShadow({ mode: "open" });
  root.innerHTML = '<input id="sin">';
});
await page.waitForTimeout(200);
const result = await page.evaluate(async () => {
  const names = new Set();
  for (const key of Object.getOwnPropertyNames(window)) if (key.startsWith("on")) names.add(key.slice(2));
  for (const extra of ["focusin", "focusout", "DOMFocusIn", "DOMFocusOut", "change", "compositionend", "beforetoggle", "toggle", "select", "selectionchange", "visibilitychange", "scroll", "scrollend", "formdata", "load", "input", "beforeinput", "keydown", "keyup", "click", "mousedown", "pointerdown", "invalid", "submit", "blur", "focus", "resize", "popstate", "hashchange", "pagehide", "pageshow", "unload", "beforeunload"]) names.add(extra);
  let log = null;
  const record = (event) => { if (log) log.push(`${event.type}@${event.target === window ? "window" : event.target === document ? "document" : event.target.id || event.target.nodeName}${event.eventPhase === 1 ? "(capture)" : ""}`); };
  for (const name of names) {
    window.addEventListener(name, record, true);
    document.addEventListener(name, record, true);
  }
  const frame = document.getElementById("fr").contentWindow;
  for (const name of names) frame.addEventListener(name, (event) => { if (log) log.push(`frame:${event.type}@${event.target === frame ? "window" : event.target.id || event.target.nodeName}`); }, true);
  const el = (id) => document.getElementById(id);
  const shadowInput = () => el("host").shadowRoot.getElementById("sin");
  const run = (label, setup, call) => {
    try { setup && setup(); } catch (e) { /* ignore */ }
    log = [];
    let error = null;
    try { call(); } catch (e) { error = String(e); }
    const events = log; log = null;
    return { label, events, error };
  };
  const out = [];
  document.body.focus();
  (document.activeElement)?.blur?.();
  out.push(run("body -> b", null, () => el("b").focus()));
  out.push(run("b -> b (already focused)", null, () => el("b").focus()));
  out.push(run("b -> a", null, () => el("a").focus()));
  // user-like edit of a: set value via execCommand insertText while focused (marks it user-edited)
  el("a").focus(); el("a").select(); document.execCommand("insertText", false, "typed");
  out.push(run("a (edited) -> b", null, () => el("b").focus()));
  el("a").focus(); el("a").select(); document.execCommand("insertText", false, "typed2");
  out.push(run("a (edited) -> textarea", null, () => el("t").focus()));
  out.push(run("-> button", null, () => el("btn").focus()));
  out.push(run("-> contenteditable", null, () => el("ce").focus()));
  out.push(run("-> plain div (not focusable)", null, () => el("plain").focus()));
  out.push(run("-> shadow input", null, () => shadowInput().focus()));
  el("dlg").show();
  out.push(run("-> input in open dialog", null, () => el("din").focus()));
  el("dlg").close();
  out.push(run("-> frame input", null, () => el("fr").contentDocument.getElementById("fin").focus()));
  out.push(run("-> main input after frame", null, () => el("a").focus()));
  out.push(run("-> input in closed details", null, () => el("inside").focus()));
  out.push(run("-> select", null, () => el("sel").focus()));
  el("pop").showPopover();
  out.push(run("a -> input in open popover", null, () => el("pin").focus()));
  out.push(run("popover open, focus outside b", null, () => el("b").focus()));
  el("pop").hidePopover();
  out.push(run("focus with preventScroll", null, () => el("t").focus({ preventScroll: true })));
  // an element removed while focused, then focus elsewhere
  const tmp = document.createElement("input"); document.body.append(tmp); tmp.focus();
  tmp.remove();
  out.push(run("after focused element removed -> b", null, () => el("b").focus()));
  return out;
});
for (const row of result) console.log(row.label.padEnd(36), row.error ? `ERROR ${row.error}` : JSON.stringify(row.events));
const kinds = new Set(result.flatMap((row) => row.events.map((event) => event.replace(/@.*/, "").replace(/^frame:/, ""))));
console.log("\nEvent types delivered synchronously by focus(), all scenarios:", [...kinds].sort().join(", "));
await browser.close();
