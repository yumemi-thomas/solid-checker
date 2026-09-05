// The runtime the `creates` implementation census walks.
//
// Every export here is a plain function of a *consuming* package — this
// package defines no Solid primitive — and every one except `unresolved`
// proposes `creates: []` (the generator's own walk finds nothing a closed
// `creates` would contradict, so it proposes the claim; `unresolved` calls an
// identifier the walk cannot resolve, and an unresolved callee is never
// evidence of harmlessness, so nothing is proposed for it; see `README.md`).
// What differs is whether the certifier's census can *prove* the claim: the
// census dispositions every call the export reaches, transitively through
// module-local helpers, and refuses the domain by name on the first premise it
// cannot establish.
//
// Dispositions exercised, one per helper (`docs/adr/0008-…`):
//   parameter-rooted   `callback(0)` — the callee is a caller-supplied callable
//   standard-library   `Array.from(...)`, `values.map(...)` — default-library
//                      members, resolved by symbol identity
//   local-recursion    `mapAll(...)`, `one()` → `two()` → `three()` — helpers of
//                      this module, censused through their own transcripts
//   unreachable        `never()` after an unconditional `return`
//
// And the constructs whose *lower bound* alone the producer cannot model —
// `loopCall`, `switchBreak`, `whileBreak` — which certify: every call inside
// them is on the wire with `reach: unknown`, which the `MayExecute` floor
// admits.
//
// Refusals exercised, each named by the census:
//   cycle              `cycleA` ↔ `cycleB`
//   depth              `deep` → nine hops, past the depth-8 bound
//   unresolved callee  `externalGlobal(...)`, an identifier no declaration binds
//   uncensused form    a tagged template, and `spreadUntyped`'s spread of an
//                      `any` operand (which drives the iteration protocol over
//                      a type that enumerates no iterator); `spreadArgs` is
//                      its cleared pair
//   unaccounted flow   `labelledBreak` — `break outer` out of a plain labelled
//                      block, which no enclosing construct of the frame owns,
//                      so the control-flow census classifies it
//                      `flow-unaccounted` and the census refuses it
//   unseen callable    `stdlibRefInvoker` hands a module-local function
//                      *reference* to `forEach`; `reflectApply` transfers
//                      control through `Reflect.apply`
//   written binding    `reassignedHelper` calls a `function helper` that a
//                      later statement reassigns

function never() {}

function mapAll(items, callback) {
  const values = Array.from(items);
  return values.map(callback);
}

export function plain(items, callback) {
  callback(0);
  return mapAll(items, callback);
  never();
}

function three(callback) {
  callback();
}

function two(callback) {
  three(callback);
}

function one(callback) {
  two(callback);
}

export function viaHelperChain(callback) {
  one(callback);
}

// The boolean stop value makes the mandatory sample finite without adding a
// coercing operator over an untyped JavaScript parameter. The static graph
// still contains the cycle cycleA -> cycleB -> cycleA.
function cycleA(done) {
  if (done) return;
  cycleB(true);
}

function cycleB(done) {
  if (done) return;
  cycleA(true);
}

export function cycle() {
  cycleA(false);
}

function hop9() {}
function hop8() {
  hop9();
}
function hop7() {
  hop8();
}
function hop6() {
  hop7();
}
function hop5() {
  hop6();
}
function hop4() {
  hop5();
}
function hop3() {
  hop4();
}
function hop2() {
  hop3();
}
function hop1() {
  hop2();
}

export function deep() {
  hop1();
}

// An identifier nothing declares. Not an import from an unaudited dependency,
// deliberately: a bare import that resolves to no accepted dependency is an
// `UnacceptedExternalDependency` closure hazard, which opens every domain of
// the artifact case at closure replay — before any candidate exists for a
// census to refuse. This callee reaches the census with no declaration and no
// parameter root, and is refused there, by name.
export function unresolved(value) {
  return externalGlobal(value);
}

function tag(strings) {
  return strings;
}

export function taggedTemplate() {
  return tag`literal`;
}

function joinAll(first, second) {
  return [first, second];
}

export function spreadArgs(...args) {
  return joinAll(...args);
}

// The same spread, one line apart, over an operand whose type the producer
// cannot decompose. `items` is an ordinary unannotated parameter, so it is
// `any`, and `any` enumerates no members at all: "the checker could not find
// `[Symbol.iterator]`" is never "iterating this reaches no user code", so the
// producer records the form and the census refuses. `spreadArgs` above is the
// pair, and the *only* difference is the parameter's own type — a rest
// parameter is `any[]`, an array, whose iterator is the engine's however its
// elements are typed.
export function spreadUntyped(items) {
  return joinAll(...items);
}

// Byte-for-byte the body of `plain`, under another name: the census proves it
// the same way, and the difference between the two rows is whether the recipe
// corpus carries a recipe for *this* export's claim.
export function noRecipe(items, callback) {
  callback(0);
  return mapAll(items, callback);
  never();
}

