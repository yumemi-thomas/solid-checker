# dispatch-delivery-join

**Claim (ADR 0361).** A synchronous dispatch runs handlers on its caller's stack, under its owner (ADR 0356), but only the handlers of the event types it fires. Every dispatching call (`dispatchEvent`, `focus()`, `click()`, `checkValidity()`, `abort()`, ...) is a set of types (`dispatch_events.rs`); an element handler stays an obligation (SC4001/SC2001 uncertifiable) when its `on…` type is in the union of every site's set in the program, and keeps its ownerless proof (a proven SC4001 violation; a write stays legal) otherwise. A call that may fire any type (an event argument that is not an inline `new …Event("T")`, a computed key, a stored member, an `execCommand`) makes every handler an obligation. A callback no element names whose every run ADR 0319's walk ends at intrinsic `on…` attributes (a forwarded prop) takes those attributes' types (ADR 0361, amended); any other flow, or a type the index cannot read, stays an obligation while any dispatch exists.

One shape is narrower than its type: a `dispatchEvent` that delivers to one known element joins only that element's handlers and fires no type program-wide. All of these hold:

1. the callee is the exact `EventTarget.dispatchEvent`;
2. the argument is an inline `new Event(T)`/`new CustomEvent(T, init)` of the `lib.dom` global, `T` a string literal, `init` absent or an object literal with static keys whose `bubbles` and `composed`, when written, are `false`;
3. `T` is not in the audited web release's delegated set (`Dialect::event_type_is_delegated`);
4. the receiver is a `const` local to a function, initialized to one intrinsic element (through `()`, `as`, `satisfies`, `!`), and every other reference is a member read;
5. the element's `on…` attributes for `T`, and the handlers of its spreads (an unresolved spread is unsupported), are all known.

