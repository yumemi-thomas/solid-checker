# dispatch-delivery-join

**Claim (ADR 0361).** A synchronous `dispatchEvent` runs handlers on its caller's stack, under its owner (ADR 0356). A dispatch is joined to the handlers it can reach when all of these hold; any other dispatch (or `click()`) makes every handler in the program conditional, as before:

1. the callee is the exact `EventTarget.dispatchEvent`;
2. the argument is an inline `new Event(T)`/`new CustomEvent(T, init)` of the `lib.dom` global, `T` a string literal, `init` absent or an object literal with static keys whose `bubbles` and `composed`, when written, are `false`;
3. `T` is not in the audited web release's delegated set (`Dialect::event_type_is_delegated`; a release not read delegates every type);
4. the receiver is a `const` local to a function, initialized to one intrinsic element (through `()`, `as`, `satisfies`, `!`), and every other reference is a member read;
5. the element's `on…` attributes for `T`, and the handlers of its spreads (an unresolved spread is unsupported), are all known.

The delivery set is those handlers: they stay obligations (SC4001/SC2001 uncertifiable). Every other element's handler keeps its proof.

This root holds the supported shapes (`App.tsx`: an unrelated handler's `onCleanup` is a proven SC4001, its write is quiet; the dispatched handler is an obligation; a project method spelled `click`/`focus` is no dispatch, so its component's handler keeps its proof). `NestedPropLiteral` pins that only code written directly in an element's own handler is joined (a literal handed to a component is delivered through that component's handler: its write stays an obligation); `SharedHandler` that a function also used as an unrelated element's handler keeps its violation. A site that is not supported makes every handler conditional, so each such shape is its own project in `variants/`. The last six hold a supported dispatch beside the shape, so the shape alone is what conditions the program:

| variant | why the dispatch is unsupported |
| --- | --- |
| `bubbles` | `new CustomEvent("ping", { bubbles: true })` reaches ancestors |
| `click-call` | `el.click()` is delegated and bubbles |
| `delegated-type` | `new Event("click")`: `click` is in `DelegatedEvents` |
| `unknown-receiver` | the receiver is a parameter |
| `unresolved-spread` | the element's spread names no handlers (helper call) |
| `ref-receiver` | the receiver is a `ref={el}` binding (not supported) |
| `escaping-receiver` | the element is also rendered (`{el}`), not only read through members |
| `shadowed-event` | `Event` is a local class, not the `lib.dom` global |
| `init-spread` | the init is not a static object literal |
| `rc13-targeted` | rc.13 reads the same `DelegatedEvents` (`dist/web.dev.js:36`): targeted |
| `rc9-unread` | web rc.9's source was not read for it: every type delegated, conditional |
| `any-receiver` | `b.dispatchEvent(…)` on an `any` receiver resolves to no declaration: any handler may run |
| `cast-click` | `(b as HTMLButtonElement).click()` is argumentless: no resolved call, still a dispatch |
| `reflect-apply` | `Reflect.apply(EventTarget.prototype.dispatchEvent, b, [e])` dispatches without calling the member |
| `bound-dispatch` | `const fire = b.dispatchEvent; fire.call(b, e)`: the member is a value |
| `check-validity` | `input.checkValidity()` fires `invalid` synchronously (ADR 0329): a dispatch no site models |
| `object-handler` | not a dispatch: an object handler (`{ handleEvent, capture: true }`) is a capture listener and no proof site; its body has no finding while the function handler beside it keeps its violation |

Stub: rc.17 typings transcribed in `solid-js.d.ts`; the rc.13 and rc.9 variants carry their own `node_modules` package files and reuse the declarations (nothing they use differs). `check-validity` and `object-handler` merge the extra intrinsic elements they need (`input`, `section`) through a local `elements.d.ts`.
