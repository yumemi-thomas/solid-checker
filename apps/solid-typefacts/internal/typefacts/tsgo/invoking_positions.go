package tsgo

import (
	"slices"
	"sort"

	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts"
)

// argumentCallableLocationsLocked reports, per argument slot of one call, the
// exact source ranges of the callables that slot provably carries.
//
// It is the argument-side twin of a return site's CarriedCallables, but it is a
// *narrower* descent, not the same one. A returned value hands the caller
// everything inside it, so a return site may credit every element of
// `[fn, clear]` and every property of `Object.assign(fn, { clear })`. The claim
// built on an argument slot is stronger — that a proven invoking position runs
// the callable it is handed — and a bundle is not one callable. Passing
// `{ handleEvent, spare }` to `addEventListener` runs `handleEvent` and never
// `spare`, so crediting both would assert execution of code the runtime never
// reaches. Only the identity-preserving single-callable forms survive here: the
// callable expression itself, the wrappers that erase at runtime, and a
// single-declaration binding naming exactly one callable.
//
// A spread ends the exact slots: see exactArgumentSlots. Every other
// construction the descent refuses contributes nothing either, and absence is
// never evidence.
func (p *project) argumentCallableLocationsLocked(
	call *ast.Node,
) []typefacts.ImplementationArgumentCallable {
	var carried []typefacts.ImplementationArgumentCallable
	arguments := call.Arguments()
	exact := exactArgumentSlots(call)
	for index := 0; index < exact; index++ {
		argument := arguments[index]
		if argument == nil {
			continue
		}
		locations := p.singleCallableLocationsLocked(argument)
		if len(locations) == 0 {
			continue
		}
		carried = append(carried, typefacts.ImplementationArgumentCallable{
			Argument: index, Locations: locations,
		})
	}
	return carried
}

// exactArgumentSlots reports how many *leading* argument expressions of a call
// sit at a fixed runtime position — every slot before the first spread, and the
// whole list when there is none.
//
// A slot index is only ever a fact about the runtime call when the syntactic
// position and the runtime position agree, and a spread breaks that agreement
// for itself and for everything after it: `target.addEventListener(...pair,
// cb)` writes `cb` second and passes it third, so a fact naming slot 1 would
// name `addEventListener`'s listener while the runtime value there is the
// `"click"` string and `cb` is the options bag. The shift is toward *lower*
// slots, which is the over-proof direction, and the producer cannot repair it:
// a spread's length is a runtime property of the spread expression, not a fact
// any of this can read.
//
// So the answer is a floor, never a renumbering. Slots before the spread keep
// their exact meaning and are stated normally; slots at or after it state
// nothing at all. A fact that needs the *total* count rather than one slot —
// the deferred premise's argumentCount — is refused whenever any spread is
// present, since no prefix makes that number knowable.
func exactArgumentSlots(call *ast.Node) int {
	arguments := call.Arguments()
	for index, argument := range arguments {
		if argument != nil && ast.IsSpreadElement(argument) {
			return index
		}
	}
	return len(arguments)
}

// defaultLibraryInvokerRule is one reviewed row of the invoker table: the exact
// standard-library member, the argument slots its runtime invokes zero or more
// times, and — for a member — the interfaces whose declaration of that name was
// reviewed. A nil container set means "any default-library declaration of this
// exact name", which is used only where the set is both unmanageable and
// uniform; see defaultLibraryMemberInvokers.
type defaultLibraryInvokerRule struct {
	invoker    typefacts.DefaultLibraryInvoker
	slots      []int
	containers map[string]struct{}
}

func containerSet(names ...string) map[string]struct{} {
	set := make(map[string]struct{}, len(names))
	for _, name := range names {
		set[name] = struct{}{}
	}
	return set
}

// defaultLibraryGlobalInvokers is the reviewed table for callees written as a
// bare identifier. Every member schedules its argument 0 and runs it later.
//
// `navigator.geolocation.watchPosition` is deliberately absent even though its
// runtime really does invoke its callback: it was not reviewed into this table,
// and "the browser probably calls it" is not a premise. Growing the table is an
// act of review, never an inference from a name.
var defaultLibraryGlobalInvokers = map[string]defaultLibraryInvokerRule{
	"setTimeout":            {invoker: typefacts.DefaultLibraryInvokerSetTimeout, slots: []int{0}},
	"setInterval":           {invoker: typefacts.DefaultLibraryInvokerSetInterval, slots: []int{0}},
	"queueMicrotask":        {invoker: typefacts.DefaultLibraryInvokerQueueMicrotask, slots: []int{0}},
	"requestAnimationFrame": {invoker: typefacts.DefaultLibraryInvokerRequestAnimationFrame, slots: []int{0}},
	"requestIdleCallback":   {invoker: typefacts.DefaultLibraryInvokerRequestIdleCallback, slots: []int{0}},
}

