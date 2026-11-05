// The measured DOM loop (the census review's item 3): instead of a hand list of the methods that dispatch, call every host method of
// surface.mjs in the pinned Chrome with listeners for every event, and record what ran before the call returned. `.focus()` runs a
// handler inside the caller's window; a list of names cannot say so, and a new method answers itself here. Each call gets a fresh
// iframe, in two states: as made, and prepared (focused, dialog and popover open, form filled), because `blur` or `close` on an element
// that is not in that state dispatches nothing. What a method does asynchronously is not measured: it runs outside the caller's window.
// Light (one page, under a minute) and not part of a test: rerun it when the Playwright pin moves, and surface.test.mjs says so.
//
//   node scripts/fuzz/surface-dom-probe.mjs        rewrites scripts/fuzz/surface-dom.json
import { writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { ORACLE_INSTALL, pinnedBrowser } from "../playwright-browser.mjs";
import { surfaceRows } from "./surface.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const { chromium } = createRequire(join(ORACLE_INSTALL, "package.json"))("playwright");

// Runs in the page. `key` is `Interface.method`.
function probe({ key, prepared }) {
  const [iface, method] = key.split(".");
  const frame = document.createElement("iframe");
  document.body.append(frame);
  const win = frame.contentWindow, doc = frame.contentDocument;
  doc.body.innerHTML = `<div id="d" tabindex="0">text<span id="s"></span></div><div id="q" popover="auto">pop</div><form id="f"><input id="i" name="n" value="v"><textarea id="t">t</textarea><select id="l"><option>a</option><option>b</option></select><button id="b"></button></form>
<dialog id="g"></dialog><details id="e"><summary>s</summary>x</details><a id="a" href="#x">l</a><img id="m"><canvas id="c"></canvas><template id="p"><i></i></template><video id="v"></video><iframe id="r"></iframe><svg id="w" tabindex="0"></svg><slot id="o"></slot>`;
  const of = (id) => doc.getElementById(id);
  const shadow = () => { const host = of("s"); return host.shadowRoot ?? host.attachShadow({ mode: "open" }); };
  const receivers = {
    EventTarget: () => of("d"), Node: () => (prepared ? of("f") : of("d")), Element: () => of("d"), HTMLElement: () => (/Popover$/.test(method) ? of("q") : of("d")), ParentNode: () => (prepared ? of("f") : of("d")), ChildNode: () => (prepared ? of("i") : of("s")), Animatable: () => of("d"), HTMLOrSVGElement: () => of("d"), GlobalEventHandlers: () => of("d"),
    SVGElement: () => of("w"), ShadowRoot: shadow, DocumentOrShadowRoot: () => doc, DocumentFragment: () => doc.createDocumentFragment(), Document: () => doc, XPathEvaluatorBase: () => doc,
    Window: () => win, WindowOrWorkerGlobalScope: () => win, AnimationFrameProvider: () => win, WindowEventHandlers: () => win,
    HTMLDialogElement: () => of("g"), HTMLFormElement: () => of("f"), HTMLInputElement: () => of("i"), HTMLTextAreaElement: () => of("t"), HTMLSelectElement: () => of("l"), HTMLButtonElement: () => of("b"), HTMLDetailsElement: () => of("e"),
    HTMLMediaElement: () => of("v"), HTMLAnchorElement: () => of("a"), HTMLHyperlinkElementUtils: () => of("a"), HTMLImageElement: () => of("m"), HTMLIFrameElement: () => of("r"), HTMLCanvasElement: () => of("c"), HTMLTemplateElement: () => of("p"), HTMLSlotElement: () => of("o"),
    CustomElementRegistry: () => win.customElements, AbortController: () => { const controller = new win.AbortController(); listen(controller.signal); return controller; }, AbortSignal: () => new win.AbortController().signal, Event: () => new win.Event("x"), History: () => win.history, Location: () => win.location, Storage: () => { try { return win.sessionStorage; } catch { return null; } }, Performance: () => win.performance,
    MutationObserver: () => new win.MutationObserver(() => {}), ResizeObserver: () => new win.ResizeObserver(() => {}), IntersectionObserver: () => new win.IntersectionObserver(() => {}), PerformanceObserver: () => new win.PerformanceObserver(() => {}), ReportingObserver: () => new win.ReportingObserver(() => {}),
  };
  const events = new Set();
  let ran = 0;
  const names = new Set(["x", "focus", "blur", "focusin", "focusout", "invalid", "submit", "reset", "toggle", "beforetoggle", "cancel", "close", "change", "input", "select", "slotchange", "scroll", "abort", "popstate", "hashchange", "storage", "message", "load", "error", "beforeinput", "formdata", "click", "dblclick", "selectionchange", "selectstart", "DOMContentLoaded", "readystatechange", "visibilitychange", "fullscreenchange", "fullscreenerror", "copy", "cut", "paste"]);
  for (const target of [win, doc]) for (const name in target) if (name.startsWith("on")) names.add(name.slice(2));
  const listen = (target) => { if (target?.addEventListener) for (const name of names) target.addEventListener(name, (event) => events.add(event.type), true); };
  listen(win); listen(doc);
  // Events that do not reach the window in the capture phase: on the elements, and in the shadow tree.
  for (const element of doc.querySelectorAll("*")) listen(element);
  const receiver = receivers[iface]?.();
  if (!receiver) return { skipped: `no receiver for ${iface}` };
  listen(receiver);
  if (prepared) {
    // An editable selection, for `execCommand`.
    try { const box = doc.createElement("div"); box.contentEditable = "true"; box.textContent = "e"; box.id = "z"; doc.body.append(box); listen(box); } catch {}
    // A modal dialog makes the rest of the document inert, so it opens only for its own methods.
    if (iface === "HTMLDialogElement") try { of("g").showModal(); } catch {}
    try { of("q").showPopover(); } catch {}
    try { of("e").open = true; } catch {}
    try { of("i").required = true; of("i").value = ""; of("t").required = true; of("t").value = ""; of("i").focus(); } catch {}
    try { if (iface === "Document") { of("z").focus(); doc.getSelection().selectAllChildren(of("z")); } else if (receiver.focus && receiver !== win && receiver !== of("f")) receiver.focus(); } catch {}
    events.clear();
  }
  const callback = () => { ran += 1; };
  const child = () => doc.createElement("b");
  const guesses = [...(method === "execCommand" ? [["insertText", false, "x"]] : []), ...(prepared && /^(removeChild|replaceChild)$/.test(method) ? [method === "replaceChild" ? [child(), of("i")] : [of("i")]] : []), [], [child()], ["x"], ["x", "y"], [of("s")], ["beforeend", child()], ["beforeend", "x"], [callback], [0], [0, 0], [{}], [new win.Event("x")], ["click", callback], [child(), of("s")], [doc.createRange()], ["div"], ["http://a.invalid/"]];
  const fn = receiver[method];
  if (typeof fn !== "function") return { absent: true };
  const needsArguments = fn.length;
  for (const args of guesses) {
    if (args.length < needsArguments) continue;
    events.clear(); ran = 0;
    try {
      fn.apply(receiver, args);
      return { args: args.map((one) => (typeof one === "function" ? "fn" : typeof one === "object" ? "obj" : String(one))), events: [...events].sort(), callback: ran > 0 };
    } catch (error) {
      if (events.size || ran) return { threw: String(error.name), events: [...events].sort(), callback: ran > 0 };
    }
  }
  return { noCall: true };
}

const { rows } = surfaceRows();
const keys = rows.filter((row) => row.startsWith("host/") && row.includes(".") && !row.endsWith(".on*")).map((row) => row.slice("host/".length));
const skipped = new Set(["Window.print", "Window.stop", "Window.close", "Window.open", "Window.alert", "Window.confirm", "Window.prompt", "Document.open", "Document.write", "Document.writeln", "Document.close", "Document.clear"]);
const pin = pinnedBrowser();
const browser = await chromium.launch({ executablePath: pin.executable, headless: true, args: ["--in-process-gpu"] });
const page = await (await browser.newContext()).newPage();
page.on("dialog", (dialog) => dialog.dismiss());
await page.setContent("<!doctype html><title>dom</title><body></body>");
const results = {};
for (const key of keys) {
  if (skipped.has(key)) { results[key] = { skipped: "ends or blocks the page" }; continue; }
  const one = {};
  for (const prepared of [false, true]) {
    try {
      one[prepared ? "prepared" : "fresh"] = await Promise.race([page.evaluate(probe, { key, prepared }), new Promise((_, reject) => setTimeout(() => reject(new Error("timeout")), 4000))]);
    } catch (error) { one[prepared ? "prepared" : "fresh"] = { failed: String(error.message).split("\n")[0].slice(0, 120) }; await page.reload().catch(() => {}); await page.setContent("<!doctype html><title>dom</title><body></body>").catch(() => {}); }
  }
  await page.evaluate(() => { document.body.innerHTML = ""; }).catch(() => {});
  results[key] = one;
}
await browser.close();
const out = { chromiumHeadlessShell: pin.revision, chrome: pin.browserVersion, measured: new Date().toISOString().slice(0, 10), results };
writeFileSync(join(here, "surface-dom.json"), `${JSON.stringify(out, null, 0).replaceAll('},"', '},\n"')}\n`);
const summary = Object.entries(results).filter(([, one]) => [one.fresh, one.prepared].some((state) => state?.events?.length || state?.callback));
console.log(`${keys.length} methods; dispatching or running a callback synchronously:`);
for (const [key, one] of summary) console.log(`  ${key}: fresh ${one.fresh?.events?.join(",") ?? "-"}${one.fresh?.callback ? " +cb" : ""}; prepared ${one.prepared?.events?.join(",") ?? "-"}${one.prepared?.callback ? " +cb" : ""}`);
console.log("not called:", Object.entries(results).filter(([, one]) => one.skipped || one.fresh?.noCall || one.fresh?.failed).map(([key]) => key).join(" "));
