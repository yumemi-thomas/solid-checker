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
// Refusals exercised, each named by the census:
//   cycle              `cycleA` ↔ `cycleB`
//   depth              `deep` → nine hops, past the depth-8 bound
//   unresolved callee  `externalGlobal(...)`, an identifier no declaration binds
//   uncensused form    a tagged template, and a spread argument (which drives
//                      the iteration protocol)
//   withheld rows      `switchBreak`, `whileBreak` — a `break` makes the
//                      producer drop the `mount(el)` call row and leave only a
//                      control-flow marker, which the census refuses on
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

// Unconditional mutual recursion, on purpose: a guard such as `depth > 0`
// would be an operator application on an untyped operand, which the producer
// records as a `coercion` form, and the census would refuse on that row before
// it ever reached the cycle. Nothing runs this; the census reads it.
function cycleA() {
  cycleB();
}

function cycleB() {
  cycleA();
}

export function cycle() {
  cycleA();
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

// The producer drops every `calls` row inside the region a `break` makes
// non-universal — here the whole `switch` — so `mount(el)` leaves **no row**.
// The dropped call is a `CallExpression`, so the uncensused-form census records
// nothing either; the only trace is the `switchReachability` marker in the
// control-flow census. A census that relaxed that marker would close `creates`
// over a call that runs. This must refuse.
export function switchBreak(kind, el) {
  switch (kind) {
    case "mount":
      mount(el);
      break;
  }
}

// The same withholding inside a loop: the `break` drops the `mount(el)` row and
// leaves the `iterationReachability` marker. This must refuse.
export function whileBreak(el) {
  while (el) {
    mount(el);
    break;
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
