// The event types each synchronously dispatching member fires on its caller's
// stack, observed in Chrome (ADR 0361's type sets). Every event type a page can
// listen for is captured on `window`, where the capture phase of any element
// target passes; each entry is `type@target:sync|async`, `sync` while the member
// call is on the stack. Each family is its own scripted click. The page claims,
// for each family, the set the checker assumes: an observed synchronous event
// outside it is `unclaimed`. `execCommand` is the one dispatching member the
// checker gives no set (the command picks among editing, clipboard and selection
// events: Chrome fires `selectstart` for `selectAll` and `input` for
// `insertText`), so it is no family here: it would withdraw every claim.
import { onCleanup } from "solid-js";

const log: Record<string, string[]> = {};
let family = "";
let inCall = false;

const focusMove = ["focus", "focusin", "blur", "focusout", "change", "compositionend"];
const unfocus = ["blur", "focusout", "change", "compositionend"];
const click = [
  "click", "input", "change", "submit", "formdata", "invalid", "reset", "beforetoggle", "command", "cancel",
  "beforematch", "focus", "focusin", "blur", "focusout", "compositionend",
];
const reportValidity = ["invalid", ...focusMove];
const requestSubmit = ["submit", "formdata", "invalid", "beforetoggle", ...focusMove];
const toggle = ["beforetoggle", ...focusMove];
const requestClose = ["cancel", "beforetoggle", ...focusMove];
// The set the checker assumes per family (rust/crates/solid-reactive-ir/src/dispatch_events.rs).
const claimed: Record<string, string[]> = {
  "click-plain": click,
  "click-checkbox": click,
  "click-label": click,
  "click-submit-valid": click,
  "click-submit-invalid": click,
  "click-reset": click,
  "click-popovertarget": click,
  "click-command-show-modal": click,
  "click-anchor-fragment": click,
  "dispatch-mouse-click": click,
  "dispatch-pointer-click": click,
  "focus": focusMove,
  "blur": unfocus,
  "checkValidity": ["invalid"],
  "form.checkValidity": ["invalid"],
  "reportValidity": reportValidity,
  "form.reset": ["reset"],
  "requestSubmit-valid": requestSubmit,
  "requestSubmit-invalid": requestSubmit,
  "requestSubmit-dialog": requestSubmit,
  "showPopover": toggle,
  "hidePopover": toggle,
  "togglePopover": toggle,
  "dialog.show": toggle,
  "dialog.close": toggle,
  "dialog.showModal": toggle,
  "dialog.requestClose": requestClose,
  "abort": [],
  "xhr": [],
};
const unclaimed = () =>
  Object.entries(log)
    .flatMap(([name, entries]) =>
      entries
        .filter((entry) => entry.endsWith(":sync"))
        .map((entry) => [name, entry.split("@")[0]] as const)
        .filter(([, type]) => !(claimed[name] ?? []).includes(type))
        .map(([, type]) => `${name}:${type}`),
    )
    .join(",");
Object.assign(globalThis, {
  oracleProbe: {
    log,
    get unclaimed() { return unclaimed(); },
    get scrollOnStack() { return Object.values(log).some((entries) => entries.some((entry) => entry.startsWith("scroll@") && entry.endsWith(":sync"))) ? "yes" : "no"; },
  },
});

const types = [
  "click", "input", "change", "submit", "formdata", "invalid", "reset", "focus", "blur", "focusin", "focusout",
  "beforetoggle", "toggle", "command", "cancel", "close", "beforeinput", "copy", "cut", "paste", "select",
  "selectstart", "compositionend", "scroll", "abort", "readystatechange", "loadstart", "loadend", "load",
  "error", "keydown", "keyup", "pointerdown", "mousedown", "mouseup", "dblclick", "contextmenu", "beforematch",
];
for (const type of types) {
  window.addEventListener(
    type,
    (event) => {
      const target = event.target;
      if (target instanceof HTMLElement && target.dataset.probe !== undefined) return;
      const name = target instanceof Element ? target.id || target.localName : target === window ? "window" : target === document ? "document" : "other";
      (log[family] ??= []).push(`${type}@${name}:${inCall ? "sync" : "async"}`);
    },
    true,
  );
}