// defaultLibraryMemberInvokers is the reviewed table for callees written as a
// property access.
//
// `addEventListener` carries no container set. Its declaring interface is not
// fixed — `EventTarget` declares the method and every one of its several
// hundred DOM subtypes redeclares it with a narrower event map, so a container
// allowlist would be a maintenance fiction rather than a proof. What is fixed,
// and what was audited at the pinned typescript-go revision
// (v0.0.0-20260724234109-8d29e62f3585), is that *every* declaration of the name
// `addEventListener` in the bundled default library is the same EventTarget
// registration whose argument 1 is the listener. The all-declarations
// default-library quantifier below still refuses a user interface of that name
// and refuses a `declare global` augmentation of a DOM one.
//
// `removeEventListener` is absent on its merits: removing a handler is not
// evidence that anything runs.
//
// The array-iteration row keeps its container set narrow and literal — the
// reviewed claim is about `Array.prototype`, and `ReadonlyArray` is the same
// method reached through a `readonly T[]` receiver. The typed arrays declare
// their own identically-shaped iteration methods and are deliberately *not*
// listed: they were not reviewed, so they stay open.
var defaultLibraryMemberInvokers = map[string]defaultLibraryInvokerRule{
	"addEventListener": {invoker: typefacts.DefaultLibraryInvokerAddEventListener, slots: []int{1}},
	"then": {
		invoker: typefacts.DefaultLibraryInvokerPromiseThen, slots: []int{0, 1},
		containers: containerSet("Promise", "PromiseLike"),
	},
	"catch": {
		invoker: typefacts.DefaultLibraryInvokerPromiseCatch, slots: []int{0},
		containers: containerSet("Promise"),
	},
	"finally": {
		invoker: typefacts.DefaultLibraryInvokerPromiseFinally, slots: []int{0},
		containers: containerSet("Promise"),
	},
	"forEach":     arrayIterationRule(),
	"map":         arrayIterationRule(),
	"filter":      arrayIterationRule(),
	"find":        arrayIterationRule(),
	"findIndex":   arrayIterationRule(),
	"some":        arrayIterationRule(),
	"every":       arrayIterationRule(),
	"flatMap":     arrayIterationRule(),
	"sort":        arrayIterationRule(),
	"reduce":      arrayIterationRule(),
	"reduceRight": arrayIterationRule(),
}

// defaultLibraryConstructInvokers is the reviewed table for `new` expressions,
// kept separate from the call tables because the two questions are different:
// `Promise(fn)` is not a call the language allows, and `new setTimeout(fn)` is
// not a construction it allows either. A row here vouches only for the
// construct position.
//
// `new Promise(executor)` is the whole table. The ES specification requires the
// constructor to call its executor synchronously, with the resolve and reject
// functions, before returning — so the executor's body is code invoking the
// constructor reaches. The `Promise` value is resolved by default-library
// symbol identity like every other row: a user `class Promise`, a locally
// shadowed binding and a `declare global` augmentation of the interface all
// resolve to a symbol with a declaration outside the default library and are
// refused whole.
var defaultLibraryConstructInvokers = map[string]defaultLibraryInvokerRule{
	"Promise": {invoker: typefacts.DefaultLibraryInvokerPromiseConstructor, slots: []int{0}},
}

func arrayIterationRule() defaultLibraryInvokerRule {
	return defaultLibraryInvokerRule{
		invoker:    typefacts.DefaultLibraryInvokerArrayIteration,
		slots:      []int{0},
		containers: containerSet("Array", "ReadonlyArray"),
	}
}

// defaultLibraryInvokerLocked answers which reviewed standard-library member
// this call's callee is, and which of its argument slots that member invokes.
//
// Resolution is by default-library symbol identity, the way
// isDefaultLibraryObjectAssignLocked resolves `Object.assign`, and never by
// spelling. A locally declared `function setTimeout(fn) { queue.push(fn) }`
// shadows the global and resolves to its own symbol; an `arr.forEach` whose
// receiver is a user type resolves to that type's member; an `any`-typed
// receiver resolves to no symbol at all. Each of the three emits nothing.
//
// A `new` expression answers from its own table (defaultLibraryConstructInvokers)
// and only for a bare-identifier constructor. `new namespace.Thing(fn)` and
// every computed constructor name no reviewed row.
func (p *project) defaultLibraryInvokerLocked(
	call *ast.Node,
) (typefacts.DefaultLibraryInvoker, []int) {
	callee := call.Expression()
	if callee == nil {
		return "", nil
	}
	if ast.IsNewExpression(call) {
		if !ast.IsIdentifier(callee) {
			return "", nil
		}
		rule, listed := defaultLibraryConstructInvokers[callee.Text()]
		if !listed {
			return "", nil
		}
		if !p.isDefaultLibraryMemberLocked(
			p.checker.GetSymbolAtLocation(callee), callee.Text(), rule.containers,
		) {
			return "", nil
		}
		return rule.invoker, append([]int(nil), rule.slots...)
	}
	if ast.IsIdentifier(callee) {
		rule, listed := defaultLibraryGlobalInvokers[callee.Text()]
		if !listed {
			return "", nil
		}
		if !p.isDefaultLibraryMemberLocked(
			p.checker.GetSymbolAtLocation(callee), callee.Text(), rule.containers,
		) {
			return "", nil
		}
		return rule.invoker, append([]int(nil), rule.slots...)
	}
	if !ast.IsPropertyAccessExpression(callee) {
		// `handlers[key](cb)` and every other computed or unresolvable callee
		// names no single member, and a member reached by an element access was
		// not what the table was reviewed against.
		return "", nil
	}
	name := callee.Name()
	if name == nil || !ast.IsIdentifier(name) {
		return "", nil
	}
	rule, listed := defaultLibraryMemberInvokers[name.Text()]
	if !listed {
		return "", nil
	}
	if !p.isDefaultLibraryMemberLocked(
		p.checker.GetSymbolAtLocation(name), name.Text(), rule.containers,
	) {
		return "", nil
	}
	return rule.invoker, append([]int(nil), rule.slots...)
}

