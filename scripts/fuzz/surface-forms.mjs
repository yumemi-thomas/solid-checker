// The hand-kept half of the surface: what no list in the bytes holds.
//
// JSX_FORMS: the forms the pinned compiler is asked about (surface.mjs compileForms); the API census's kitchen sink (surface/jsx/forms.mjs,
// 2026-11-05). The compiler is a native binary with no list of its forms, so a new form on a Solid bump shows as a new helper export or
// a changed README (surface.test.mjs pins its hash); the forms themselves are this list.
//
// PRODUCTS: the crossings a name row cannot answer (the census review's section 6: `new Map` holding plain values satisfies a name row
// and proves nothing about an accessor in a Map). `productRows` crosses each list, so a new carrier or value kind makes new rows.
//
// ABSTRACT_OPERATIONS: the operations of the language that run user code without a call in the source (ECMA-262 abstract operations,
// Annex B keys). `__proto__` is a literal key, in no export list, no `.d.ts` row and no SyntaxKind.
export const JSX_FORMS = {
  "static attr": `<div id="a" />`, "dynamic attr": `<div id={v()} />`, "bool attr true": `<input disabled />`, "bool attr dyn": `<input disabled={v()} />`,
  "data-*": `<div data-x={v()} />`, "aria-*": `<div aria-label={v()} />`, "class string": `<div class={v()} />`, "class object": `<div class={{ a: v() }} />`, "class array": `<div class={["a", { b: v() }]} />`,
  "classList (legacy)": `<div classList={{ a: v() }} />`, "style string": `<div style={v()} />`, "style object": `<div style={{ color: v() }} />`, "style var": `<div style={{ "--x": v() }} />`,
  "innerHTML": `<div innerHTML={v()} />`, "textContent": `<div textContent={v()} />`, "innerText": `<div innerText={v()} />`,
  "ref variable": `<div ref={el} />`, "ref callback": `<div ref={(e) => f(e)} />`, "ref member": `<div ref={o.el} />`, "ref array": `<div ref={[r1, r2]} />`, "ref call-result": `<div ref={mk()} />`,
  "onClick fn": `<button onClick={() => f()} />`, "onClick ident": `<button onClick={f} />`, "onClick member": `<button onClick={o.f} />`, "onClick tuple": `<button onClick={[f, 1]} />`, "onClick object": `<button onClick={{ handleEvent: f, passive: true }} />`,
  "onFocus (non-delegated)": `<input onFocus={f} />`, "onInput": `<input onInput={f} />`, "onScroll": `<div onScroll={f} />`, "onclick lowercase": `<button onclick={f} />`,
  "on:custom": `<div on:custom={f} />`, "oncapture:click": `<div oncapture:click={f} />`, "on:click object": `<div on:click={{ handleEvent: f, once: true }} />`,
  "prop:value": `<input prop:value={v()} />`, "prop:custom": `<x-el prop:foo={v()} />`, "attr:title": `<div attr:title={v()} />`, "bool:hidden": `<div bool:hidden={v()} />`,
  "use:directive": `<div use:dir={v()} />`, "use:directive bare": `<div use:dir />`, "class:name": `<div class:a={v()} />`, "style:prop": `<div style:color={v()} />`,
  "xlink:href": `<svg><use xlink:href={v()} /></svg>`, "xmlns": `<svg xmlns="http://www.w3.org/2000/svg" />`, "svg": `<svg><circle r={v()} /></svg>`, "math": `<math><mi>{v()}</mi></math>`,
  "custom element": `<my-el foo={v()} />`, "is=": `<button is="x-btn" />`, "$ServerOnly": `<div $ServerOnly />`, "$key": `<li $key={v()} />`,
  "spread": `<div {...p} />`, "spread + attrs": `<div id="a" {...p} class="b" />`, "spread fn-call": `<div {...mk()} />`, "component spread": `<C {...p} />`,
  "children text": `<div>hi</div>`, "children expr": `<div>{v()}</div>`, "children call": `<div>{f()}</div>`, "children cond": `<div>{v() ? <a /> : <b />}</div>`, "children &&": `<div>{v() && <a />}</div>`,
  "children array": `<div>{[<a />, <b />]}</div>`, "children map": `<ul>{xs().map((x) => <li>{x}</li>)}</ul>`, "children fn (component)": `<C>{(x) => <a>{x}</a>}</C>`, "children prop": `<C children={<a />} />`,
  "children mixed": `<div>a{v()}<b />{" "}c</div>`, "fragment": `<><a /><b /></>`, "fragment keyed": `<Fragment><a /></Fragment>`, "nested component": `<C><D /></C>`,
  "member tag": `<Ns.C x={v()} />`, "lowercase member tag": `<ns.c x={v()} />`, "deep member tag": `<A.B.C />`, "namespaced tag": `<svg:rect />`, "component with getter prop": `<C x={v()} />`, "component static prop": `<C x="a" />`,
  "component bool prop": `<C x />`, "component ref": `<C ref={el} />`, "component event-named prop": `<C onClick={f} />`, "dynamic tag (Dynamic)": `<Dynamic component={v()} x={v()} />`,
  "Portal": `<Portal mount={m}><a /></Portal>`, "Show fn": `<Show when={v()}>{(x) => <a>{x()}</a>}</Show>`, "Show keyed": `<Show when={v()} keyed>{(x) => <a>{x}</a>}</Show>`, "For": `<For each={xs()}>{(x, i) => <li>{x}{i()}</li>}</For>`,
  "For unkeyed": `<For each={xs()} keyed={false}>{(x, i) => <li>{x()}{i}</li>}</For>`, "Repeat": `<Repeat count={v()}>{(i) => <li>{i}</li>}</Repeat>`, "Switch/Match": `<Switch fallback={<a />}><Match when={v()}><b /></Match></Switch>`,
  "Loading": `<Loading fallback={<a />}><C /></Loading>`, "Loading on": `<Loading on={v()}><C /></Loading>`, "Errored": `<Errored fallback={(e, r) => <a onClick={r}>{String(e())}</a>}><C /></Errored>`, "Reveal": `<Reveal order="sequential"><C /></Reveal>`,
  "Hydration": `<Hydration><C /></Hydration>`, "NoHydration": `<NoHydration><C /></NoHydration>`, "<template>": `<template><a /></template>`, "<script>": `<script>{"x"}</script>`, "<style>": `<style>{"a{}"}</style>`,
  "<slot>": `<slot name="a" />`, "<textarea value>": `<textarea value={v()} />`, "<select value>": `<select value={v()}><option value="a">a</option></select>`, "<input checked>": `<input type="checkbox" checked={v()} />`,
  "<input value>": `<input value={v()} />`, "<dialog open>": `<dialog open={v()} />`, "<details open>": `<details open={v()} />`, "popover attr": `<div popover="auto" />`, "<a href>": `<a href={v()} />`,
  "<img src>": `<img src={v()} />`, "<iframe srcdoc>": `<iframe srcdoc={v()} />`, "<video autoplay>": `<video autoplay muted />`, "entity": `<div>&amp;&nbsp;</div>`, "JSX comment": `<div>{/* c */}</div>`,
  "attr template literal": "<div id={`a${v()}`} />", "attr string template": "<div id={`a`} />", "attr arrow (non-event)": `<div title={() => v()} />`, "attr IIFE": `<div id={(() => v())()} />`, "attr comma": `<div id={(f(), v())} />`,
  "conditional attr spread": `<div {...(v() ? p : {})} />`, "attr with typeof": `<div id={typeof v()} />`, "attr optional chain": `<div id={o?.x} />`, "attr nullish": `<div id={o.x ?? "a"} />`,
  "attr non-null": `<div id={o!.x} />`, "attr as": `<div id={o.x as string} />`, "attr satisfies": `<div id={o.x satisfies string} />`, "attr tagged template": "<div id={tag`a${v()}`} />", "attr new": `<div id={new Date().toString()} />`,
  "attr JSX in attr": `<C x={<a />} />`, "attr array literal": `<C x={[v()]} />`, "attr object literal": `<C x={{ a: v() }} />`, "attr getter object": `<C x={{ get a() { return v(); } }} />`, "attr class expr": `<C x={class {}} />`,
  "attr regex": `<C x={/a/} />`, "attr bigint": `<C x={1n} />`, "attr void": `<C x={void 0} />`, "attr delete": `<C x={delete o.x} />`, "attr assignment": `<C x={(o.x = 1)} />`,
  "children generator": `<ul>{(function* () { yield <li />; })()}</ul>`, "children Promise": `<div>{Promise.resolve("a")}</div>`, "children async iterable": `<div>{it}</div>`, "children Symbol": `<div>{Symbol("a")}</div>`, "children bigint": `<div>{1n}</div>`,
  "children Node": `<div>{document.createElement("b")}</div>`, "children store": `<div>{s.v}</div>`, "children deep expr": `<div>{xs()[0].a.b}</div>`,
};

