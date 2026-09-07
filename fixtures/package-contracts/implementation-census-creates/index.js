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
//   value-initialized  `callInitialized` calls a `const` holding what `wrap`
//                      returned, not a function literal
//
// And the one indirection the census does take since 2026-09-06: `constBound`
// calls an arrow a `const` holds, followed through its unwritten,
// once-declared binding.
//
// And the premise the form census classifies under since ADR 0038: the
// export's declared signature in `index.d.ts`. `typedCoercion`,
// `returnedCallbackCoercion` and `declaredMemberCoercion` certify on
// coercions that were `any` before; `untypedCoercion` (declared `unknown`) and
// `helperCoercion` (the coercion is a helper's, which has no declaration)
// refuse and pin the boundary.

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

// Byte-for-byte the body of `plain` once more, declared with two overloads in
// `index.d.ts` and with no hand recipe: the export has no *one* call signature
// to synthesize from, only a complete overload set, and synthesis samples
// every member of it. `seed` is unused on purpose — the second overload's
// extra parameter is a call shape, not behavior.
export function overloaded(items, callback, seed) {
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

// An arrow a `const` holds. The producer resolves `boundHelper(callback)` to
// the arrow (or, from another module, to the identifier `boundHelper`); the
// verifier's own parse binds the arrow to the declarator that holds it, finds
// the identifier unwritten and declared once, and walks the arrow's transcript
// exactly as it walks `function helper` — since 2026-09-06. Before that the
// census refused every callable a variable held, which was most of the
// module-local helpers in bundled output (`const isMotionValue = (value) =>`).
const boundHelper = (callback) => callback(0);

export function constBound(callback) {
  return boundHelper(callback);
}

// The same binding shape with an initializer that is not a function literal.
// Whatever `wrap` returns is what runs, and the census does not trace values,
// so the call refuses by name.
function wrap(fn) {
  return fn;
}

const wrappedHelper = wrap(() => 0);

export function callInitialized() {
  return wrappedHelper();
}

// A member of *this export's own parameter*. The call is `parameter-rooted`
// (the producer states `calleeParameter`, parameter 0, path `["read"]`), and
// since ADR 0034 the read of `.read` off a value whose type is unknown — an
// `uncensused invoking form`, `property-access-unknown-accessor` — is
// dispositioned `parameter-rooted-accessor` too: the producer states that its
// subject is rooted at parameter 0, a plain, unwritten binding of this very
// declaration, so whatever getter or trap that read reaches sits on an object
// the caller handed over. The export **certifies**; the generator's walk now
// proposes the direct-parameter member callee to match. The exports that
// follow pin the boundary of that disposition, one premise each.
export function memberParameterRooted(source) {
  return source.read();
}

// ADR 0034's this-protocol table: `Object.prototype.toString` reaches user code
// only through `Get(this, @@toStringTag)`, and `value` is an unwritten parameter,
// so the by-reference owner rule is decided before it refuses. **Certifies.**
export function toStringTagViaCall(value) {
  return Object.prototype.toString.call(value) === "[object String]";
}

// A parameter written before the read: the object read from is not the one the
// caller handed over. The producer states no subject parameter; **refuses**.
export function writtenBeforeRead(source) {
  source = registryObject;
  return source.value;
}

// A parameter written *after* the read. The disposition is deliberately not
// flow-sensitive — an unwritten binding is the premise ADR 0029 reviewed — so
// this **refuses** too, and says why the `scrollIntoView` pair still does.
export function writtenAfterRead(source) {
  const seen = source.value;
  source = registryObject;
  return seen;
}

// A read on a module-level value whose type the compiler does not know: the
// member is an uncensused form with no parameter root. **Refuses.** (A module
// object literal's own data property would resolve and record no form at all;
// the `any` cast is what makes this the accessor question rather than a bound
// data property.)
export function moduleReceiverRead() {
  return untypedRegistry.value;
}

// A read through a *nested callable's own* parameter. `items.map` is the
// export's parameter and its read is rooted; `item.value` is the arrow's
// parameter, which is the invoker's value, not this invocation's. **Refuses.**
export function nestedCallableParameterRead(items) {
  return items.map((item) => item.value);
}

// `.call` on a receiver that is not a default-library member: the by-reference
// owner rule stands. **Refuses.**
export function callNonLibraryReceiver(value) {
  return identity.call(value);
}

// `.call` on a default-library receiver outside the reviewed this-protocol
// table: `Array.prototype.slice` may read length and indices of its `this`, a
// reach nobody reviewed. **Refuses.**
export function callLibraryOutsideTable(value) {
  return Array.prototype.slice.call(value);
}

// An accessor in write position on the parameter. The setter that may run was
// installed by the caller on the object it passed, so it is the caller's code
// exactly as its getter would be (ADR 0040); the receipt names the site
// `parameter-rooted-accessor-write`, which is what a future `writes` census
// must refuse. **Certifies.**
export function setterOnParameter(source) {
  source.value = 1;
}

// A compound assignment runs the caller's getter and then the caller's setter,
// both on the object the caller passed. Same premise, one site.
// **Certifies.**
export function updateOnParameter(source) {
  source.value += 1;
}

// ADR 0042: the whole `chain` shape, which five packages in the ecosystem
// publish verbatim. Three premises meet here and none of them is about this
// module's own code: `callbacks` is the caller's iterable, so its
// `Symbol.iterator` and the `next` calls after it are the caller's; `args` is a
// rest parameter, whose array the engine itself builds, so spreading it reaches
// `Array.prototype` and nothing else; and `callback` is a value that iterable
// yielded, so calling it runs the caller's code exactly as calling a parameter
// would. **Certifies.**
export function chainCallbacks(callbacks) {
  return (...args) => {
    for (const callback of callbacks) callback && callback(...args);
  };
}

// The same loop over a value this module made. Nothing roots the iterable at a
// parameter, so neither the iteration nor the callee has a premise.
// **Refuses.**
const moduleCallbacks = [];
export function chainModuleCallbacks() {
  for (const callback of moduleCallbacks) callback();
}

// A `for await…of` over the caller's iterable. The async iteration protocol
// reaches `Symbol.asyncIterator` and the promise machinery, which no ADR has
// reviewed, so the form refuses whatever it is rooted at. **Refuses.**
export async function awaitIterateParameter(callbacks) {
  for await (const callback of callbacks) void callback;
}

// ADR 0041: an object spread reads every own enumerable property of its
// operand, invoking each getter among them. The operand is the object the
// caller passed, so the getters are the caller's exactly as a named read's
// would be. **Certifies.**
export function spreadParameter(source) {
  return { ...source };
}

// The same spread on a *written* parameter. The binding may hold something
// other than the caller's argument by the time it is read, so no premise
// covers it and the form refuses. **Refuses.**
export function spreadWrittenParameter(source) {
  source = source ?? {};
  return { ...source };
}

// A destructuring of a parameter-rooted value: each binding element reads a
// property of the caller's object, and the rest element reads whatever own
// properties remain of it. One premise, one site per element. **Certifies.**
export function destructureParameter(source) {
  const { first, ...rest } = source;
  return first === undefined ? rest : first;
}

// A destructuring of a value this module made. Nothing roots it at a
// parameter, so the reads refuse exactly as they did before ADR 0041.
// **Refuses.**
export function destructureModuleValue() {
  const { value } = untypedRegistry;
  return value;
}

// The same write, on the module-level untyped value `moduleReceiverRead` reads.
// Nothing roots it at a parameter, so no premise covers the accessor and the
// form refuses exactly as it did before ADR 0040: the boundary is provenance,
// not position. **Refuses.**
export function setterOnModuleValue() {
  untypedRegistry.value = 1;
}

// ADR 0038: the form census is classified under the export's *declared*
// signature. `index.d.ts` types every parameter below, and the unannotated
// JavaScript parameter that was `any` — so that `v > max` might reach a
// `valueOf` — is `number` under the premise, which no coercion can reach. The
// certificate records the premise; the four exports after this one pin its
// boundary.
export function typedCoercion(min, max, v) {
  return v > max ? max : v < min ? min : v;
}

// The declared type is `unknown`, which is not provably a non-object: the
// premise binds and the coercion stands under it. **Refuses.**
export function untypedCoercion(value) {
  return value + 1;
}

// The declared signature's *return type* types the returned arrow's parameter
// contextually — `(p: number) => number` — so `p * step` clears too.
// **Certifies.**
export function returnedCallbackCoercion(step) {
  return (p) => p * step;
}

// A declared object type. The reads of `.max` and `.min` stay unknown
// accessors — a declaration file is not runtime bytes — and are dispositioned
// as parameter-rooted (ADR 0034); the subtraction's operands are `number`
// under the premise. **Certifies.**
export function declaredMemberCoercion(axis) {
  return axis.max - axis.min;
}

// The coercion sits in a local helper, whose parameters have no declared
// signature. The premise reaches it anyway (ADR 0038, helper premises): the
// root's premised census records the argument types at `subtract(a, b)` —
// `number`, `number` under the declared signature — the verifier demands the
// helper's transcript under exactly those types, and `x - y` clears on the
// helper's own twin. The receipt carries a `census-premise:` site for the
// helper's parameters too. **Certifies.**
function subtract(x, y) {
  return x - y;
}

export function helperCoercion(a, b) {
  return subtract(a, b);
}

// A spread displaces every slot at or after it, so the call carries no
// argument premise; the helper is censused over its own `any` and `x - y`
// refuses at depth 1. The spread itself clears — under the premise `[a, b]`
// is a `number[]`, an engine container. **Refuses.**
export function helperSpreadCoercion(a, b) {
  return subtract(...[a, b]);
}

// One slot is `any` — `JSON.parse` returns `any` — so the premise names slot 0
// only; `y` stays `any` on the helper's twin and `x - y` refuses at depth 1.
// **Refuses.**
export function helperUntypedArgument(a) {
  return subtract(a, JSON.parse("1"));
}

const registryObject = { value: 1 };
const untypedRegistry = /** @type {any} */ (registryObject);

function identity() {
  return this;
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

// ---------------------------------------------------------------------------
// ADR 0043: the root set is closed under the reads this census already
// dispositions. Naming an intermediate does not change whose value it is, so
// each of the four legs below reaches exactly what the ADR 0034 spelling of
// the same value reaches — and the one leg that is a *different* claim, a
// parameter's default, says so in its receipt site.
// ---------------------------------------------------------------------------

// A parameter carrying a default that names another rooted parameter. The
// value is the caller's argument at this slot when one was passed and the
// caller's argument at `axis` when none was, so the accessor is the caller's
// code under either branch. The site records `parameter-default`, not
// `parameter`. **Certifies.**
export function defaultedFromParameter(axis, sourceAxis = axis) {
  return sourceAxis.min;
}

// A parameter whose default is a value *this* module made. When the caller
// omits the argument the accessor that may run is this module's own, which is
// exactly what ADR 0034 excluded a defaulted parameter for. **Refuses.**
//
// The default is the untyped module value rather than an object literal, and
// the trap is worth naming: written `source = { value: 1 }` the compiler binds
// `value` as a **data property of a literal in this file**, so it records no
// form at all and the export certifies without the census ever reaching the
// premise — the same vacuity `setterOnModuleValue` guards against.
export function defaultedFromModuleValue(source = untypedRegistry) {
  return source.value;
}

// A default naming a parameter that is *itself* defaulted. A second hop this
// ADR does not review: only a source rooted as a plain parameter carries the
// premise forward. **Refuses.**
export function defaultedFromDefaulted(axis, mid = axis, tail = mid) {
  return tail.min;
}

// An object binding pattern in *parameter* position, with no default on the
// parameter and none on the element. The pattern reads a property of the
// caller's argument, so what it binds is the caller's — the same value
// `f(source)` plus `source.inner` names. **Certifies.**
export function patternParameter({ inner }) {
  return inner.value;
}

// The same pattern carrying a *parameter* default. When the caller omits the
// argument the pattern destructures an object this code wrote, so `inner` may
// hold this module's own value. **Refuses.**
export function patternParameterDefault({ inner } = untypedRegistry) {
  return inner.value;
}

// A binding element carrying *its own* default, for the same reason one slot
// down. **Refuses.**
export function patternElementDefault({ inner = untypedRegistry }) {
  return inner.value;
}

// A *rest* element of a parameter pattern. The object is one the engine built
// with CopyDataProperties rather than one the caller passed, so ADR 0043 does
// not root it at the parameter — and ADR 0044 roots it as an **own literal**
// for exactly that reason: every own property of it is a data property. Its
// sibling `first` is the caller's; `rest` is this program's; both clear.
// Refused between the two ADRs, **certifies** since ADR 0044.
export function patternRestParameter({ first, ...rest }) {
  return first === undefined ? rest.value : first;
}

// A local bound from an already-rooted read, twice over: `inner.value.text`
// spelled across two declarations reaches exactly what `source.inner.value`
// reaches, which ADR 0034 dispositions. Pins the fixpoint — the second
// declaration roots only once the first has. **Certifies.**
export function localBindingFromParameter(source) {
  const inner = source.inner;
  const leaf = inner.value;
  return leaf.text;
}

// A local bound by a *pattern* over a rooted value: the same fact by the same
// route, one property deep. **Certifies.**
export function localPatternFromParameter(source) {
  const { inner } = source;
  return inner.value;
}

// A local the declaration *writes*. The binding may hold something other than
// the value it was initialized with by the time it is read, so nothing roots
// it — the premise is not flow-sensitive, and says so. **Refuses.**
export function localBindingWritten(source) {
  let inner = source.inner;
  inner = untypedRegistry;
  return inner.value;
}

// A local bound from a *call result*. What the call returned is rooted at
// nothing, so the read of it refuses exactly as it did before. **Refuses.**
export function localBindingFromCall(source) {
  const inner = JSON.parse(source.text);
  return inner.value;
}

// ---------------------------------------------------------------------------
// ADR 0044: a value *this program* built. Every own property of an object or
// array literal is created with CreateDataPropertyOrThrow, so reading any
// member of one — a computed key included, which is exactly the case the
// checker resolves no symbol for — reaches a data property or the engine's own
// prototype chain. The census already takes this premise wherever the key is a
// literal, silently, by recording no form; these exports make it explicit.
// ---------------------------------------------------------------------------

const lookupTable = { first: { value: 1 }, second: { value: 2 } };
const orderedKeys = ["first", "second"];
const accessorTable = {
  get first() {
    return 1;
  },
};
const protoTable = { __proto__: untypedRegistry, first: 1 };
let mutableTable = { first: 1 };
mutableTable = untypedRegistry;

// A computed key on a module-level object literal. **Certifies.**
export function ownTableRead(key) {
  return lookupTable[key];
}

// The same on an array literal: every element is created by index.
// **Certifies.**
export function ownArrayRead(index) {
  return orderedKeys[index];
}

// The same in *write* position. Setting a member of a data-only object runs no
// user code either — and where the key reaches `Object.prototype`, the one
// accessor there is the engine's. **Certifies.**
export function ownTableWrite(key) {
  lookupTable[key] = { value: 1 };
}

// A local built by an object pattern's **rest** element: CopyDataProperties
// creates data properties whatever the source held, which is the same fact
// ADR 0043 excludes a rest element from *parameter* rooting for. The spread of
// it is the form that clears. **Certifies.**
export function ownRestSpread(source) {
  const { first, ...rest } = source;
  return first === undefined ? { ...rest } : first;
}

// A literal that installs a **getter**. Reading a member of it may run this
// module's own accessor, which is the one thing the premise rules out.
// **Refuses.**
export function accessorTableRead(key) {
  return accessorTable[key];
}

// A literal carrying a `__proto__:` member. That sets the prototype rather
// than a property, replacing the one chain this premise reasons about.
// **Refuses.**
export function protoTableRead(key) {
  return protoTable[key];
}

// A binding this module **writes**: it may hold something other than the
// literal by the time it is read. **Refuses.**
export function writtenTableRead(key) {
  return mutableTable[key];
}

// One level further in. What a data property *holds* is an arbitrary value, so
// the outer read refuses while the inner one clears — the premise roots a
// direct reference, never a chain. **Refuses.**
export function ownTableMemberRead(key) {
  return lookupTable[key].value;
}

// An **array** pattern's rest element. Its elements come from the source's
// iterator, which is the source's code, so this is an iteration question and
// no ADR has reviewed it. The pattern's own iteration clears — `source` is
// parameter-rooted — and the read of what it bound refuses. **Refuses.**
export function arrayRestRead(source) {
  const [, ...tail] = source;
  return tail[0];
}

// ---------------------------------------------------------------------------
// ADR 0045: a coercion over a call into this program's own runtime source. A
// call to a module-local helper types as `any` in compiled JavaScript however
// well the package's declarations type that helper, so a sum of one refused
// wherever the declared-signature premise had typed everything else. The form
// now names the calls its clearance rests on, and each callee's own census
// answers whether the value it hands back is provably a primitive.
// ---------------------------------------------------------------------------

// The helper must hand back its own argument, not an arithmetic result. Written
// `return value * factor` the completion is a `number` whatever the parameters
// are — `*` always yields one — so the call site is already a primitive, no
// coercion form is recorded, and every export below certifies without the
// census reaching the premise. That is the same vacuity ADRs 0043 and 0044
// each walked into, in its third dress.
function scaleBy(value) {
  return value;
}

function boxOf(value) {
  return { value };
}

let mutableScale = scaleBy;
mutableScale = boxOf;

// The shape the corpus is full of: a sum of a local helper's result and a
// declared `number`. The helper is censused under the argument types recorded
// at this very call, and under those its completion is `number`.
// **Certifies.**
export function coerceHelperResult(base) {
  return scaleBy(base) + base;
}

// The same through a local binding the file declares once, writes nowhere and
// initializes — naming an intermediate does not change where the value came
// from, exactly as in ADR 0043. **Certifies.**
export function coerceBoundHelperResult(base) {
  const scaled = scaleBy(base);
  return scaled + base;
}

// The same through both arms of a conditional. **Certifies.**
export function coerceConditionalHelperResult(base, factor) {
  const scaled = factor === 0 ? base : scaleBy(base);
  return scaled + base;
}

// A helper whose completion is an **object**. Coercing it reaches whatever
// `valueOf` or `toString` that object carries, which is this module's own code
// and exactly what the premise rules out. **Refuses.**
export function coerceObjectHelperResult(base) {
  return boxOf(base) + base;
}

// A call through a binding initialized by an **identifier** rather than a
// function literal: what runs is whatever `mutableScale` holds when the call
// executes, and this census does not trace values, so the *call* refuses
// before any coercion premise is reached. The premise cannot be granted for a
// callee the census could not bind, which is the point. **Refuses.**
export function coerceWrittenHelperResult(base) {
  return mutableScale(base) + base;
}

// A coercion over a call into the **default library**. `JSON.parse` is not a
// declaration in this program's runtime source, so no premise covers it and
// the form refuses — the boundary is provenance, and the standard-library
// disposition of the *call* says nothing about the value it returns.
//
// It must be a library member the declarations type as `any`. Written
// `Math.min(base, 1) + base` the operand is a declared `number`, so the
// classifier records **no form at all** and the export certifies without the
// census reaching the premise — the same vacuity ADRs 0043 and 0044 each
// walked into once. **Refuses.**
export function coerceLibraryResult(base) {
  return JSON.parse("1") + base;
}

// ---------------------------------------------------------------------------
// ADR 0047: `x instanceof C` performs GetMethod(C, @@hasInstance) and calls it
// when there is one; otherwise OrdinaryHasInstance reads `C.prototype` and
// walks x's prototype chain, which runs nothing. So the only question the
// operator ever asks is whose `Symbol.hasInstance` the **constructor** could
// carry, and the answer is a matter of provenance like every other in this
// family.
// ---------------------------------------------------------------------------

class OwnMarker {
  constructor(value) {
    this.value = value;
  }
}

class DerivedMarker extends OwnMarker {}

class ComputedMarker {
  static [Symbol.hasInstance](value) {
    return typeof value === "string";
  }
}

// A constructor the caller supplied. Whatever `Symbol.hasInstance` it carries,
// the caller installed it — the same argument that excuses a getter on an
// object the caller passed. **Certifies.**
export function instanceOfParameter(value, constructor) {
  return value instanceof constructor;
}

// The engine's own constructor: its `Symbol.hasInstance` is
// `Function.prototype`'s. **Certifies.**
export function instanceOfLibrary(value) {
  return value instanceof Error;
}

// A class this module declares, with no heritage clause and no computed
// member, so nothing on its prototype chain can carry the method.
// **Certifies.**
export function instanceOfOwnClass(value) {
  return value instanceof OwnMarker;
}

// A class with a **superclass**: `C[Symbol.hasInstance]` is looked up along
// C's own prototype chain, which runs through the superclass constructor, and
// that is a value this walk does not have. **Refuses.**
export function instanceOfDerivedClass(value) {
  return value instanceof DerivedMarker;
}

// A class carrying a **computed** member name, which is exactly how
// `[Symbol.hasInstance]` is written. **Refuses.**
export function instanceOfComputedClass(value) {
  return value instanceof ComputedMarker;
}

// A constructor read off a value this module made: nothing roots it, so no
// premise covers the operator. **Refuses.**
export function instanceOfModuleValue(value) {
  return value instanceof untypedRegistry.ctor;
}

// ---------------------------------------------------------------------------
// ADR 0048: what the caller's own function handed back is the caller's, by the
// argument ADR 0042 makes about what its iterable yielded.
// ---------------------------------------------------------------------------

// A read of the value a caller-supplied callee returned. **Certifies.**
export function readCallerResult(transform, point) {
  return transform(point).y;
}

// The same through a local binding, which is how compiled code writes it.
// **Certifies.**
export function readBoundCallerResult(transform, point) {
  const mapped = transform(point);
  return mapped.y;
}

// A call to a **module-local** helper: what it returned is this module's, and
// nothing here says its members are data properties. **Refuses.**
export function readLocalResult(point) {
  return identityOf(point).y;
}

function identityOf(value) {
  return untypedRegistry;
}

// ---------------------------------------------------------------------------
// ADR 0050: a binding the file **writes**, every value of which is rooted.
// ADRs 0034 and 0043 refuse a written binding because the premise is not
// flow-sensitive. This needs no flow sensitivity: if every value the binding
// can hold is the caller's, then whichever one it holds at the read is the
// caller's, and which branch assigned it never comes up.
// ---------------------------------------------------------------------------

// The loop shape compiled code walks a tree with. Its sources are the
// parameter and a read of the binding itself, which is the chain rule ADR 0034
// already applies to `a.b.c` written as a loop. **Certifies.**
export function writtenJoin(source) {
  let current = source;
  while (current.parent) {
    current = current.parent;
  }
  return current.value;
}

// A binding with **no** initializer: before the first assignment it holds
// `undefined`, and a member read of that throws before any lookup, so it
// contributes no source at all. **Certifies.**
export function writtenFromUninitialized(items) {
  let entry;
  for (let index = 0; index < 1; index++) {
    entry = items[index];
  }
  return entry.value;
}

// A join written as an expression rather than as assignments: each arm of a
// `??` is rooted at the same parameter, so the value is the caller's whichever
// arm supplies it. **Certifies.**
//
// Both arms must root at *one* slot. `first ?? second.fallback` is the
// caller's either way and still refuses, because the receipt names a slot and
// naming either would say the caller passed something it did not — the same
// boundary `writtenFromTwoSlots` pins for the assignment spelling.
export function joinedArms(source) {
  const chosen = source.primary ?? source.fallback;
  return chosen.value;
}

// One source is a value this module made. The join holds only when *every*
// source is the caller's. **Refuses.**
export function writtenFromModuleValue(source, flag) {
  let current = source;
  if (flag) {
    current = untypedRegistry;
  }
  return current.value;
}

// Two different parameters. The value is the caller's either way, but the
// receipt names one slot and naming either would say the caller passed
// something it did not. **Refuses.**
export function writtenFromTwoSlots(first, second, flag) {
  let current = first;
  if (flag) {
    current = second;
  }
  return current.value;
}

// A **destructuring** write takes a property of something else, which this
// walk has no single expression for — so the whole binding refuses rather than
// the source being skipped, because a skipped source would make the join a
// claim about only some of the values. **Refuses.**
export function writtenByDestructuring(source, other) {
  let current = source;
  ({ current } = other);
  return current.value;
}
