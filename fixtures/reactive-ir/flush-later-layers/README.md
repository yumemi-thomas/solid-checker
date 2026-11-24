# flush-later-layers

**Claim (ADR 1206).** A `flush()` in exact in-place layers (ADR 1090's clean climb: closures and helpers called once, IIFEs, up to sixteen layers, more through ADR 1199's entries) whose outermost call runs past a suspension of its own async function no await dominates (ADR 1201: a counted loop's later pass, the `finally` behind a `try` head's await that cannot throw first) runs past it too. The flush census asks the continuation question at the climb's end when that call has no census class (`climbed_caller`), and so does the effect-phase walk's unplaced SC2008 arm (`flush_runs_off_any_delivery`, for a compute handed by name). The program steps an action, runs an effect's apply and opens a leaf scope, so no program-wide reason ends any site here.

Clean: `WrapperNamed`, `Chain`, `DeepFinally` (15 IIFEs), `Deep17` (17 IIFEs: past the climb's bound of 16, but ADR 1199 takes the innermost IIFE's one call as its entry and the climb from there crosses the rest).

Uncertifiable (fail closed): `HandedOn` (the closure is also pushed into an array: a run of it may be anywhere), `AlsoFirstPass` (the helper is also called on the first pass: no single-call layer, and its other call runs before the await).
