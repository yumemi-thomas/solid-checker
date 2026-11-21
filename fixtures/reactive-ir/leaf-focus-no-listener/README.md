# leaf-focus-no-listener (ADR 1151)

**Claim.** The leaf-scope walk refuses an element's `focus()` (ADR 0329): a focus move delivers `focus`, `focusin`, `blur`, `focusout` and
`change` before it returns, and a listener for one runs on the stack of the leaf callback, under the leaf owner, where `onCleanup`, a flush
and a creation are refused (SC9012 `leaf-callback-opaque` at the call). The refusal is lifted for that one member where the whole program
holds nothing that could run there (`focus_delivery.rs`):

- no listener for an event a focus move delivers, anywhere: a JSX attribute naming one (`onFocus`, `onBlur`, `onFocusIn`, `onFocusOut`,
  `onChange`, `onInput`, …), a spread onto an intrinsic element, an `addEventListener` that is not a direct call with a literal type outside
  the list, an `on<event>` property or key, markup or an event attribute built from a string, a private delegated key;
- nothing that stands in for `focus`: a class, object literal or assignment member named `focus`, or any write the shared standard-member
  premise (ADR 1129) withholds, in the project or a loaded package; an unread module is one.

The call is `x.focus(…)` or `x?.focus(…)` with the exact `lib.dom` declaration, and every argument a primitive or an object literal of data
(the host reads an option bag's members: an operand or a literal with an accessor runs code on the stack of the call).

*What `focus()` delivers* was measured in Chrome 148 (the pinned headless shell) with every `on*` event type of `window` and the legacy and
composition names listened for in the capture phase on `window`, `document` and a child frame, over 18 moves (from nothing, from an edited
input, into a shadow tree, a dialog, a popover, a frame and back, from a removed element, `preventScroll`): `focus`, `focusin`, `DOMFocusIn`
(the receiver's), `blur`, `focusout`, `DOMFocusOut`, `change` (an input the user edited, when focus leaves it) and, across a frame boundary,
`focus` and `blur` at the window. The composition and input events of an IME commit are named after ADR 0642, not measured (a headless page
has no composition).

- `App.tsx`: `LetRef` (the textbook shape, `cs-ref-directive`), `Namespace` (`S.onSettled`), `Direct`, `OptionBag`, `Tracked` (`createTrackedEffect`) and `ViaHelper` are
  clean. `Blur`, `Click`, `ShowModal` and the bare global `BareFocus` keep their SC9012 (the proof is for an element's `focus()`);
  `OptionsBinding` and `OptionsGetter` keep theirs (the argument is no data).
- `variants/jsx-focus`, `jsx-blur` (the element losing focus hears `blur` inside the second leaf callback), `jsx-focusin`, `jsx-change`,
  `jsx-spread` (a spread's keys are no fact's): the program-wide premise fails, every `focus()` keeps its row.
- `variants/add-listener-literal`, `add-listener-variable` (a type no fact names), `add-listener-alias` (a bound method), `on-property`,
  `object-key` (`Object.assign(el, { onfocus })`), `package-listener` (an installed package listens for `focusin` where it loads).
- `variants/definer-class` (a custom element overrides `focus`), `definer-object` (an impostor cast to the element type, handed in as a
  prop), `assign-prototype` (`HTMLElement.prototype.focus = …`), `define-property` (`Object.defineProperty(HTMLElement.prototype, "focus", …)`).
- `variants/other-listeners` (negative control): a `keydown` listener and an `onClick` handler are none a focus move delivers, so the call is
  proved.

Chrome (`scripts/runtime-oracle.mjs --cases fixtures/reactive-ir --only leaf-focus-no-listener`, and `--cases
fixtures/reactive-ir/leaf-focus-no-listener/variants`): the main case is quiet; every variant with a listener or a stand-in reports
`CLEANUP_IN_FORBIDDEN_SCOPE` (the listener's `onCleanup`, on the stack of the leaf callback's `focus()`) and is covered by the kept SC9012.
Stubs: `solid-js.d.ts` declares the three primitives and the intrinsic elements; `tsc` accepts every case against the real rc.21 packages.

## Round 1 (the Opus review of 35704e13e, FIX-FIRST)

The v1 census read the syntax of a few spellings and did not follow the runtime's registration helpers; Chrome rc.21 confirmed four false cleans and three more
were inferred. Every case is a variant below, with the registration route it names; each keeps its SC9012 at the call (the quiet controls are proved), and the
reviewed build had **no** SC9012 at the call for the ones marked `old`. Every name is compared by its decoded value (the coordinator's item from ADR 1148's round 2:
`\u0061ddEventListener`, `"\u0066ocus"`, `o\u006efocus`), and the escaped spellings are variants too.

- The runtime's registrars (`@solidjs/web`, by the dialect's per-specifier audit `runtime_module_listener_registrations`): `runtime-add-event` (F1, old),
  `runtime-add-event-delegated` (F2, old: `addEvent(el, "focusin", h, true)` with `delegateEvents`), `runtime-set-property` (old), `runtime-set-attribute` (F7,
  inferred, old), `runtime-template` (F6, inferred, old), `runtime-spread`, `runtime-frames` (old: the frames tier is audited as `any`), `runtime-path-alias` (the quiet
  `click` call by a `paths` alias into the runtime's file: unknown, taken as registering) and the control `runtime-add-event-quiet` (the same quiet call by the
  runtime's own specifier: every call passes a constant outside the focus list, so it is proved).
- Computed keys and spreads: `computed-add` (F3, old: `const add = "addEventListener"; el[add]("focus", h)`), `computed-unproven-key`, `dynamic-spread` (F4, old: a spread onto
  `Dynamic` that `Object.fromEntries` fills with `onFocus`).
- Markup: `jsx-inner-html` (F5, inferred, old), `jsx-inner-html-escaped` (old: `o\u006efocus` in the string), the control `jsx-inner-html-quiet` (a constant with no event
  attribute and no script: proved).
- Global writes (our own findings): `global-onfocus` (old), `global-onchange-logical` (`onchange ??= h`, old), `global-escaped-onfocus` (old).
- Installed packages: `package-typed` (a package imported by name resolves to its declaration file; its runtime source is no analysed file), `package-computed` (the
  package's source is read, and registers by a computed key).
- Escaped spellings: `escaped-method-name`, `-string`, `-template` (`addEventListener`), `escaped-event-name`, `-template` (`"\u0066ocus"`), `escaped-on-property`,
  `-string`, `escaped-handler-key`, `escaped-document-listener`, `escaped-bare-listener` (old: the bare global's name was compared as source text), `escaped-focus-member`
  (`f\u006fcus`), and the control `escaped-quiet-event` (`"\u006beydown"` is no focus-family event: proved).
- `oracle-add-event`, `oracle-add-event-delegated`, `oracle-computed-add`, `oracle-dynamic-spread` are the reviewer's four Chrome-verified cases with the handler in a
  package (`fz-handler`, the reviewer's `pr-h`): the only App frame on the stack is the `focus()` line, so only the finding there covers Chrome's diagnostic. The
  oracle reports `unproven-clean` for the reviewed build and `covered-uncertifiable` for this one (`node scripts/runtime-oracle.mjs --cases
  fixtures/reactive-ir/leaf-focus-no-listener/variants --only oracle-add-event,oracle-add-event-delegated,oracle-computed-add,oracle-dynamic-spread`). The other variants
  write the handler in `App.tsx`, where its own rows (SC3001, SC4001) cover the diagnostic at its `onCleanup`; their snapshots hold the row at the call.
- The page (`../leaf-focus-page-*`, top-level fixtures: a project's HTML is read from its whole tree): `clean` (the page's only script is the program's module entry:
  proved), `inline-handler`, `body-handler`, `inline-script`, `extra-script`, `second-module` and `unread` (an element host discovery's grammar does not read) each
  withhold the proof. Round 2 (the review's N1, the grammar of ADR 1154): `solidus-body` (`<body/onfocusin=…>`), `solidus-attribute` (`<div id="root"/onfocusin=…>`),
  `uppercase-handler` (`ONFOCUSIN`), `uppercase-solidus`, `comment-bang` (a comment HTML ends at `--!>`), and the pages of `public/`: `public-handler`
  (`<body/onfocus=…>`), `public-upper`; the control `public-static` (markup alone) keeps the proof. The reviewed build (853e30b5c) proved the first six and the two
  `public` handlers (no SC9012 at the call); Chrome 148 runs each handler (`../product-realm-page-handler-solidus/html-tokenizer-probe.mjs`). A project with no HTML is the
  closed world (the main case).

The stubs of the `@solidjs/web` variants declare the registrars byte-faithfully to `types/client.d.ts` of `@solidjs/web@2.0.0-rc.21`, except `Dynamic` (the stub
declares no `JSX.Element`); the runtime oracle types every case against the real packages. The `runtime-path-alias` stand-in `web.js` is written for the fixture, not the
published bytes: the dialect answers by the module the build resolves to, and the runtime's own files are not read.