// isDefaultLibraryMemberLocked requires the symbol to carry exactly `name`, to
// own at least one declaration, and for *every* declaration to sit in a file
// the compiler itself considers a default library — so one user-file
// augmentation refuses the whole symbol. A non-nil `containers` set also
// requires every declaration's parent to be one of the named interfaces.
//
// It is deliberately a sibling of isDefaultLibrarySymbolLocked rather than a
// generalization of it: that function's single-container arm is a statement of
// what `Object.assign` was verified against, and widening it to a set would
// blur that record.
func (p *project) isDefaultLibraryMemberLocked(
	symbol *ast.Symbol,
	name string,
	containers map[string]struct{},
) bool {
	symbol = p.canonicalSymbol(symbol)
	if symbol == nil || symbol.Name != name || len(symbol.Declarations) == 0 {
		return false
	}
	for _, declaration := range symbol.Declarations {
		sourceFile := ast.GetSourceFileOfNode(declaration)
		if sourceFile == nil || !p.program.IsSourceFileDefaultLibrary(sourceFile.Path()) {
			return false
		}
		if containers == nil {
			continue
		}
		parent := declaration.Parent
		if parent == nil || parent.Name() == nil {
			return false
		}
		if _, listed := containers[parent.Name().Text()]; !listed {
			return false
		}
	}
	return true
}

// Bounds on the callee-parameter descent. Four bodies is two more than the
// deepest real forwarding chain observed (`createIntervalCounter` →
// `createPolled` → `createTimer`), and the node budget stops one pathological
// body from costing the whole census. Exceeding either bound makes the result
// inexact, which suppresses caching and, at the top, emits nothing.
const (
	maxCalleeInvocationDepth  = 4
	maxCalleeInvocationBudget = 2048
)

// Bounds on the composition performed *inside* one callee body. They are the
// verifier's own execution-premise bounds, deliberately: this is that premise
// applied one body further in, so a shape the verifier would refuse for cost at
// the top must be refused for cost here too.
const (
	maxCalleeCompositionDepth = 8
	maxCalleeCompositionNodes = 256
)

// Bounds on a conditional claim. A conjunction of four invoking-slot premises
// already describes four levels of nesting, and four alternatives are four
// independent routes to the same parameter; a claim needing more is refused
// rather than transmitted. Refusing costs a demand, transmitting an unreadable
// fact costs the meaning of the fact.
const (
	maxPendingRequirements = 4
	maxPendingAlternatives = 4
)

// invokingPremises is a conjunction of invoking-slot premises: every slot in it
// must invoke the callable it is handed, or the claim it guards does not hold.
// Kept sorted and deduplicated so that two conjunctions with the same content
// compare equal.
type invokingPremises []typefacts.InvokingSlotPremise

// executionProof is a disjunction of those conjunctions — any one alternative
// proves the claim. A nil proof proves nothing. A proof that holds the empty
// conjunction is unconditional and is normalized to exactly that one
// alternative, because no weaker route adds anything to it.
type executionProof []invokingPremises

func unconditionalProof() executionProof {
	return executionProof{nil}
}

func (proof executionProof) unconditional() bool {
	return len(proof) == 1 && len(proof[0]) == 0
}

func premiseLess(left, right typefacts.InvokingSlotPremise) bool {
	if left.Module != right.Module {
		return left.Module < right.Module
	}
	if left.Name != right.Name {
		return left.Name < right.Name
	}
	if left.Slot != right.Slot {
		return left.Slot < right.Slot
	}
	return left.ArgumentCount < right.ArgumentCount
}

func premisesLess(left, right invokingPremises) bool {
	if len(left) != len(right) {
		return len(left) < len(right)
	}
	for index := range left {
		if left[index] != right[index] {
			return premiseLess(left[index], right[index])
		}
	}
	return false
}

// unionPremises conjoins two requirement sets. Exceeding the requirement bound
// is a refusal, not a truncation: dropping a requirement would turn a claim
// that needs it into a claim that does not.
func unionPremises(left, right invokingPremises) (invokingPremises, bool) {
	union := make(invokingPremises, 0, len(left)+len(right))
	union = append(union, left...)
	for _, premise := range right {
		if !slices.Contains(union, premise) {
			union = append(union, premise)
		}
	}
	if len(union) > maxPendingRequirements {
		return nil, false
	}
	slices.SortFunc(union, func(left, right typefacts.InvokingSlotPremise) int {
		switch {
		case premiseLess(left, right):
			return -1
		case premiseLess(right, left):
			return 1
		default:
			return 0
		}
	})
	return union, true
}

// normalizeProof sorts the alternatives smallest-first, removes duplicates and
// any alternative another already implies, collapses an unconditional route,
// and keeps at most the bound. Dropping a surplus *alternative* is safe in the
// direction that matters: fewer routes can only refuse more.
func normalizeProof(alternatives executionProof) executionProof {
	slices.SortFunc(alternatives, func(left, right invokingPremises) int {
		switch {
		case premisesLess(left, right):
			return -1
		case premisesLess(right, left):
			return 1
		default:
			return 0
		}
	})
	var kept executionProof
	for _, alternative := range alternatives {
		if len(alternative) == 0 {
			return unconditionalProof()
		}
		implied := false
		for _, existing := range kept {
			if containsAllPremises(alternative, existing) {
				implied = true
				break
			}
		}
		if implied {
			continue
		}
		kept = append(kept, alternative)
		if len(kept) == maxPendingAlternatives {
			break
		}
	}
	return kept
}

