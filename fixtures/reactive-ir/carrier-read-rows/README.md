# carrier-read-rows

**Claim (ADR 1104).** A reactive read through a value an accessor or a root left in a carrier has an SC9012 row of its own at the
read, besides the carrier's row (the R-carrier patterns: Chrome reports STRICT_READ_UNTRACKED at the read, not at the carrier).

| component | rows past the carrier |
|---|---|
| `ElementCall` (`Object.values(o)[0]()`) | the element call |
| `ElementHeld` (twin: `typeof Object.values(o)[0]`) | none: only the carrier rows |
| `ForOf` (`for (const f of Object.values(accessors)) t += f()`) | `f()` |
| `ForOfHolder` (`for (const f of sources) t += f()`, `sources = [a, b]`) | `f()` |
| `ForOfHeld` (twin: the loop binding is never called) | none: only the carrier rows |
| `ThisGetter` (`get v() { return this.s(); }`) | `this.s()` |
| `HostRead` (`JSON.stringify(props)` in `Child`) | the standard call (and, by ADR 0860's hand-on, `console.log` of its result) |
| `Ambient` (`declare function ext`, `globalThis.ext = a`) | `ext()` |

| `LetForOf` (1b: `let f; for (f of src) f()`) | `f()` |
| `SelfAlias` (1b: `const self = this; self.s()`) | `self.s()` |
| `NestedThis` (1b: `this.inner.s()`) | `this.inner.s()` |
| `DefinedGetter` (1b: `Object.defineProperty(o, "v", { get() { return this.s(); } })`) | `this.s()` |
| `AmbientConst`, `AmbientVar` (1b: `declare const`, `declare var`) | `extC()`, `extV()` |
| `OtherSlot` (1b twin: `this.k()`, `k` not the accessor's slot) | none past the carrier |
| `PropsLoops` (1b twins: `Object.entries(props)` key `key.length`, value `Math.min(10, v)`; `Object.values(props)` summed in a memo read by `m()` in JSX) | none at the key, the value or `m()` |

Every row is uncertifiable; no violation. `solid-js.d.ts` is `async-creation-pass`'s. With this stub's types the two `for…of` calls already had a `type-only-source` row before ADR 1104; the walk past the loop binding is shown by the pattern programs it closed (`values/d3-object-values`, `values/d3-sources-const`, `returns/v11-for-of`, in real Chrome against the real packages).