export const ABSTRACT_OPERATIONS = [
  "Get: getter on an object literal", "Get: getter on a class", "Get: getter through a prototype chain", "Get: Proxy get trap",
  "Set: setter on an object literal", "Set: setter on a class", "Set: Proxy set trap",
  "ToPrimitive: Symbol.toPrimitive", "ToPrimitive: valueOf", "ToPrimitive: toString in a template", "ToPrimitive: toString in string concatenation", "ToPrimitive: property-key coercion",
  "ToPropertyKey: computed key with an object", "OrdinaryHasInstance: Symbol.hasInstance through instanceof",
  "GetIterator: Symbol.iterator in for-of", "GetIterator: Symbol.iterator in spread", "GetIterator: Symbol.iterator in array destructuring", "GetIterator: Symbol.asyncIterator in for-await", "IteratorClose: return() on early exit",
  "Await: thenable then()", "PromiseResolve: thenable handed to resolve", "Promise species: Symbol.species",
  "DisposeResources: Symbol.dispose on using", "DisposeResources: Symbol.asyncDispose on await using",
  "SetPrototypeOf: __proto__ literal key", "SetPrototypeOf: __proto__ accessor", "SetPrototypeOf: Object.setPrototypeOf",
  "AnnexB: __defineGetter__", "AnnexB: __defineSetter__", "AnnexB: __lookupGetter__", "AnnexB: __lookupSetter__",
  "JSON.stringify: toJSON", "JSON.parse: reviver", "Array species: Symbol.species", "String: Symbol.replace and friends through a RegExp-like", "Function.prototype.toString",
  "Class field initializer", "Class static block", "Decorator evaluation", "Getter on a default parameter pattern", "Parameter default evaluation", "Destructuring default evaluation",
  "Tagged template call", "new.target", "Import evaluation: top-level code of an imported module", "Dynamic import evaluation", "Finalization callback",
];