func containsAllPremises(alternative, required invokingPremises) bool {
	for _, premise := range required {
		if !slices.Contains(alternative, premise) {
			return false
		}
	}
	return true
}

// mergeProofs is disjunction: either route proves the claim.
func mergeProofs(left, right executionProof) executionProof {
	if len(left) == 0 {
		return right
	}
	if len(right) == 0 {
		return left
	}
	if left.unconditional() || right.unconditional() {
		return unconditionalProof()
	}
	return normalizeProof(append(append(executionProof(nil), left...), right...))
}

// combineProofs is conjunction: both links of a chain must hold, so every
// alternative of the result carries the requirements of one alternative from
// each side. A side that proves nothing makes the whole chain prove nothing.
func combineProofs(left, right executionProof) executionProof {
	if len(left) == 0 || len(right) == 0 {
		return nil
	}
	if left.unconditional() {
		return right
	}
	if right.unconditional() {
		return left
	}
	var combined executionProof
	for _, first := range left {
		for _, second := range right {
			if union, ok := unionPremises(first, second); ok {
				combined = append(combined, union)
			}
		}
	}
	return normalizeProof(combined)
}

// calleeInvocationFacts is what one callee's own body proves about its
// parameters. The three claims are nested by construction:
// directlyCalled ⊆ stronglyInvoked ⊆ invoked.
//
// directlyCalled stays unconditional and stays in the body's own frame — it is
// the claim "this body calls that parameter", and a call the body only reaches
// through a closure it hands away is a different claim. The other two carry a
// proof rather than a bare membership, because a call site inside a nested
// callable is credited exactly when something proves that callable runs, and
// the last link of that chain is sometimes a premise only a dialect owner can
// answer.
type calleeInvocationFacts struct {
	directlyCalled  []int
	invoked         map[int]executionProof
	stronglyInvoked map[int]executionProof
}

func newCalleeInvocationFacts() *calleeInvocationFacts {
	return &calleeInvocationFacts{
		invoked:         make(map[int]executionProof),
		stronglyInvoked: make(map[int]executionProof),
	}
}

// creditInvoked records that the value at this parameter runs, on this proof.
func (facts *calleeInvocationFacts) creditInvoked(index int, proof executionProof) {
	if len(proof) == 0 {
		return
	}
	facts.invoked[index] = mergeProofs(facts.invoked[index], proof)
}

// creditStronglyInvoked records the stronger claim, which implies the weaker
// one on the same proof — the nesting is maintained here rather than assumed
// by a reader.
func (facts *calleeInvocationFacts) creditStronglyInvoked(index int, proof executionProof) {
	if len(proof) == 0 {
		return
	}
	facts.stronglyInvoked[index] = mergeProofs(facts.stronglyInvoked[index], proof)
	facts.creditInvoked(index, proof)
}

// wire splits the facts into the unconditional index lists and the conditional
// claims. A parameter whose weak claim is already unconditional transmits no
// pending alternative at all: nothing conditional adds to it.
func (facts *calleeInvocationFacts) wire() (
	directlyCalled, invoked, stronglyInvoked []int,
	pending []typefacts.CalleePendingInvocation,
) {
	directlyCalled = facts.directlyCalled
	invokedSet := make(map[int]struct{})
	strongSet := make(map[int]struct{})
	for index, proof := range facts.stronglyInvoked {
		if proof.unconditional() {
			strongSet[index] = struct{}{}
			continue
		}
		for _, alternative := range proof {
			pending = append(pending, typefacts.CalleePendingInvocation{
				Parameter: index, Strong: true, Requires: alternative,
			})
		}
	}
	for index, proof := range facts.invoked {
		if proof.unconditional() {
			invokedSet[index] = struct{}{}
			continue
		}
		for _, alternative := range proof {
			// The same alternative already transmitted as the stronger claim
			// carries the weaker one with it.
			if strongProof, credited := facts.stronglyInvoked[index]; credited &&
				slices.ContainsFunc(strongProof, func(strong invokingPremises) bool {
					return len(strong) == len(alternative) &&
						containsAllPremises(strong, alternative)
				}) {
				continue
			}
			pending = append(pending, typefacts.CalleePendingInvocation{
				Parameter: index, Requires: alternative,
			})
		}
	}
	slices.SortFunc(pending, func(left, right typefacts.CalleePendingInvocation) int {
		if left.Parameter != right.Parameter {
			return left.Parameter - right.Parameter
		}
		if left.Strong != right.Strong {
			if left.Strong {
				return -1
			}
			return 1
		}
		switch {
		case premisesLess(left.Requires, right.Requires):
			return -1
		case premisesLess(right.Requires, left.Requires):
			return 1
		default:
			return 0
		}
	})
	return directlyCalled, sortedIndices(invokedSet), sortedIndices(strongSet), pending
}

// calleeParameterInvocationFactsLocked is the census entry point: what does the
// callee of this call do with its own parameters?
func (p *project) calleeParameterInvocationFactsLocked(
	callee *ast.Node,
) (
	directlyCalled, invoked, stronglyInvoked []int,
	pending []typefacts.CalleePendingInvocation,
) {
	facts, _ := p.calleeInvocationFactsLocked(callee, 0, make(map[*ast.Symbol]struct{}))
	if facts == nil {
		return nil, nil, nil, nil
	}
	return facts.wire()
}

