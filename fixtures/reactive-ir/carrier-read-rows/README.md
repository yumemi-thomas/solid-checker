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

Every row is uncertifiable; no violation. `solid-js.d.ts` is `async-creation-pass`'s. With this stub's types the two `for…of` calls already had a `type-only-source` row before ADR 1104; the walk past the loop binding is shown by the pattern programs it closed (`values/d3-object-values`, `values/d3-sources-const`, `returns/v11-for-of`, in real Chrome against the real packages).