// Forms submit into a hidden frame, so the page stays: `submit` and `formdata` fire as in a navigation.
const host = document.createElement("div");
document.body.append(host);
host.innerHTML = `
  <iframe name="sink" hidden></iframe>
  <input id="a" /> <input id="b" /> <input type="checkbox" id="cb" />
  <button id="plain">plain</button>
  <form id="valid" target="sink"><input name="n" value="x" /><button id="vsubmit" type="submit">s</button></form>
  <form id="invalid" target="sink"><input id="req" name="r" required /><button id="isubmit" type="submit">s</button></form>
  <form id="resettable" target="sink"><input name="n" value="x" /><button id="rbutton" type="reset">r</button></form>
  <div id="pop" popover>popover</div><button id="poptarget" popovertarget="pop">toggle</button>
  <dialog id="dlg"><button id="indlg">in</button></dialog>
  <button id="cmd" commandfor="dlg" command="show-modal">cmd</button>
  <label id="lbl" for="cb2">label</label><input type="checkbox" id="cb2" />
  <div id="pop2" popover="auto">popover2</div>
  <a id="anchor" href="#revealed">go</a><div hidden="until-found"><p id="revealed">revealed</p></div>
  <dialog id="dlg2" open><form id="dialogform" method="dialog"><button>ok</button></form></dialog>
  <input type="checkbox" id="cb3" />
`;
const $ = <T extends Element>(id: string) => host.querySelector<T>("#" + id)!;

function run(name: string, act: () => void, prepare?: () => void) {
  family = "prepare";
  prepare?.();
  family = name;
  inCall = true;
  try {
    act();
  } finally {
    inCall = false;
  }
}
const scenarios: Array<[string, () => void, (() => void)?]> = [
  ["click-plain", () => $<HTMLElement>("plain").click()],
  ["click-checkbox", () => $<HTMLElement>("cb").click()],
  ["click-label", () => $<HTMLElement>("lbl").click()],
  ["click-submit-valid", () => $<HTMLElement>("vsubmit").click()],
  ["click-submit-invalid", () => $<HTMLElement>("isubmit").click()],
  ["click-reset", () => $<HTMLElement>("rbutton").click()],
  ["click-popovertarget", () => $<HTMLElement>("poptarget").click()],
  ["click-command-show-modal", () => $<HTMLElement>("cmd").click()],
  ["click-anchor-fragment", () => $<HTMLElement>("anchor").click()],
  ["dispatch-mouse-click", () => $<HTMLElement>("cb3").dispatchEvent(new MouseEvent("click", { bubbles: true }))],
  ["dispatch-pointer-click", () => $<HTMLElement>("cb3").dispatchEvent(new PointerEvent("click", { bubbles: true }))],
  ["focus", () => $<HTMLElement>("a").focus(), () => $<HTMLElement>("b").focus()],
  ["blur", () => $<HTMLElement>("b").blur(), () => $<HTMLElement>("b").focus()],
  ["checkValidity", () => $<HTMLInputElement>("req").checkValidity()],
  ["form.checkValidity", () => $<HTMLFormElement>("invalid").checkValidity()],
  ["reportValidity", () => $<HTMLInputElement>("req").reportValidity(), () => $<HTMLElement>("a").focus()],
  ["form.reset", () => $<HTMLFormElement>("resettable").reset()],
  ["requestSubmit-valid", () => $<HTMLFormElement>("valid").requestSubmit(), () => $<HTMLElement>("a").focus()],
  ["requestSubmit-invalid", () => $<HTMLFormElement>("invalid").requestSubmit(), () => $<HTMLElement>("a").focus()],
  ["requestSubmit-dialog", () => $<HTMLFormElement>("dialogform").requestSubmit()],
  ["showPopover", () => $<HTMLElement>("pop2").showPopover(), () => $<HTMLElement>("a").focus()],
  ["hidePopover", () => $<HTMLElement>("pop2").hidePopover()],
  ["togglePopover", () => $<HTMLElement>("pop2").togglePopover()],
  ["dialog.show", () => $<HTMLDialogElement>("dlg").show(), () => $<HTMLElement>("a").focus()],
  ["dialog.close", () => $<HTMLDialogElement>("dlg").close()],
  ["dialog.showModal", () => $<HTMLDialogElement>("dlg").showModal(), () => $<HTMLElement>("a").focus()],
  ["dialog.requestClose", () => $<HTMLDialogElement>("dlg").requestClose()],
  ["abort", () => new AbortController().abort()],
  [
    "xhr",
    () => {
      const request = new XMLHttpRequest();
      request.open("GET", "data:text/plain,x");
      request.abort();
    },
  ],
];

// The control: no family fires `scroll`, so this handler is never run by a dispatch. It runs from the
// harness with no owner, and its `onCleanup` is a proven SC4001 violation (NO_OWNER_CLEANUP).
export function Probe() {
  return (
    <section>
      <div onScroll={() => { onCleanup(() => undefined); }}>scroll</div>
      {scenarios.map(([name, act, prepare]) => (
        <button data-probe={name} onClick={() => run(name, act, prepare)}>{name}</button>
      ))}
    </section>
  );
}