// calleeInvocationFactsLocked resolves one callee expression to a callable with
// a body in the analysed program and answers what that body does with its
// parameters. The second result reports whether the answer is *exact* — no
// depth cut, no cycle cut, no exhausted budget — because only an exact answer
// is a fixed point that may be cached or believed at another depth.
//
// Every refusal answers nil facts, which the consumer reads as no evidence:
//   - a callee that is not a bare identifier. `handlers[key](cb)` and
//     `obj.method(cb)` name no single canonical symbol, and guessed member
//     dispatch is not resolution.
//   - a callee whose implementation lives outside the analysed program — a
//     dependency, an ambient declaration, an overload set with no
//     implementation body. There is no body to read, so there is no fact.
func (p *project) calleeInvocationFactsLocked(
	callee *ast.Node,
	depth int,
	visiting map[*ast.Symbol]struct{},
) (*calleeInvocationFacts, bool) {
	if depth >= maxCalleeInvocationDepth {
		return nil, false
	}
	callee = identityPreservingUnwrap(callee)
	if callee == nil || !ast.IsIdentifier(callee) {
		return nil, true
	}
	target := p.canonicalSymbol(p.checker.GetSymbolAtLocation(callee))
	if target == nil {
		return nil, true
	}
	if _, cycling := visiting[target]; cycling {
		return nil, false
	}
	key := calleeInvocationKey{target: target, depth: depth}
	if cached, memoized := p.calleeInvocations[key]; memoized {
		return cached, true
	}
	implementation := p.calleeImplementationLocked(target)
	if implementation == nil {
		p.memoizeCalleeInvocationsLocked(key, nil)
		return nil, true
	}
	visiting[target] = struct{}{}
	facts, exact := p.calleeInvocationFactsForBodyLocked(implementation, depth, visiting)
	delete(visiting, target)
	if exact {
		p.memoizeCalleeInvocationsLocked(key, facts)
	}
	return facts, exact
}

// calleeInvocationKey names one memoized answer. The depth is part of the
// question, not part of the caller's history: see the field's own comment in
// project.go for why keying by the symbol alone made the fact set a function of
// the demand order.
type calleeInvocationKey struct {
	target *ast.Symbol
	depth  int
}

func (p *project) memoizeCalleeInvocationsLocked(
	key calleeInvocationKey,
	facts *calleeInvocationFacts,
) {
	if p.calleeInvocations == nil {
		p.calleeInvocations = make(map[calleeInvocationKey]*calleeInvocationFacts)
	}
	p.calleeInvocations[key] = facts
}

// calleeImplementationLocked names the callable body a resolved callee symbol
// denotes.
//
// The ordinary path is the signature implementation a function declaration or
// method owns. The second path exists because bundlers emit
// `var access = (v) => …` for a module-level arrow, which is a variable
// declaration and not a callable one. Descending into it demands exactly one
// declaration and that nothing in the declaring file ever writes to the
// binding — the same assignment test the returned-callable descent applies, and
// for the same reason: a reassignable binding does not prove the call reaches
// the body this initializer spells.
func (p *project) calleeImplementationLocked(target *ast.Symbol) *ast.Node {
	if implementation := invocationImplementationDeclaration(nil, target); implementation != nil &&
		implementation.Body() != nil {
		return implementation
	}
	if len(target.Declarations) != 1 {
		return nil
	}
	declaration := target.Declarations[0]
	if !ast.IsVariableDeclaration(declaration) || p.symbolIsAssignedLocked(target, declaration) {
		return nil
	}
	initializer := identityPreservingUnwrap(declaration.Initializer())
	if !isCallableDeclaration(initializer) || initializer.Body() == nil {
		return nil
	}
	return initializer
}

