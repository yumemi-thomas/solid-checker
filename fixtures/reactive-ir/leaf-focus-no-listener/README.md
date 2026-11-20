# leaf-focus-no-listener (ADR 1147)

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