export const PRODUCTS = {
  // Value kind × path (review rank 1): one accessor, handed any of these ways, is the same read to the runtime.
  value: {
    a: ["accessor", "setter", "store-root", "props", "handed-member", "pending"],
    b: [
      "class-public-field", "class-ctor-field", "class-static-field", "class-private-field", "class-private-static", "this-getter",
      "map-get", "map-set-get", "set-values", "weakmap-get", "weakref-deref", "proxy-wrap", "globalthis",
      "object-assign", "object-fromentries", "object-values", "array-at", "array-prototype-call",
      "reassign", "reassign-block", "reassign-for", "reassign-try", "reassign-switch", "reassign-label", "reassign-static-block", "reassign-called-arrow", "reassign-promise-executor", "reassign-foreach",
      "or-tail", "and-tail", "nullish-tail", "comma-tail", "iife-return", "generator-yield", "await-promise-resolve",
      "api-return-children", "api-return-mapArray", "api-return-repeat", "api-return-createRoot", "api-return-untrack", "api-return-runWithOwner",
    ],
  },
  // Solid callback × closure layer (review rank 2): a callback runs in the window of the call that invokes it.
  callback: {
    a: ["reconcile-key", "createRoot", "runWithOwner", "mapArray-map", "repeat-map", "untrack"],
    b: ["inline-timer", "host-property", "named-timer", "named-then", "named-listener", "object-on-host", "named-handler", "after-await-iife", "factory-scheduled", "project-helper", "timer-in-apply", "body-iife"],
  },
  // Mount × claim (review rank 3): a body's window depends on how it was mounted.
  mount: {
    a: ["Dynamic", "clientOnly-first-mount", "clientOnly-later-mount", "dynamicComponent", "lazy-resolved", "lazy-import", "mapArray-render"],
    b: ["body-read", "body-write", "pending-read"],
  },
  // Callee form (review rank 4), and pending-source kind (rank 5): one list each.
  callee: { a: ["f()[0]()", "Object.values(o)[0]()", "Array.from([c])[0]()", "[c].at(0)!()", "m.get(k)()", "pick()[0]()", "createMemo(fn)()"] },
  pending: { a: ["NotReadyError-throw", "NotReadyError-helper", "async-generator", "until-in-sync-memo", "resolve-in-sync-memo", "promise-of-accessor"] },
};

/** Every crossing, as `product/<axis>/<a>*<b>`, or `product/<axis>/<a>` for a plain list. */
export const productRows = () => Object.entries(PRODUCTS).flatMap(([axis, { a, b }]) => a.flatMap((one) => (b ? b.map((other) => `product/${axis}/${one}*${other}`) : [`product/${axis}/${one}`])));