// calleeInvocationFactsForBodyLocked reads one callable's body and answers what
// it does with each of its own parameters.
//
// Three claims of decreasing strength are produced together:
//
//   - directlyCalled — the parameter appears as the callee of a call in this
//     body's own frame. This is the only one that by itself says the position
//     is used as a function, and the only one that is never conditional.
//   - stronglyInvoked — directlyCalled, or the parameter is forwarded as a
//     bare identifier at slot j of a further local callee whose own slot j is
//     stronglyInvoked. Every hop is a plain forward and the chain terminates in
//     a direct call, so the whole chain says the position is used as a function.
//   - invoked — stronglyInvoked, or the parameter reaches a proven invoking
//     position, or it is forwarded to a local callee whose slot is merely
//     invoked. This says the value runs; it does not say this callee calls it.
//
// A parameter that is returned, stored on a property, pushed onto an array, or
// handed to an unresolvable callee appears in none of the three. Nothing here
// walks "flows somewhere": every credit is a call position or a forward into
// one.
//
// Two restrictions on *where* in the body a credit may come from carry the
// weight of the whole fact:
//
//   - A call site inside a nested callable is credited only by composition.
//     A call this body writes down is not a call this body makes unless this
//     body's own execution reaches it: `function storeLater(fn) {
//     registry.push(() => { fn(); }); }` writes `fn()` and never runs it.
//     Crediting the site because its bytes sit inside the body would make the
//     closure-wrapped forward indistinguishable from a direct call, and would
//     defeat the property-storage refusal above by the simple act of wrapping
//     the stored value in an arrow. So the site counts exactly when the
//     callable immediately containing it is handed to a slot something proves
//     invoking, and that slot's own call composes in turn — the same discipline
//     the verifier applies to the exported implementation, applied one body
//     further in. `registry.push` proves nothing about what it is handed, so
//     the stored closure still breaks the chain and the demand stays open.
//
//     There is deliberately no return-site route here. A callee that *returns*
//     a closure hands it to its own caller, which in this position is the body
//     being analysed — and that body only called the callee, so nothing runs
//     the closure. `function returnsClosure(fn) { return () => fn(); }`
//     therefore proves nothing about `fn`.
//
//   - An unreachable call site contributes nothing. Statements after a `return`
//     or a `throw` are written down and never executed, and a fact that
//     credited them would assert a run the runtime cannot perform. ReachUnknown
//     is still credited: a guarded call is a call on some path, which is the
//     same strength the in-body direct-call premise already claims.
//
// Only identifier parameters are credited. A destructured member is a different
// value from the parameter that contains it, and attributing a member's call to
// the whole object is the attribution error the alias domain already refuses.
func (p *project) calleeInvocationFactsForBodyLocked(
	implementation *ast.Node,
	depth int,
	visiting map[*ast.Symbol]struct{},
) (*calleeInvocationFacts, bool) {
	walk := &calleeBodyWalk{
		p:         p,
		bySymbol:  make(map[*ast.Symbol]int),
		depth:     depth,
		visiting:  visiting,
		composing: make(map[*ast.Node]struct{}),
		budget:    maxCalleeCompositionNodes,
		exact:     true,
	}
	for _, root := range p.parameterCensusRootsLocked(implementation) {
		if len(root.path) != 0 {
			continue
		}
		if symbol := p.canonicalSymbol(root.symbol); symbol != nil {
			walk.bySymbol[symbol] = root.index
		}
	}
	walk.collectCallsLocked(implementation)
	return walk.creditLocked(), walk.exact
}

// calleeArgumentCallables binds one argument slot of one body call to the
// callables that slot provably carries, by node identity rather than by range:
// two callables never share a node, and a range comparison would have to
// re-derive what the descent already knows.
type calleeArgumentCallables struct {
	slot  int
	nodes []*ast.Node
}

// calleeBodyCall is one call or construct expression written inside a callee's
// body, with the two facts composition needs about it — which callable
// immediately contains it, and whether this body's own control flow reaches it
// — and the lazily computed answers about what it carries and what it invokes.
type calleeBodyCall struct {
	node      *ast.Node
	construct bool
	enclosing *ast.Node
	reach     typefacts.Reachability

	carried     []calleeArgumentCallables
	carriedRead bool

	invokerSlots []int
	invokerRead  bool

	callee     *calleeInvocationFacts
	calleeRead bool
}

// calleeBodyWalk is one pass over one callee body. It holds the two independent
// recursion guards this analysis needs: `visiting` stops an interprocedural
// cycle between callee symbols, and `composing` stops a cycle within one body,
// which a self-referential `const f = () => { g(f); }` makes reachable.
type calleeBodyWalk struct {
	p         *project
	bySymbol  map[*ast.Symbol]int
	calls     []*calleeBodyCall
	depth     int
	visiting  map[*ast.Symbol]struct{}
	composing map[*ast.Node]struct{}
	budget    int
	exact     bool
}

// collectCallsLocked records every call and construct expression in the body,
// at every nesting depth, with the innermost callable containing it.
//
// The reachability rules mirror implementationCallCensusLocked's, because the
// two must agree about what "this body executes that call" means: sequential
// statements after a `return` or `throw` are unreachable, a loop or switch body
// is unknown rather than reachable, and the branches of an `if` merge.
//
// It differs from that census in one deliberate place: *every* callable other
// than the body's own frame is a nesting boundary here, including a concise
// body that is itself a callable. `var wrap = fn => () => fn()` calls nothing —
// it returns a closure — and a walk that treated the returned arrow as the
// frame would credit `fn()` as a direct call.
func (w *calleeBodyWalk) collectCallsLocked(implementation *ast.Node) {
	budget := maxCalleeInvocationBudget
	var visit func(*ast.Node, *ast.Node, typefacts.Reachability) typefacts.Reachability
	visit = func(node, enclosing *ast.Node, reach typefacts.Reachability) typefacts.Reachability {
		if node == nil {
			return reach
		}
		if budget <= 0 {
			w.exact = false
			return reach
		}
		budget--
		nested := enclosing
		if isCallableDeclaration(node) {
			nested = node
		}
		if ast.IsBlock(node) {
			current := reach
			for _, statement := range node.AsBlock().Statements.Nodes {
				current = visit(statement, nested, current)
			}
			return current
		}
		if ast.IsIfStatement(node) {
			statement := node.AsIfStatement()
			visit(statement.Expression, nested, reach)
			thenReach := visit(statement.ThenStatement, nested, reach)
			elseReach := reach
			if statement.ElseStatement != nil {
				elseReach = visit(statement.ElseStatement, nested, reach)
			}
			return mergeReachability(thenReach, elseReach)
		}
		if ast.IsTryStatement(node) {
			statement := node.AsTryStatement()
			tryReach := visit(statement.TryBlock, nested, reach)
			catchReach := typefacts.Unreachable
			if statement.CatchClause != nil {
				catchReach = visit(statement.CatchClause.AsCatchClause().Block, nested, reach)
			}
			merged := mergeReachability(tryReach, catchReach)
			if statement.FinallyBlock != nil {
				visit(statement.FinallyBlock, nested, reach)
			}
			return merged
		}
		if ast.IsIterationStatement(node, true) || ast.IsSwitchStatement(node) {
			node.ForEachChild(func(child *ast.Node) bool {
				visit(child, nested, typefacts.ReachUnknown)
				return false
			})
			return typefacts.ReachUnknown
		}
		construct := ast.IsNewExpression(node)
		if ast.IsCallExpression(node) || construct {
			w.calls = append(w.calls, &calleeBodyCall{
				node: node, construct: construct, enclosing: nested, reach: reach,
			})
		}
		terminates := ast.IsReturnStatement(node) || ast.IsThrowStatement(node)
		node.ForEachChild(func(child *ast.Node) bool {
			visit(child, nested, reach)
			return false
		})
		if terminates {
			return typefacts.Unreachable
		}
		return reach
	}
	visit(implementation.Body(), nil, typefacts.Reachable)
}