// A helper with nothing to refuse: the point of the four exports below is what
// happens *around* the call to it, not inside it.
function mount(el) {
  return el;
}

// A loop with no jump in it at all. The producer cannot give a *lower* bound on
// reachability inside a loop body — control may not enter it — so the
// control-flow census reports `iterationReachability`, classified
// `reachability-lower-bound`: the construct is walked in full and `mount(el)` is
// on the wire with `reach: unknown`. That is everything a census of the
// callables this body can reach needs, so it **certifies**.
//
// This export is the direct measurement of ADR 0008 item 0. Before the producer
// classified its markers, a body shaped exactly like this — `flatten` and
// `chainedTranslator` in `@solid-primitives/i18n` — refused for a marker left by
// a loop that withheld nothing.
export function loopCall(el) {
  while (el) {
    mount(el);
  }
}

// The `break` is owned by the `switch` it sits in, which is what makes its
// target resolvable: the producer covers the whole `switch` as the region the
// jump makes non-universal, reduces `mount(el)`'s reach to `unknown` there, and
// **states the row**. It used to *drop* it — and since a dropped
// `CallExpression` leaves no uncensused-form row either, the only trace was the
// `switchReachability` marker, so the census had to refuse every marker or
// close `creates` over a call that runs. Now the row is what the census reads,
// and this export certifies with `mount` dispositioned by local recursion.
export function switchBreak(kind, el) {
  switch (kind) {
    case "mount":
      mount(el);
      break;
  }
}

// The same, inside a loop: the `break` is owned by the `while`, the row is
// stated at `unknown`, and the export certifies.
export function whileBreak(el) {
  while (el) {
    mount(el);
    break;
  }
}

// A construct whose flow the producer genuinely cannot account for. `break
// outer` leaves a plain labelled *block*, and no enclosing loop or `switch` of
// this frame owns that target — which is what bounds every region-based repair
// the two censuses apply to a jump, and what
// `constructCompletesNormallyLocked` reasons about. So the marker is
// `jumpReachability`, classified `flow-unaccounted`, and the census refuses by
// marker and location. This is the arm that keeps the relaxation above from
// being a blanket one: `mount(el)` is on the wire here too, and the refusal is
// not about a missing row but about a frame whose control flow nobody modelled.
export function labelledBreak(el) {
  outer: {
    mount(el);
    break outer;
  }
}

function work(value) {
  return value;
}

// `Array.prototype.forEach` is a reviewed default-library invoker of its slot 0,
// and `work` is a module-local function *reference*: not a parameter of this
// implementation, not a callable literal inside it, and a declaration the
// producer's argument tracer does not follow (it follows `const` bindings
// only). The standard-library disposition's premise covers the engine's own
// body, not the code the engine runs on the census's behalf, so this refuses.
export function stdlibRefInvoker(items) {
  Array.from(items).forEach(work);
}

// `Reflect.apply` runs whatever sits in its first slot, however it got there.
// The member itself is refused by qualified name, whatever the slots prove.
export function reflectApply(args) {
  return Reflect.apply(work, undefined, args);
}

// The producer resolves `helper(el)` to the `function helper` declaration —
// that is the symbol's declaration, and it stays so after the reassignment
// below. What runs is the arrow. The verifier's own parse of these bytes finds
// the write and refuses to walk a declaration that is not proven to be the
// code the call runs.
function helper(el) {
  return el;
}

helper = (el) => mount(el);

export function reassignedHelper(el) {
  return helper(el);
}

// The two exports below are why the generator's `creates` walk does **not**
// align its unresolved-member declines with this census, though the corpus
// shape ranking made it look as if it could. Each is a shape the ranking called
// decidable or spurious, and each is refused here — so a walk that proposed
// them would plan a candidate this census refuses at witness acquisition,
// turning a certified row into a refused one.

// A member of *this export's own parameter*: the producer does state
// `calleeParameter` (parameter 0, path `["read"]`), so the `parameter-rooted`
// disposition would decide this call. The export still refuses, and on a
// different premise — reading `.read` off a value whose type is unknown is an
// **uncensused invoking form** (`property-access-unknown-accessor`), recorded
// exactly when the compiler resolves no symbol for the property, which is the
// same condition that makes the generator's walk decline the callee. A `.d.ts`
// `read(): unknown` may perfectly well describe a `.js` getter, so absence of a
// symbol is not evidence of a plain data property. This is the pin that keeps
// the generator's walk from proposing it.
export function memberParameterRooted(source) {
  return source.read();
}

// An immediately-invoked function expression. Its body is lexically inside this
// export and every call in it is already walked, so the generator has no
// counterexample to name — and the census still **refuses** it, by name, as an
// unresolved callee: the producer resolves the transcript row's callee to
// nothing. That is why the walk keeps declining `expression-callee` rather than
// treating it as spurious.
export function iife(value) {
  return (function () {
    return value;
  })();
}