`App.tsx` holds that shape (an unrelated handler's `onCleanup` is a proven SC4001, its write is quiet; the dispatched handler is an obligation; a project method spelled `click`/`focus` is no dispatch). `NestedPropLiteral` pins that only code written directly in an element's own handler is joined; `SharedHandler` that a function also used as an unrelated element's handler keeps its violation. A dispatch outside the narrow shape fires its type set program-wide, so each is its own project in `variants/`:

| variant | what it fires, and why not the narrow join |
| --- | --- |
| `bubbles` | `{ping}`: `bubbles: true` reaches ancestors; the unrelated click handler keeps its proof |
| `click-call` | `{click…}`: `el.click()` is delegated and bubbles |
| `delegated-type` | `{click}`: `click` is in `DelegatedEvents`; every click handler is an obligation |
| `unknown-receiver` | `{scroll}`: the receiver is a parameter |
| `unresolved-spread` | `{scroll}`: the element's spread names no handlers (helper call) |
| `ref-receiver` | `{scroll}`: the receiver is a `ref={el}` binding |
| `escaping-receiver` | `{scroll}`: the element is also rendered (`{el}`) |
| `shadowed-event` | all types: `Event` is a local class, not the `lib.dom` global |
| `init-spread` | `{ping}`: the init is not a static object |
| `rc13-targeted` | rc.13 reads the same `DelegatedEvents` (`dist/web.dev.js:36`): narrow join |
| `rc9-unread` | web rc.9's source was not read: every dispatch reaches every handler |
| `any-receiver` | `{scroll}`: `b.dispatchEvent(…)` on an `any` receiver resolves to no declaration |
| `cast-click` | `{click…}`: `(b as HTMLButtonElement).click()` is argumentless |
| `reflect-apply`, `bound-dispatch` | all types: a dispatching member taken as a value runs wherever it is called |
| `check-validity` | `{invalid}`: `input.checkValidity()`; the unrelated click handler keeps its proof |
| `object-handler` | not a dispatch: an object handler (`{ handleEvent, capture: true }`) is a capture listener and no proof site |

The type-set variants (`event-elements.d.ts` adds `input`, `section`, `form`; each has a `Reached` or listed handlers that stay obligations and handlers of other types that keep their proof, with SC2001 write twins):

| variant | what it pins |
| --- | --- |
| `focus-call` | `focus()` fires `focus focusin blur focusout change`: those handlers are obligations; click, scroll, input handlers keep their proof |
| `click-checkbox`, `click-submit` | `click()` fires its activation events (`input change`; `submit formdata invalid reset`); scroll keeps its proof |
| `abort-controller` | `abort`/`open`/`send`/`FileReader.abort` fire at no element: every handler keeps its proof |
| `custom-type-dispatch` | `dispatchEvent(new CustomEvent("sync"))` on a parameter fires `{sync}`: no handler of that type, every handler keeps its proof |
| `custom-type-reached` | the same with `onSync` handlers: they are obligations, the rest keep their proof |
| `dispatch-unknown-type` | `dispatchEvent(ev)` with `ev` a parameter fires any type: every handler is an obligation |
| `computed-member-call` | `target[name]()` on an `any` receiver names no member: any type |
| `computed-member-benign` | `fns[i]()` (index typed `number`), `loose[Symbol.asyncIterator]()` and a `Record` callee of a typed signature name no member: nothing fires |
| `computed-literal-member` | `loose["focus"]()` is the `focus` set |
| `unresolved-focus-spelling` | `loose.focus()` on an `any` receiver is the `focus` set |
| `spread-key-type` | a spread holder's functions are typed by key: `onFocus` is reached, `onScroll` is not |
| `mouse-click`, `pointer-click` | `dispatchEvent(new MouseEvent("click"))` runs the checkbox's activation behavior (`input change`): the `onChange` handlers are obligations, scroll keeps its proof |
| `custom-event-click` | contrast: `new CustomEvent("click")` (exactly `Event`/`CustomEvent`) is no activation event: `{click}` alone, `onChange` keeps its proof |
| `request-submit-dialog` | `requestSubmit()` fires `beforetoggle` (`method=dialog` closes the dialog): `onBeforeToggle` handlers are obligations |
| `anchor-fragment-click` | `a.click()` fires `beforematch` (a same-document fragment reveals `hidden=until-found`): `onBeforeMatch` handlers are obligations |
| `key-union`, `key-generic` | `target[k]()` with `k: "click" \| "focus"` or `K extends …` resolves to no target set: any type, `onFocus` and the rest are obligations |
| `owned-member-value`, `owned-member-reflect` | `dialog.showModal.call(dialog)` and `Reflect.apply(dialog.showModal, …)` run an owner-specific member as a value: any type |
| `owned-member-read` | contrast: `dialog.open` and a project `store.close.call(dialog)` are no dispatch: every handler keeps its proof |

Forwarded callbacks (each variant's `package.json` closes the program, which the walk needs; a `focus()` site fires the focus move):

| variant | what it pins |
| --- | --- |
| `forwarded-type-join` | a literal forwarded through a component prop to `onScroll` takes `scroll`: its `onCleanup` is a proven violation and its write is legal; one forwarded to `onFocus` keeps both obligations |
| `forwarded-two-terminals` | forwarded to an `onScroll` and an `onFocus`: the union is reached, obligations |
| `forwarded-unknown-terminal` | the prop is stored where the walk cannot follow it: no terminal, no proof |
| `forwarded-override` | the terminal `onScroll` is followed by an unread spread that may replace it (ADR 0364): no terminal, obligations |
| `forwarded-narrow-site` | a narrow `dispatchEvent(new Event("sync"))` on the element the literal is forwarded to keeps it an obligation; the same forward to another element is a violation |

A dispatch on a provably ownerless stack is no site (ADR 0365; same `Handlers` in each: a click, a focus and a change handler with `onCleanup`, and a click write):

| variant | what it pins |
| --- | --- |
| `dispatch-after-await` | `await load(); target.click()` (also inside `try`): nothing fires, every handler keeps its proof |
| `dispatch-skippable-await` | `if (wait) await load(); target.click()`: the await may be skipped, the `click()` set fires |
| `dispatch-in-{getter,constructor,jsx,static-block}-after-await` | the `click()` after the `await` is in a getter, a constructor, a JSX child or a static block: not the continuation's own flow, the `click()` set fires (the static block is a deliberate miss) |
| `dispatch-in-microtask` | `queueMicrotask`, `setTimeout`, `requestAnimationFrame` and `Promise.resolve(0).then` callbacks that dispatch: nothing fires |
| `dispatch-in-stored-closure` | `queueMicrotask(later)`, a closure the callback stores, a `then` on a parameter typed `Promise<void>`: the focus move fires |
| `dispatch-in-helper-after-await` | ai-memory-ui's `saveBlob`: a module-local helper every call of which follows a dominating `await`; nothing fires |
| `dispatch-helper-mixed-callers` | the same helper also called synchronously: the `click()` set fires |
| `dispatch-owner-restored` | `runWithOwner(owner, () => el.focus())` in a `setTimeout` callback and after an `await`: the focus move fires |

A dispatch on the global `window` or `document` is supported with any argument and an empty delivery set (no element is on the path; `Dialect::global_dispatch_reaches_no_element_handler`). Variants `window-dispatch`, `document-dispatch` (every handler keeps its proof), `shadowed-window` (`window` is a parameter: `{app:ping}`), `shared-handler` (a function that is an element handler and a `window` listener keeps its element's violation; a listener no element names stays conditional).

Stub: rc.17 typings transcribed in `solid-js.d.ts`; the rc.13 and rc.9 variants carry their own `node_modules` package files and reuse the declarations. `check-validity` and `object-handler` merge the extra elements they need through a local `elements.d.ts`.