// creditLocked turns the collected sites into the three claims.
func (w *calleeBodyWalk) creditLocked() *calleeInvocationFacts {
	facts := newCalleeInvocationFacts()
	directlyCalled := make(map[int]struct{})
	for _, call := range w.calls {
		proof := w.executionProofLocked(call, 0)
		if len(proof) == 0 {
			continue
		}
		// `new parameter()` is deliberately not a direct call here. Whether a
		// construction of a callback position is the same claim as a call of it
		// was not reviewed, and the census states no callee parameter for a
		// construct site either.
		if !call.construct {
			if index, rooted := w.p.parameterIndexOfLocked(call.node.Expression(), w.bySymbol); rooted {
				if call.enclosing == nil && proof.unconditional() {
					directlyCalled[index] = struct{}{}
				}
				facts.creditStronglyInvoked(index, proof)
			}
		}
		w.creditForwardedParametersLocked(call, proof, facts)
	}
	facts.directlyCalled = sortedIndices(directlyCalled)
	return facts
}

// executionProofLocked answers whether this body's own execution can reach one
// of its call sites, and on which unanswered premises that depends.
//
// A site in the body's own frame runs when the body runs. A site inside a
// nested callable runs only if something runs that callable: it must be handed
// to a slot proven invoking, on a call that itself composes. Nothing here reads
// byte containment, and a callable that is merely defined, stored, pushed or
// assigned is carried by nothing and breaks the chain.
func (w *calleeBodyWalk) executionProofLocked(call *calleeBodyCall, depth int) executionProof {
	if call.reach == typefacts.Unreachable {
		return nil
	}
	if call.enclosing == nil {
		return unconditionalProof()
	}
	if depth >= maxCalleeCompositionDepth || w.budget <= 0 {
		w.exact = false
		return nil
	}
	if _, cycling := w.composing[call.node]; cycling {
		return nil
	}
	w.composing[call.node] = struct{}{}
	defer delete(w.composing, call.node)
	var proof executionProof
	for _, outer := range w.calls {
		if outer == call {
			continue
		}
		if w.budget <= 0 {
			w.exact = false
			break
		}
		w.budget--
		for _, carried := range w.carriedLocked(outer) {
			if !containsNode(carried.nodes, call.enclosing) {
				continue
			}
			slotProof := w.slotInvokingProofLocked(outer, carried.slot)
			if len(slotProof) == 0 {
				continue
			}
			outerProof := w.executionProofLocked(outer, depth+1)
			if len(outerProof) == 0 {
				continue
			}
			proof = mergeProofs(proof, combineProofs(slotProof, outerProof))
			if proof.unconditional() {
				return proof
			}
		}
	}
	return proof
}

// creditForwardedParametersLocked credits the parameters this call forwards
// into a position that is already proven invoking, on the proof that this call
// site itself runs.
//
// A reviewed default-library invoker credits the *weak* claim only. Handing a
// listener to `addEventListener` proves the listener runs; it says nothing
// about whether the enclosing callee treats the position as a function, so a
// chain that ends there must not satisfy a claim that the position is callable.
// The same is true of a deferred dialect premise: composing through an invoking
// position changes where a call may sit, never what the chain proves, so only a
// local callee's own strong fact carries the strong claim onward.
func (w *calleeBodyWalk) creditForwardedParametersLocked(
	call *calleeBodyCall,
	siteProof executionProof,
	facts *calleeInvocationFacts,
) {
	arguments := call.node.Arguments()
	if len(arguments) == 0 {
		return
	}
	forwarded := make([]calleeForwardedParameter, 0, len(arguments))
	// Only the slots a spread has not displaced; see exactArgumentSlots.
	for slot := 0; slot < exactArgumentSlots(call.node); slot++ {
		argument := arguments[slot]
		if argument == nil {
			continue
		}
		if index, rooted := w.p.parameterIndexOfLocked(argument, w.bySymbol); rooted {
			forwarded = append(forwarded, calleeForwardedParameter{slot: slot, index: index})
		}
	}
	if len(forwarded) == 0 {
		return
	}
	for _, sent := range forwarded {
		if slotProof := w.slotInvokingProofLocked(call, sent.slot); len(slotProof) != 0 {
			facts.creditInvoked(sent.index, combineProofs(siteProof, slotProof))
		}
		callee := w.calleeFactsLocked(call)
		if callee == nil {
			continue
		}
		if strong, credited := callee.stronglyInvoked[sent.slot]; credited {
			facts.creditStronglyInvoked(sent.index, combineProofs(siteProof, strong))
		}
	}
}

type calleeForwardedParameter struct {
	slot  int
	index int
}

// slotInvokingProofLocked answers whether one argument slot of one body call
// invokes the callable it is handed, in the same three tiers the verifier's own
// premise uses.
//
//   - a reviewed default-library member, resolved by symbol identity;
//   - the callee's own body, when the callee resolves to a callable in this
//     program that sends that parameter to an invoking position;
//   - a dialect primitive — which this producer may not decide. It knows no
//     framework vocabulary, and inferring one from a module and a name is
//     exactly the shortcut the precision contract forbids, so it states the
//     syntax exactly and defers: which module the callee was imported from,
//     which name it was exported under, which slot, how many arguments. A
//     verifier that owns that dialect answers it; one that does not leaves the
//     claim unproven.
//
// A callee that names no module carries no deferred premise at all, so a local
// helper, a member call, a computed callee and a bare global all stay refused
// here unless one of the first two tiers already answered.
func (w *calleeBodyWalk) slotInvokingProofLocked(call *calleeBodyCall, slot int) executionProof {
	// A slot a spread has displaced is not this call's slot at runtime, so no
	// tier may answer for it; see exactArgumentSlots.
	arguments := call.node.Arguments()
	exact := exactArgumentSlots(call.node)
	if slot >= exact {
		return nil
	}
	if containsIndex(w.invokerSlotsLocked(call), slot) {
		return unconditionalProof()
	}
	if facts := w.calleeFactsLocked(call); facts != nil {
		if proof, credited := facts.invoked[slot]; credited {
			return proof
		}
	}
	if call.construct {
		// No construct-position dialect vocabulary was reviewed, and the census
		// states no callee-body facts for a construction either.
		return nil
	}
	if exact != len(arguments) {
		// The deferred premise transmits an argument count as well as a slot,
		// and a dialect answer can turn on it (`mergeProps` reads every source
		// below the count; `createResource` gives argument 0 a different role
		// at one argument than at two). A spread anywhere in the list makes the
		// runtime count unknowable, so the premise is not stated at all.
		return nil
	}
	target, name, module := w.p.callTargetIdentityLocked(call.node.Expression())
	if target == nil || name == "" || module == "" {
		return nil
	}
	return executionProof{invokingPremises{{
		Module: module, Name: name, Slot: slot, ArgumentCount: len(arguments),
	}}}
}

func (w *calleeBodyWalk) invokerSlotsLocked(call *calleeBodyCall) []int {
	if !call.invokerRead {
		call.invokerRead = true
		_, call.invokerSlots = w.p.defaultLibraryInvokerLocked(call.node)
	}
	return call.invokerSlots
}

func (w *calleeBodyWalk) calleeFactsLocked(call *calleeBodyCall) *calleeInvocationFacts {
	if call.calleeRead {
		return call.callee
	}
	call.calleeRead = true
	if call.construct {
		return nil
	}
	facts, resolved := w.p.calleeInvocationFactsLocked(
		call.node.Expression(), w.depth+1, w.visiting,
	)
	if !resolved {
		w.exact = false
	}
	call.callee = facts
	return call.callee
}

func (w *calleeBodyWalk) carriedLocked(call *calleeBodyCall) []calleeArgumentCallables {
	if call.carriedRead {
		return call.carried
	}
	call.carriedRead = true
	arguments := call.node.Arguments()
	// A spread's contribution to the slots is not fixed, so nothing at or after
	// it carries a proven callable at a knowable position; see
	// exactArgumentSlots.
	for slot := 0; slot < exactArgumentSlots(call.node); slot++ {
		argument := arguments[slot]
		if argument == nil {
			continue
		}
		nodes := w.p.returnedCallablesLocked(argument, carriedCallableDescentSingleCallable)
		if len(nodes) == 0 {
			continue
		}
		call.carried = append(call.carried, calleeArgumentCallables{slot: slot, nodes: nodes})
	}
	return call.carried
}

func containsNode(nodes []*ast.Node, node *ast.Node) bool {
	for _, candidate := range nodes {
		if candidate == node {
			return true
		}
	}
	return false
}

// parameterIndexOfLocked reports the parameter an expression is, exactly. Only
// a bare identifier — after the wrappers that erase at runtime — counts: a
// property access, a call result, or a wrapper expression is a different value,
// and forwarding one of those is not forwarding the parameter.
func (p *project) parameterIndexOfLocked(
	expression *ast.Node,
	bySymbol map[*ast.Symbol]int,
) (int, bool) {
	node := identityPreservingUnwrap(expression)
	if node == nil || !ast.IsIdentifier(node) {
		return 0, false
	}
	index, rooted := bySymbol[p.canonicalSymbol(p.checker.GetSymbolAtLocation(node))]
	return index, rooted
}

func containsIndex(indices []int, index int) bool {
	for _, candidate := range indices {
		if candidate == index {
			return true
		}
	}
	return false
}

func sortedIndices(set map[int]struct{}) []int {
	if len(set) == 0 {
		return nil
	}
	indices := make([]int, 0, len(set))
	for index := range set {
		indices = append(indices, index)
	}
	sort.Ints(indices)
	return indices
}
