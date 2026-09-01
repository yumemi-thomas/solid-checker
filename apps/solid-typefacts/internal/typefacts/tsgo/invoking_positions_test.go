package tsgo

import (
	"context"
	"encoding/json"
	"fmt"
	"path/filepath"
	"strings"
	"testing"

	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts"
)

// implementationTranscriptsForFunctions asks for one export-value transcript per
// named top-level function declaration in `source` and returns the
// implementation censuses by name. Every name must appear exactly once as
// `function <name>` and once as `void <name>;`.
func implementationTranscriptsForFunctions(
	t *testing.T,
	source string,
	names []string,
) map[string]*typefacts.ExportImplementationTranscript {
	t.Helper()
	return implementationTranscriptsForProject(t, source, names, nil)
}

// implementationTranscriptsForProject is the same, with extra files alongside
// `facts.ts` — a declaration file is how an *imported* callee is spelled, and
// the deferred dialect premise exists only for one.
func implementationTranscriptsForProject(
	t *testing.T,
	source string,
	names []string,
	extra map[string]string,
) map[string]*typefacts.ExportImplementationTranscript {
	t.Helper()
	dir := t.TempDir()
	files := map[string]string{"facts.ts": source}
	for name, content := range extra {
		files[name] = content
	}
	writeInvocationProject(t, dir, files)
	return implementationTranscriptsInProjectDir(t, dir, source, names)
}

// implementationTranscriptsInProjectDir answers one demand list against a
// project directory that is already written. Opening a *fresh* project over the
// *same* directory is what lets one program be asked for in several demand
// orders and the answers compared byte for byte: a new temporary directory
// would move every path and every path-derived symbol id, and the comparison
// would be about the harness rather than about the analysis.
func implementationTranscriptsInProjectDir(
	t *testing.T,
	dir string,
	source string,
	names []string,
) map[string]*typefacts.ExportImplementationTranscript {
	t.Helper()
	opened, err := OpenProject(context.Background(), filepath.Join(dir, "tsconfig.json"), nil)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = opened.Close() })
	analyzer, ok := opened.(typefacts.ExportValueAnalyzer)
	if !ok {
		t.Fatal("TypeScript-Go project does not implement ExportValueAnalyzer")
	}
	path := filepath.Join(dir, "facts.ts")
	demands := make([]typefacts.ExportValueDemand, 0, len(names))
	for _, name := range names {
		queryStart := strings.LastIndex(source, "void "+name+";")
		implementationStart := strings.Index(source, "function "+name)
		if queryStart < 0 || implementationStart < 0 {
			t.Fatalf("source lacks `void %s;` or `function %s`", name, name)
		}
		queryStart += len("void ")
		implementationStart += len("function ")
		demands = append(demands, typefacts.ExportValueDemand{
			Location: typefacts.Location{
				Path: path, StartByte: queryStart, EndByte: queryStart + len(name),
			},
			ImplementationLocation: &typefacts.Location{
				Path: path, StartByte: implementationStart, EndByte: implementationStart + len(name),
			},
		})
	}
	answer, err := analyzer.ExportValueTranscripts(context.Background(), demands)
	if err != nil {
		t.Fatal(err)
	}
	if len(answer.Transcripts) != len(names) {
		t.Fatalf("transcripts = %d, want %d", len(answer.Transcripts), len(names))
	}
	censuses := make(map[string]*typefacts.ExportImplementationTranscript, len(names))
	for index, name := range names {
		implementation := answer.Transcripts[index].Implementation
		if implementation == nil {
			t.Fatalf("%s has no implementation census: %#v", name, answer.Transcripts[index])
		}
		censuses[name] = implementation
	}
	return censuses
}

// callAt is the census entry for the call whose source text starts at the first
// occurrence of `needle`.
func callAt(
	t *testing.T,
	implementation *typefacts.ExportImplementationTranscript,
	source, needle string,
) typefacts.ImplementationCall {
	t.Helper()
	start := strings.Index(source, needle)
	if start < 0 {
		t.Fatalf("source lacks %q", needle)
	}
	for _, call := range implementation.Calls {
		if call.Location.StartByte == start {
			return call
		}
	}
	t.Fatalf("no census call starts at %q (byte %d); calls = %#v", needle, start, implementation.Calls)
	return typefacts.ImplementationCall{}
}

func argumentCallables(call typefacts.ImplementationCall, argument int) []typefacts.Location {
	for _, carried := range call.ArgumentCallables {
		if carried.Argument == argument {
			return carried.Locations
		}
	}
	return nil
}

// TestArgumentCallablesCarryTheSameProofReturnSitesDo pins the argument-side
// twin of CarriedCallables: the descent is argument-agnostic, so an arrow handed
// to a call is reported at that slot exactly as a returned arrow is reported at
// a return site, and a nested call inside it is bound by containment.
func TestArgumentCallablesCarryTheSameProofReturnSitesDo(t *testing.T) {
	source := `declare function schedule(callback: () => void, ...rest: unknown[]): void;
declare function scheduleAll(callbacks: (() => void)[]): void;
declare const node: HTMLElement;
declare const spread: [() => void];
declare const store: { onDone?: () => void };
function inlineArrow(callback: () => void) {
  schedule(() => { callback(); });
}
function constIndirection(callback: () => void) {
  const handler = () => { callback(); };
  schedule(handler);
}
function spreadArgument(callback: () => void) {
  void callback;
  schedule(...spread);
}
function propertyStore(callback: () => void) {
  const handler = () => { callback(); };
  store.onDone = handler;
}
function objectLiteralListener(callback: () => void) {
  const listener = {
    handleEvent() { void 0; },
    spare: () => { callback(); },
  };
  node.addEventListener("click", listener);
}
function arrayLiteralArgument(callback: () => void) {
  scheduleAll([() => { callback(); }]);
}
void inlineArrow;
void constIndirection;
void spreadArgument;
void propertyStore;
void objectLiteralListener;
void arrayLiteralArgument;
`
	names := []string{
		"inlineArrow", "constIndirection", "spreadArgument", "propertyStore",
		"objectLiteralListener", "arrayLiteralArgument",
	}
	censuses := implementationTranscriptsForFunctions(t, source, names)

	// The inline arrow is carried at slot 0, and the `callback()` inside it lies
	// within that range. Containment is what makes the fact bind.
	inline := censuses["inlineArrow"]
	outer := callAt(t, inline, source, "schedule(() => { callback(); })")
	carried := argumentCallables(outer, 0)
	if len(carried) != 1 {
		t.Fatalf("inline argument callables = %#v, want one carried arrow", outer.ArgumentCallables)
	}
	inner := callAt(t, inline, source, "callback();")
	if !inner.Captured || !locationWithinAny(inner.Location, carried) {
		t.Fatalf("captured call %#v is not inside the carried arrow %#v", inner.Location, carried)
	}

	// A single-declaration `const` naming the arrow preserves identity, exactly
	// as it does for a returned value.
	indirection := censuses["constIndirection"]
	forwarded := callAt(t, indirection, source, "schedule(handler)")
	if len(argumentCallables(forwarded, 0)) != 1 {
		t.Fatalf("const indirection argument callables = %#v, want the arrow", forwarded.ArgumentCallables)
	}

	// A spread's contribution to the slots is not fixed, so no slot carries a
	// proven callable through it.
	spread := censuses["spreadArgument"]
	spreadCall := callAt(t, spread, source, "schedule(...spread)")
	if len(spreadCall.ArgumentCallables) != 0 {
		t.Fatalf("spread argument callables = %#v, want none", spreadCall.ArgumentCallables)
	}

	// The trap: a property store hands the callable somewhere no call vouches
	// for. There is no argument slot at all, so nothing carries it and the
	// nested call stays open.
	store := censuses["propertyStore"]
	for _, call := range store.Calls {
		if len(call.ArgumentCallables) != 0 {
			t.Fatalf("property store produced argument callables %#v", call.ArgumentCallables)
		}
	}

	// An `EventListenerObject` is the shape that decides why this descent is
	// narrower than a return site's. `addEventListener` calls exactly the
	// `handleEvent` member and never `spare`, so a slot that credited every
	// property of the literal would assert execution of code the runtime cannot
	// reach. The whole literal carries nothing, and the demand stays open —
	// the same answer a bare `{ handleEvent }` gets, because the descent refuses
	// the construction rather than trying to pick the right member out of it.
	listener := censuses["objectLiteralListener"]
	registration := callAt(t, listener, source, `node.addEventListener("click", listener)`)
	if len(registration.ArgumentCallables) != 0 {
		t.Fatalf(
			"an object literal at an invoking slot carried %#v, want nothing",
			registration.ArgumentCallables,
		)
	}

	// An array literal is refused for the same reason: it is several callables
	// in one value, and an invoking slot runs at most the one its runtime names.
	elements := censuses["arrayLiteralArgument"]
	batch := callAt(t, elements, source, "scheduleAll([")
	if len(batch.ArgumentCallables) != 0 {
		t.Fatalf(
			"an array literal at an argument slot carried %#v, want nothing",
			batch.ArgumentCallables,
		)
	}
}

// TestEnclosingCallableNamesTheImmediatelyContainingCallable pins the fact a
// composing execution premise needs, and the shape that makes byte containment
// alone unsound.
//
// `storesInner` is the falsifier: `callback()` sits inside the range
// `createEffect` invokes, and yet `callback` never runs, because the effect only
// stores the closure that would have called it. What separates it from the
// debounce shape below is not containment — both nest identically — but which
// callable *immediately* contains the call, and whether that one is handed to a
// position anything proves invoking.
func TestEnclosingCallableNamesTheImmediatelyContainingCallable(t *testing.T) {
	source := `declare function effect(run: () => void): void;
declare const registry: (() => void)[];
function storesInner(callback: () => void) {
  effect(() => {
    const inner = () => { callback(); };
    registry.push(inner);
  });
}
function schedulesInner(callback: () => void) {
  return () => {
    setTimeout(() => { callback(); }, 0);
  };
}
void storesInner;
void schedulesInner;
`
	censuses := implementationTranscriptsForFunctions(
		t, source, []string{"storesInner", "schedulesInner"},
	)

	stores := censuses["storesInner"]
	outer := callAt(t, stores, source, "effect(() => {")
	if outer.Captured || outer.EnclosingCallable != nil {
		t.Fatalf("the effect call is in the body itself, not a callable: %#v", outer)
	}
	effectArrow := argumentCallables(outer, 0)
	if len(effectArrow) != 1 {
		t.Fatalf("effect slot 0 carries %#v, want the arrow", outer.ArgumentCallables)
	}
	inner := callAt(t, stores, source, "callback();")
	if inner.EnclosingCallable == nil {
		t.Fatal("the captured call names no enclosing callable")
	}
	// The decisive assertion. The call is *contained* by the arrow `effect`
	// invokes, and is *enclosed* by the one `registry.push` merely stores. A
	// premise reading containment cannot tell those apart; one reading the
	// enclosing callable must find `inner` carried at a proven invoking slot,
	// and `push` is no such slot.
	if *inner.EnclosingCallable == effectArrow[0] {
		t.Fatalf(
			"enclosing callable %#v is the effect's arrow, not the stored inner closure",
			*inner.EnclosingCallable,
		)
	}
	if !locationWithinAny(inner.Location, effectArrow) {
		t.Fatalf("the falsifier requires the call to sit inside the effect arrow: %#v", inner)
	}
	push := callAt(t, stores, source, "registry.push(inner)")
	stored := argumentCallables(push, 0)
	if len(stored) != 1 || stored[0] != *inner.EnclosingCallable {
		t.Fatalf("registry.push slot 0 carries %#v, want the inner arrow", push.ArgumentCallables)
	}
	if push.EnclosingCallable == nil || *push.EnclosingCallable != effectArrow[0] {
		t.Fatalf("the push call is enclosed by %#v, want the effect arrow", push.EnclosingCallable)
	}

	// The debounce shape composes exactly, one proven link at a time: the
	// returned closure encloses the `setTimeout` call, and the arrow `setTimeout`
	// invokes encloses the callback call. Every link is a separate fact.
	schedules := censuses["schedulesInner"]
	timer := callAt(t, schedules, source, "setTimeout(")
	returned := schedules.ControlFlow.Returns
	if len(returned) != 1 || len(returned[0].CarriedCallables) != 1 {
		t.Fatalf("return sites = %#v, want one carrying one closure", returned)
	}
	if timer.EnclosingCallable == nil || *timer.EnclosingCallable != returned[0].CarriedCallables[0] {
		t.Fatalf("setTimeout is enclosed by %#v, want the returned closure", timer.EnclosingCallable)
	}
	scheduled := argumentCallables(timer, 0)
	if len(scheduled) != 1 {
		t.Fatalf("setTimeout slot 0 carries %#v, want the arrow", timer.ArgumentCallables)
	}
	deferred := callAt(t, schedules, source, "callback(); }, 0)")
	if deferred.EnclosingCallable == nil || *deferred.EnclosingCallable != scheduled[0] {
		t.Fatalf(
			"the deferred call is enclosed by %#v, want the arrow setTimeout invokes",
			deferred.EnclosingCallable,
		)
	}
}

// TestDefaultLibraryInvokerResolvesBySymbolIdentity pins the Tier-B table: the
// members on it are recognized by default-library symbol identity, and every
// near miss — a shadow, a user type, an `any` receiver, an unlisted member — is
// refused rather than trusted.
func TestDefaultLibraryInvokerResolvesBySymbolIdentity(t *testing.T) {
	source := `declare const node: HTMLElement;
declare const loose: any;
declare const numbers: number[];
declare const settled: Promise<number>;
declare const bag: { forEach(callback: (value: number) => void): void };
function libraryInvokers(callback: () => void) {
  setTimeout(callback, 0);
  setInterval(callback, 0);
  queueMicrotask(callback);
  requestAnimationFrame(callback);
  node.addEventListener("pointerdown", callback);
  numbers.forEach(() => callback());
  settled.then(() => callback(), () => callback());
}
function refusedInvokers(callback: () => void) {
  node.removeEventListener("pointerdown", callback);
  bag.forEach(() => callback());
  loose.addEventListener("pointerdown", callback);
  navigator.geolocation.watchPosition(() => callback());
}
function shadowedTimeout(callback: () => void) {
  function setTimeout(handler: () => void) { queue.push(handler); }
  setTimeout(callback);
}
declare const queue: (() => void)[];
void libraryInvokers;
void refusedInvokers;
void shadowedTimeout;
`
	censuses := implementationTranscriptsForFunctions(
		t, source, []string{"libraryInvokers", "refusedInvokers", "shadowedTimeout"},
	)
	listed := censuses["libraryInvokers"]
	for _, expected := range []struct {
		needle  string
		invoker typefacts.DefaultLibraryInvoker
		slots   []int
	}{
		{"setTimeout(callback, 0)", typefacts.DefaultLibraryInvokerSetTimeout, []int{0}},
		{"setInterval(callback, 0)", typefacts.DefaultLibraryInvokerSetInterval, []int{0}},
		{"queueMicrotask(callback)", typefacts.DefaultLibraryInvokerQueueMicrotask, []int{0}},
		{"requestAnimationFrame(callback)", typefacts.DefaultLibraryInvokerRequestAnimationFrame, []int{0}},
		{`node.addEventListener("pointerdown", callback)`, typefacts.DefaultLibraryInvokerAddEventListener, []int{1}},
		{"numbers.forEach(() => callback())", typefacts.DefaultLibraryInvokerArrayIteration, []int{0}},
		{"settled.then(", typefacts.DefaultLibraryInvokerPromiseThen, []int{0, 1}},
	} {
		call := callAt(t, listed, source, expected.needle)
		if call.DefaultLibraryInvoker != expected.invoker {
			t.Fatalf("%s invoker = %q, want %q", expected.needle, call.DefaultLibraryInvoker, expected.invoker)
		}
		if len(call.InvokedArguments) != len(expected.slots) {
			t.Fatalf("%s invoked arguments = %#v, want %#v", expected.needle, call.InvokedArguments, expected.slots)
		}
		for index, slot := range expected.slots {
			if call.InvokedArguments[index] != slot {
				t.Fatalf("%s invoked arguments = %#v, want %#v", expected.needle, call.InvokedArguments, expected.slots)
			}
		}
	}

	// Every one of these really is a "the runtime probably calls it" shape, and
	// every one of them must stay open: removing a handler is not evidence
	// anything runs; a user type's `forEach` is not the library's; an
	// `any`-typed receiver resolves to no symbol; and `watchPosition` was never
	// reviewed onto the table, so the browser's own behavior is not a premise
	// this producer may assert.
	refused := censuses["refusedInvokers"]
	for _, needle := range []string{
		`node.removeEventListener("pointerdown", callback)`,
		"bag.forEach(() => callback())",
		`loose.addEventListener("pointerdown", callback)`,
		"navigator.geolocation.watchPosition(() => callback())",
	} {
		call := callAt(t, refused, source, needle)
		if call.DefaultLibraryInvoker != "" || len(call.InvokedArguments) != 0 {
			t.Fatalf(
				"%s invoker = %q slots = %#v, want no fact at all",
				needle, call.DefaultLibraryInvoker, call.InvokedArguments,
			)
		}
	}

	// A locally declared `setTimeout` that stores its handler shadows the
	// global. The name matches and the symbol does not.
	shadowed := censuses["shadowedTimeout"]
	call := callAt(t, shadowed, source, "setTimeout(callback);")
	if call.DefaultLibraryInvoker != "" || len(call.InvokedArguments) != 0 {
		t.Fatalf("shadowed setTimeout invoker = %q, want none", call.DefaultLibraryInvoker)
	}
	// It is also not an invoking callee on its own merits: it pushes.
	if len(call.CalleeInvokedParameters) != 0 || len(call.CalleeStronglyInvokedParameters) != 0 {
		t.Fatalf("shadowed setTimeout callee facts = %#v", call)
	}
}

// TestCalleeParameterInvocationFactsSeparateStrengths pins the S4 producer
// facts and, above all, the boundary between them: a chain that terminates in a
// direct call proves the position is used as a function, and a chain that
// terminates at `addEventListener` proves only that the value runs.
func TestCalleeParameterInvocationFactsSeparateStrengths(t *testing.T) {
	source := `declare const node: HTMLElement;
declare const queue: unknown[];
function callsDirectly(handler: () => void) { handler(); }
function forwardsOnce(handler: () => void) { callsDirectly(handler); }
function forwardsTwice(handler: () => void) { forwardsOnce(handler); }
function registers(handler: () => void) { node.addEventListener("click", handler as never); }
function forwardsToRegistrar(handler: () => void) { registers(handler); }
var guardedAccess = (value: unknown) => (typeof value === "function" ? value() : value);
function tap(handler: () => void) { return handler; }
function stash(handler: () => void) { queue.push(handler); }
function maybe(handler: () => void, on: boolean) { if (on) queue.push(handler); }
function storeLater(handler: () => void) { queue.push(() => { handler(); }); }
function wrapsForward(handler: () => void) { queue.push(() => { callsDirectly(handler); }); }
function returnsClosure(handler: () => void) { return () => handler(); }
function afterReturn(handler: () => void) { return; handler(); }
function afterThrow(handler: () => void) { throw new Error("x"); handler(); }
function twoHop(callback: () => void) { forwardsTwice(callback); }
function guarded(callback: () => void) { void guardedAccess(callback); }
function registrarChain(callback: () => void) { forwardsToRegistrar(callback); }
function returnsIt(callback: () => void) { void tap(callback); }
function storesIt(callback: () => void) { stash(callback); }
function conditionallyQueues(callback: () => void) { maybe(callback, true); }
function storesInAClosure(callback: () => void) { storeLater(callback); }
function forwardsInAClosure(callback: () => void) { wrapsForward(callback); }
function receivesAClosure(callback: () => void) { void returnsClosure(callback); }
function callsAfterReturn(callback: () => void) { afterReturn(callback); }
function callsAfterThrow(callback: () => void) { afterThrow(callback); }
void twoHop;
void guarded;
void registrarChain;
void returnsIt;
void storesIt;
void conditionallyQueues;
void storesInAClosure;
void forwardsInAClosure;
void receivesAClosure;
void callsAfterReturn;
void callsAfterThrow;
`
	names := []string{
		"twoHop", "guarded", "registrarChain", "returnsIt", "storesIt", "conditionallyQueues",
		"storesInAClosure", "forwardsInAClosure", "receivesAClosure",
		"callsAfterReturn", "callsAfterThrow",
	}
	censuses := implementationTranscriptsForFunctions(t, source, names)

	// Two plain forwards then a direct call: strong, and therefore also invoked.
	twoHop := callAt(t, censuses["twoHop"], source, "forwardsTwice(callback)")
	if len(twoHop.CalleeStronglyInvokedParameters) != 1 || twoHop.CalleeStronglyInvokedParameters[0] != 0 {
		t.Fatalf("two-hop strong = %#v, want [0]", twoHop.CalleeStronglyInvokedParameters)
	}
	if len(twoHop.CalleeInvokedParameters) != 1 || twoHop.CalleeInvokedParameters[0] != 0 {
		t.Fatalf("two-hop invoked = %#v, want [0]", twoHop.CalleeInvokedParameters)
	}
	// Directly-called is about the callee's *own* body, which forwards rather
	// than calls, so it stays empty and the distinction is legible.
	if len(twoHop.CalleeDirectlyCalledParameters) != 0 {
		t.Fatalf("two-hop directly called = %#v, want none", twoHop.CalleeDirectlyCalledParameters)
	}

	// A bundler-emitted `var` arrow whose guarded body calls the parameter is a
	// direct call on some path. This is the shape @solid-primitives/utils ships
	// as `access`.
	guarded := callAt(t, censuses["guarded"], source, "guardedAccess(callback)")
	if len(guarded.CalleeDirectlyCalledParameters) != 1 ||
		len(guarded.CalleeStronglyInvokedParameters) != 1 {
		t.Fatalf("guarded access facts = %#v, want parameter 0 directly called", guarded)
	}

	// The boundary. `registers` hands its parameter to `addEventListener`, so
	// the value runs — invoked — but nothing in the chain calls it, so it is not
	// strongly invoked and may not discharge a callable claim.
	chain := callAt(t, censuses["registrarChain"], source, "forwardsToRegistrar(callback)")
	if len(chain.CalleeInvokedParameters) != 1 || chain.CalleeInvokedParameters[0] != 0 {
		t.Fatalf("registrar chain invoked = %#v, want [0]", chain.CalleeInvokedParameters)
	}
	if len(chain.CalleeStronglyInvokedParameters) != 0 {
		t.Fatalf(
			"registrar chain strong = %#v, want none: reaching addEventListener is not a direct call",
			chain.CalleeStronglyInvokedParameters,
		)
	}

	// Returning a parameter is not invoking it; storing it is not invoking it;
	// and storing it behind a condition is the trap a "flows somewhere" fact
	// would wrongly credit.
	//
	// The next three are the same refusals with an arrow wrapped around them,
	// and they are the ones that decide whether the property-storage refusal
	// means anything. `storeLater` *writes down* `handler()` and never runs it;
	// a walk that descended into the stored closure would credit it and make
	// wrapping the stored value in an arrow a way past every bullet above.
	// `wrapsForward` is the same defeat one hop further out, and
	// `returnsClosure` is the expression-bodied form where the callee's whole
	// body is the closure.
	//
	// The last two are written down and unreachable. Code after a `return` or a
	// `throw` is text, not execution, and a fact that credited it would assert
	// a run the runtime cannot perform.
	for name, needle := range map[string]string{
		"returnsIt":           "tap(callback)",
		"storesIt":            "stash(callback)",
		"conditionallyQueues": "maybe(callback, true)",
		"storesInAClosure":    "storeLater(callback)",
		"forwardsInAClosure":  "wrapsForward(callback)",
		"receivesAClosure":    "returnsClosure(callback)",
		"callsAfterReturn":    "afterReturn(callback)",
		"callsAfterThrow":     "afterThrow(callback)",
	} {
		call := callAt(t, censuses[name], source, needle)
		if len(call.CalleeInvokedParameters) != 0 ||
			len(call.CalleeStronglyInvokedParameters) != 0 ||
			len(call.CalleeDirectlyCalledParameters) != 0 {
			t.Fatalf("%s produced callee invocation facts %#v", name, call)
		}
	}
}

// TestCalleeParameterInvocationFactsRefuseUnresolvableCallees pins the four
// refusals of §2.4/§4.4 that are about resolution rather than about what a body
// does: a member callee, a computed callee, an overload set with no
// implementation body, and a callee whose implementation is outside the analysed
// program. None of the four may emit a fact.
func TestCalleeParameterInvocationFactsRefuseUnresolvableCallees(t *testing.T) {
	source := `import { external } from "fixture-external";
declare const handlers: Record<string, (callback: () => void) => void>;
declare const holder: { invoke(callback: () => void): void };
declare function overloaded(callback: () => void): void;
declare function overloaded(callback: () => void, flag: boolean): void;
function memberCallee(callback: () => void) { holder.invoke(callback); }
function computedCallee(callback: () => void) { handlers["click"](callback); }
function overloadCallee(callback: () => void) { overloaded(callback); }
function externalCallee(callback: () => void) { external(callback); }
void memberCallee;
void computedCallee;
void overloadCallee;
void externalCallee;
`
	dir := t.TempDir()
	writeInvocationProject(t, dir, map[string]string{
		"facts.ts": source,
		// A declaration file gives the callee a type and no body at all, which
		// is what an out-of-program dependency looks like to the census.
		"fixture-external.d.ts": `declare module "fixture-external" {
  export function external(callback: () => void): void;
}
`,
	})
	opened, err := OpenProject(context.Background(), filepath.Join(dir, "tsconfig.json"), nil)
	if err != nil {
		t.Fatal(err)
	}
	defer opened.Close()
	analyzer := opened.(typefacts.ExportValueAnalyzer)
	path := filepath.Join(dir, "facts.ts")
	names := []string{"memberCallee", "computedCallee", "overloadCallee", "externalCallee"}
	demands := make([]typefacts.ExportValueDemand, 0, len(names))
	for _, name := range names {
		queryStart := strings.LastIndex(source, "void "+name+";") + len("void ")
		implementationStart := strings.Index(source, "function "+name) + len("function ")
		demands = append(demands, typefacts.ExportValueDemand{
			Location: typefacts.Location{
				Path: path, StartByte: queryStart, EndByte: queryStart + len(name),
			},
			ImplementationLocation: &typefacts.Location{
				Path: path, StartByte: implementationStart, EndByte: implementationStart + len(name),
			},
		})
	}
	answer, err := analyzer.ExportValueTranscripts(context.Background(), demands)
	if err != nil {
		t.Fatal(err)
	}
	for index, name := range names {
		implementation := answer.Transcripts[index].Implementation
		if implementation == nil {
			t.Fatalf("%s has no implementation census", name)
		}
		for _, call := range implementation.Calls {
			if len(call.CalleeInvokedParameters) != 0 ||
				len(call.CalleeStronglyInvokedParameters) != 0 ||
				len(call.CalleeDirectlyCalledParameters) != 0 {
				t.Fatalf("%s produced callee invocation facts %#v", name, call)
			}
		}
	}
}

// TestCalleeInvocationRecursionTerminatesOnACycle pins the cycle guard. Mutual
// recursion is ordinary code, and a census that walks callee bodies must
// terminate on it rather than trust that the input was a tree.
func TestCalleeInvocationRecursionTerminatesOnACycle(t *testing.T) {
	source := `function ping(handler: () => void) { pong(handler); }
function pong(handler: () => void) { ping(handler); }
function cyclic(callback: () => void) { ping(callback); }
void cyclic;
`
	censuses := implementationTranscriptsForFunctions(t, source, []string{"cyclic"})
	call := callAt(t, censuses["cyclic"], source, "ping(callback)")
	// Neither side of the cycle ever calls the handler, so the honest answer is
	// no evidence — reached by terminating, not by hanging.
	if len(call.CalleeInvokedParameters) != 0 || len(call.CalleeStronglyInvokedParameters) != 0 {
		t.Fatalf("cyclic forwarding produced facts %#v", call)
	}
}

// TestConstructExpressionsJoinTheCensusUnderTheirOwnTable pins the one
// construct row and the four refusals around it.
//
// `new Promise(executor)` runs its executor synchronously, before the
// constructor returns, so a census that recorded call expressions only left the
// executor's whole body unreachable to any execution premise. The row is
// resolved by default-library symbol identity exactly like the call table's,
// and the kind travels with every census entry so that a consumer whose claim
// is specifically about a call can refuse a construction.
func TestConstructExpressionsJoinTheCensusUnderTheirOwnTable(t *testing.T) {
	source := `class Runner { constructor(run: () => void) { void run; } }
declare const parts: [(resolve: (value: number) => void) => void];
function promiseExecutor(callback: () => void) {
  return new Promise<number>((resolve) => {
    callback();
    resolve(1);
  });
}
function userConstructor(callback: () => void) {
  void new Runner(() => { callback(); });
}
function shadowedPromise(callback: () => void) {
  class Promise { constructor(run: (resolve: (value: number) => void) => void) { void run; } }
  void new Promise((resolve) => { callback(); resolve(1); });
}
function spreadConstructor(callback: () => void) {
  void callback;
  void new Promise<number>(...parts);
}
void promiseExecutor;
void userConstructor;
void shadowedPromise;
void spreadConstructor;
`
	names := []string{"promiseExecutor", "userConstructor", "shadowedPromise", "spreadConstructor"}
	censuses := implementationTranscriptsForFunctions(t, source, names)

	// The positive: the construction is in the census, under its own kind, with
	// the reviewed invoker and slot 0 carrying the executor arrow. The callback
	// call inside it names that arrow as its enclosing callable, which is the
	// link a composing premise needs and the one the previous census could not
	// supply at all.
	executor := censuses["promiseExecutor"]
	construction := callAt(t, executor, source, "new Promise<number>((resolve)")
	if construction.Kind != typefacts.CallKindConstruct {
		t.Fatalf("construction kind = %q, want construct", construction.Kind)
	}
	if construction.DefaultLibraryInvoker != typefacts.DefaultLibraryInvokerPromiseConstructor ||
		len(construction.InvokedArguments) != 1 || construction.InvokedArguments[0] != 0 {
		t.Fatalf(
			"new Promise invoker = %q slots = %#v, want promiseConstructor [0]",
			construction.DefaultLibraryInvoker, construction.InvokedArguments,
		)
	}
	carried := argumentCallables(construction, 0)
	if len(carried) != 1 {
		t.Fatalf("new Promise slot 0 carries %#v, want the executor arrow", construction.ArgumentCallables)
	}
	// A construction states neither of the two facts that are claims about a
	// resolved *function's* body. A constructor resolves differently, and that
	// resolution was not reviewed.
	if construction.CalleeParameter != nil ||
		len(construction.CalleeDirectlyCalledParameters) != 0 ||
		len(construction.CalleeInvokedParameters) != 0 ||
		len(construction.CalleeStronglyInvokedParameters) != 0 ||
		len(construction.CalleePendingInvocations) != 0 {
		t.Fatalf("construction states callee-body facts: %#v", construction)
	}
	inner := callAt(t, executor, source, "callback();")
	if inner.Kind != typefacts.CallKindCall {
		t.Fatalf("call kind = %q, want call", inner.Kind)
	}
	if inner.EnclosingCallable == nil || *inner.EnclosingCallable != carried[0] {
		t.Fatalf(
			"the executed call is enclosed by %#v, want the arrow `new Promise` runs",
			inner.EnclosingCallable,
		)
	}

	// The refusals. A user class is not the library symbol; a locally shadowed
	// `Promise` is not either, however exactly it is spelled; and a spread's
	// contribution to the slots is not fixed, so the reviewed slot carries no
	// proven callable even though the invoker row still applies.
	for _, refusal := range []struct{ name, needle string }{
		{"userConstructor", "new Runner("},
		{"shadowedPromise", "new Promise((resolve)"},
	} {
		call := callAt(t, censuses[refusal.name], source, refusal.needle)
		if call.Kind != typefacts.CallKindConstruct {
			t.Fatalf("%s kind = %q, want construct", refusal.needle, call.Kind)
		}
		if call.DefaultLibraryInvoker != "" || len(call.InvokedArguments) != 0 {
			t.Fatalf(
				"%s invoker = %q slots = %#v, want no fact at all",
				refusal.needle, call.DefaultLibraryInvoker, call.InvokedArguments,
			)
		}
	}
	spread := callAt(t, censuses["spreadConstructor"], source, "new Promise<number>(...parts)")
	if spread.DefaultLibraryInvoker != typefacts.DefaultLibraryInvokerPromiseConstructor {
		t.Fatalf("spread construction invoker = %q, want the reviewed row", spread.DefaultLibraryInvoker)
	}
	if len(spread.ArgumentCallables) != 0 {
		t.Fatalf(
			"a spread construction carried %#v, want nothing: no slot is fixed",
			spread.ArgumentCallables,
		)
	}
}

// TestCalleeBodyComposesThroughProvenInvokingPositions pins the second half of
// the composing premise: a parameter called from inside a callable the callee
// *hands to a proven invoking position* is credited, and one called from inside
// a callable the callee merely stores is not — the same distinction the
// verifier draws about an exported implementation, drawn one body further in.
//
// The premise a chain ends on is sometimes a dialect fact, which this producer
// may not decide: it knows no framework vocabulary, and inferring one from a
// module and a name is the shortcut the precision contract forbids. So it
// states the syntax exactly — module, exported name, slot, argument count — and
// the verifier that owns the dialect answers it.
func TestCalleeBodyComposesThroughProvenInvokingPositions(t *testing.T) {
	source := `import { effect } from "fixture-dialect";
declare const node: HTMLElement;
declare const registry: (() => void)[];
declare function localEffect(run: () => void): void;
function callsInsideEffect(delay: () => number) { effect(() => { void delay(); }); }
function forwardsToEffect(delay: () => number) { callsInsideEffect(delay); }
function registersInsideEffect(handler: () => void) {
  effect(() => { node.addEventListener("click", handler); });
}
function schedulesInside(handler: () => void) { setTimeout(() => { handler(); }, 0); }
function storesInsideEffect(handler: () => void) {
  effect(() => {
    const inner = () => { handler(); };
    registry.push(inner);
  });
}
function callsInsideLocal(handler: () => void) { localEffect(() => { handler(); }); }
function timerChain(delay: () => number) { forwardsToEffect(delay); }
function registrarChain(handler: () => void) { registersInsideEffect(handler); }
function scheduledChain(handler: () => void) { schedulesInside(handler); }
function storedChain(handler: () => void) { storesInsideEffect(handler); }
function localChain(handler: () => void) { callsInsideLocal(handler); }
void timerChain;
void registrarChain;
void scheduledChain;
void storedChain;
void localChain;
`
	names := []string{"timerChain", "registrarChain", "scheduledChain", "storedChain", "localChain"}
	censuses := implementationTranscriptsForProject(t, source, names, map[string]string{
		"fixture-dialect.d.ts": `declare module "fixture-dialect" {
  export function effect(run: () => void): void;
}
`,
	})

	// The recovery, in the shape @solid-primitives/timer ships:
	// `createTimer` calls `delay()` inside the closure it hands to
	// `createEffect`, and two plain forwards carry that fact out to the export.
	// It is a *strong* claim — the terminal is a direct call of the parameter
	// and every hop is a plain forward — and it is conditional on exactly one
	// premise this producer refuses to decide.
	chain := callAt(t, censuses["timerChain"], source, "forwardsToEffect(delay)")
	if len(chain.CalleeStronglyInvokedParameters) != 0 || len(chain.CalleeInvokedParameters) != 0 {
		t.Fatalf("the chain claims an unconditional fact: %#v", chain)
	}
	if len(chain.CalleePendingInvocations) != 1 {
		t.Fatalf("chain pending = %#v, want one conditional claim", chain.CalleePendingInvocations)
	}
	pending := chain.CalleePendingInvocations[0]
	if pending.Parameter != 0 || !pending.Strong {
		t.Fatalf("chain pending = %#v, want parameter 0, strong", pending)
	}
	if len(pending.Requires) != 1 || pending.Requires[0] != (typefacts.InvokingSlotPremise{
		Module: "fixture-dialect", Name: "effect", Slot: 0, ArgumentCount: 1,
	}) {
		t.Fatalf("chain requirements = %#v, want the effect slot spelled exactly", pending.Requires)
	}

	// The strength ladder survives composition. A chain that reaches
	// `addEventListener` inside the effect closure proves the value runs and
	// nothing more, so it stays weak however it got there.
	registrar := callAt(t, censuses["registrarChain"], source, "registersInsideEffect(handler)")
	if len(registrar.CalleePendingInvocations) != 1 ||
		registrar.CalleePendingInvocations[0].Strong {
		t.Fatalf(
			"registrar chain pending = %#v, want one weak claim: addEventListener is not a direct call",
			registrar.CalleePendingInvocations,
		)
	}

	// A composition whose every link is already proven needs no premise at all,
	// and lands in the unconditional list. `setTimeout` is a reviewed row, so
	// nothing is deferred — and the terminal is still a direct call, so the
	// claim is still strong.
	scheduled := callAt(t, censuses["scheduledChain"], source, "schedulesInside(handler)")
	if len(scheduled.CalleeStronglyInvokedParameters) != 1 ||
		scheduled.CalleeStronglyInvokedParameters[0] != 0 {
		t.Fatalf("scheduled chain strong = %#v, want [0]", scheduled.CalleeStronglyInvokedParameters)
	}
	if len(scheduled.CalleePendingInvocations) != 0 {
		t.Fatalf("scheduled chain deferred %#v, want nothing", scheduled.CalleePendingInvocations)
	}
	// It is still not a *direct* call: `handler()` is written inside a closure,
	// not in the callee's own frame, and that distinction is the whole point of
	// the field.
	if len(scheduled.CalleeDirectlyCalledParameters) != 0 {
		t.Fatalf(
			"scheduled chain directly called = %#v, want none",
			scheduled.CalleeDirectlyCalledParameters,
		)
	}

	// The falsifier, and the invariant this extension exists to preserve: the
	// call sits inside a closure that sits inside a proven invoking position,
	// and it is still credited to nothing, because the closure immediately
	// containing it is pushed onto an array. Composition asks each link
	// separately; `registry.push` proves nothing about what it is handed, so
	// the chain breaks there.
	//
	// The last case is the same refusal for a different reason: a callee that
	// names no module carries no deferred premise, so a locally declared
	// `localEffect` with no body proves nothing about the closure it is given.
	for name, needle := range map[string]string{
		"storedChain": "storesInsideEffect(handler)",
		"localChain":  "callsInsideLocal(handler)",
	} {
		call := callAt(t, censuses[name], source, needle)
		if len(call.CalleeInvokedParameters) != 0 ||
			len(call.CalleeStronglyInvokedParameters) != 0 ||
			len(call.CalleeDirectlyCalledParameters) != 0 ||
			len(call.CalleePendingInvocations) != 0 {
			t.Fatalf("%s produced callee invocation facts %#v", name, call)
		}
	}
}

// TestEveryFunctionLikeBodyIsANestingBoundary pins the predicate both walks
// share, in the shapes that defeated its earlier spelling.
//
// A getter, a setter, a constructor and a static block each own a body whose
// statements run when *that* member runs, not when the code around it runs.
// Listing only arrows, function expressions, function declarations and methods
// left all four transparent: `registry.push({ get value() { cb(); return 1; }
// })` stores an object and calls nothing, and yet the census reported `cb()` as
// an uncaptured call of the enclosing body — the strongest form of every claim
// built on it — while the callee-body walk credited slot 0 of the enclosing
// function as directly called. Both are pinned here, because the two walks fail
// independently and the arrow-shaped closure pins reach neither.
//
// Object-literal getters are not a curiosity: `createComponent(X, { get
// children() { … } })` is the standard compiled-JSX lowering for a lazy prop,
// so every certified artifact that ships compiled JSX is in this shape.
func TestEveryFunctionLikeBodyIsANestingBoundary(t *testing.T) {
	source := `declare const registry: unknown[];
function stashGetter(getterCb: () => void) {
  registry.push({ get value() { getterCb(); return 1; } });
}
function stashSetter(setterCb: () => void) {
  registry.push({ set value(next: number) { setterCb(); void next; } });
}
function stashConstructor(ctorCb: () => void) {
  class Holder { constructor() { ctorCb(); } }
  registry.push(Holder);
}
function stashStaticBlock(staticCb: () => void) {
  class Holder { static { staticCb(); } }
  registry.push(Holder);
}
function stashGetterChain(callback: () => void) { stashGetter(callback); }
function stashSetterChain(callback: () => void) { stashSetter(callback); }
function stashConstructorChain(callback: () => void) { stashConstructor(callback); }
function stashStaticBlockChain(callback: () => void) { stashStaticBlock(callback); }
void stashGetter;
void stashSetter;
void stashConstructor;
void stashStaticBlock;
void stashGetterChain;
void stashSetterChain;
void stashConstructorChain;
void stashStaticBlockChain;
`
	names := []string{
		"stashGetter", "stashSetter", "stashConstructor", "stashStaticBlock",
		"stashGetterChain", "stashSetterChain", "stashConstructorChain", "stashStaticBlockChain",
	}
	censuses := implementationTranscriptsForFunctions(t, source, names)

	// The census half: each inner call is captured, and names the member that
	// contains it. `captured` is what three consumers read directly and mean
	// "this body calls it"; a false there is an unconditional over-proof.
	for name, needle := range map[string]string{
		"stashGetter":      "getterCb();",
		"stashSetter":      "setterCb();",
		"stashConstructor": "ctorCb();",
		"stashStaticBlock": "staticCb();",
	} {
		call := callAt(t, censuses[name], source, needle)
		if !call.Captured || call.EnclosingCallable == nil {
			t.Errorf(
				"%s: %s is captured=%v enclosing=%#v, want captured inside the member that owns it",
				name, needle, call.Captured, call.EnclosingCallable,
			)
		}
	}

	// The callee-body half: none of the four bodies invokes its parameter, so a
	// caller of any of them is credited with nothing at all.
	for name, needle := range map[string]string{
		"stashGetterChain":      "stashGetter(callback)",
		"stashSetterChain":      "stashSetter(callback)",
		"stashConstructorChain": "stashConstructor(callback)",
		"stashStaticBlockChain": "stashStaticBlock(callback)",
	} {
		call := callAt(t, censuses[name], source, needle)
		if len(call.CalleeDirectlyCalledParameters) != 0 ||
			len(call.CalleeInvokedParameters) != 0 ||
			len(call.CalleeStronglyInvokedParameters) != 0 ||
			len(call.CalleePendingInvocations) != 0 {
			t.Errorf("%s produced callee invocation facts %#v", name, call)
		}
	}
}

// TestAConciseCallableBodyIsStillANestedCallable pins the census exemption's
// removal.
//
// `export const wrap = cb => () => cb();` returns a closure; calling `wrap`
// calls nothing. The census used to exempt the implementation's own body from
// the nesting test, which for a concise arrow body means exempting the very
// callable that *is* the return value — so the `cb()` site was stamped
// `captured: false`, and every consumer that reads that field to mean "this
// implementation calls it" believed it.
//
// Nothing is lost by stamping it honestly. The reachable return site carries
// exactly that arrow, so `enclosingCallable` composes and the execution premise
// still holds; only the claims that say *call* refuse it now.
func TestAConciseCallableBodyIsStillANestedCallable(t *testing.T) {
	// The exemption only ever fired for an implementation whose Body() is
	// itself a callable, which is the `var`-declared concise arrow a bundler
	// emits — so the export under test has to be one, and the harness that
	// finds `function <name>` cannot spell it.
	source := `var wrap = (cb: () => void) => () => cb();
var callsDirectly = (cb: () => void) => cb();
void wrap;
void callsDirectly;
`
	dir := t.TempDir()
	writeInvocationProject(t, dir, map[string]string{"facts.ts": source})
	opened, err := OpenProject(context.Background(), filepath.Join(dir, "tsconfig.json"), nil)
	if err != nil {
		t.Fatal(err)
	}
	defer opened.Close()
	analyzer := opened.(typefacts.ExportValueAnalyzer)
	path := filepath.Join(dir, "facts.ts")
	names := []string{"wrap", "callsDirectly"}
	demands := make([]typefacts.ExportValueDemand, 0, len(names))
	for _, name := range names {
		queryStart := strings.LastIndex(source, "void "+name+";") + len("void ")
		implementationStart := strings.Index(source, "var "+name+" =") + len("var ")
		demands = append(demands, typefacts.ExportValueDemand{
			Location: typefacts.Location{
				Path: path, StartByte: queryStart, EndByte: queryStart + len(name),
			},
			ImplementationLocation: &typefacts.Location{
				Path: path, StartByte: implementationStart, EndByte: implementationStart + len(name),
			},
		})
	}
	answer, err := analyzer.ExportValueTranscripts(context.Background(), demands)
	if err != nil {
		t.Fatal(err)
	}
	censuses := make(map[string]*typefacts.ExportImplementationTranscript, len(names))
	for index, name := range names {
		implementation := answer.Transcripts[index].Implementation
		if implementation == nil {
			t.Fatalf("%s has no implementation census: %#v", name, answer.Transcripts[index])
		}
		censuses[name] = implementation
	}

	// The honest stamp: `cb()` runs when the *returned* arrow runs, not when
	// `wrap` does, so the site is captured and names that arrow.
	wrapped := censuses["wrap"]
	inner := callAt(t, wrapped, source, "cb();")
	if !inner.Captured || inner.EnclosingCallable == nil {
		t.Fatalf(
			"a call in a concise callable body is captured=%v enclosing=%#v, want captured",
			inner.Captured, inner.EnclosingCallable,
		)
	}
	// And nothing is lost: the return site carries exactly the callable that
	// encloses it, so the execution premise still composes through the chain.
	if wrapped.ControlFlow == nil || len(wrapped.ControlFlow.Returns) != 1 {
		t.Fatalf("concise body control flow = %#v, want one return site", wrapped.ControlFlow)
	}
	carried := wrapped.ControlFlow.Returns[0].CarriedCallables
	if len(carried) != 1 || carried[0] != *inner.EnclosingCallable {
		t.Fatalf(
			"return site carries %#v, want exactly the enclosing callable %#v",
			carried, *inner.EnclosingCallable,
		)
	}

	// A concise body that is *not* a callable is untouched: `cb => cb()` really
	// does call its parameter, in its own frame.
	direct := callAt(t, censuses["callsDirectly"], source, "cb();\nvoid wrap")
	if direct.Captured || direct.EnclosingCallable != nil {
		t.Fatalf("a concise *call* body is captured=%v, want uncaptured", direct.Captured)
	}
}

// TestASpreadEndsTheExactArgumentSlots pins the positional discipline at every
// producer site that turns an argument expression into a slot index.
//
// A slot index is a fact about the runtime call only while the written position
// and the runtime position agree. A spread breaks that agreement for itself and
// for everything after it, always toward *lower* slots — which is the
// over-proof direction, because the reviewed tables list low slots as invoking.
// `target.addEventListener(...pair, cb)` writes `cb` second and passes it
// third, where `addEventListener` reads an options bag and invokes nothing.
//
// The answer is a floor and never a renumbering: a spread's length is a runtime
// property the producer cannot read. Slots before it keep their exact meaning,
// which the second half of this test pins so the refusal cannot quietly widen
// into "any spread anywhere kills the call".
func TestASpreadEndsTheExactArgumentSlots(t *testing.T) {
	source := `declare function schedule(first: () => void, ...rest: unknown[]): void;
declare const extras: [() => void];
declare const listenerArgs: ["click", (event: Event) => void];
declare const target: EventTarget;
function displacedSlot(callback: () => void) {
  schedule(...extras, () => { callback(); });
}
function earlySlotSurvives(callback: () => void) {
  schedule(() => { callback(); }, ...extras);
}
function shiftedForward(cb: (() => void) & AddEventListenerOptions) {
  target.addEventListener(...listenerArgs, cb);
}
function exactForward(cb: (event: Event) => void) {
  target.addEventListener("click", cb, ...extras);
}
function shiftedForwardChain(cb: (() => void) & AddEventListenerOptions) {
  shiftedForward(cb);
}
function exactForwardChain(cb: (event: Event) => void) {
  exactForward(cb);
}
void displacedSlot;
void earlySlotSurvives;
void shiftedForward;
void exactForward;
void shiftedForwardChain;
void exactForwardChain;
`
	names := []string{
		"displacedSlot", "earlySlotSurvives", "shiftedForward", "exactForward",
		"shiftedForwardChain", "exactForwardChain",
	}
	censuses := implementationTranscriptsForFunctions(t, source, names)

	// Site 1, argumentCallables: the arrow is written at slot 1 and arrives at
	// slot 1 + len(extras). No slot carries it.
	displaced := callAt(t, censuses["displacedSlot"], source, "schedule(...extras,")
	if len(displaced.ArgumentCallables) != 0 {
		t.Errorf(
			"a slot after a spread carried %#v, want nothing: its runtime position is not fixed",
			displaced.ArgumentCallables,
		)
	}
	// And the parameter identity of a displaced slot is withheld too, while the
	// list keeps one entry per written argument so its length still means what
	// every consumer already reads it to mean. `cb` is written second and
	// arrives third, where `addEventListener` reads an options bag; a
	// `parameterIndex` at slot 1 would let the dialect and default-library
	// tables answer about the listener position.
	registration := callAt(t, censuses["shiftedForward"], source, "target.addEventListener(...listenerArgs")
	if len(registration.ArgumentParameters) != 2 || registration.ArgumentParameters[1] != nil {
		t.Errorf(
			"displaced argument parameters = %#v, want two entries with the second withheld",
			registration.ArgumentParameters,
		)
	}
	// The mirror image: a spread *after* the parameter leaves the parameter's
	// own slot stated, and withholds only the displaced tail.
	exactRegistration := callAt(t, censuses["exactForward"], source, `target.addEventListener("click", cb`)
	if len(exactRegistration.ArgumentParameters) != 3 ||
		exactRegistration.ArgumentParameters[1] == nil ||
		exactRegistration.ArgumentParameters[2] != nil {
		t.Errorf(
			"exact argument parameters = %#v, want slot 1 stated and slot 2 withheld",
			exactRegistration.ArgumentParameters,
		)
	}

	// Do not over-poison: a spread *after* the slot leaves that slot exact, and
	// the arrow written first really is `schedule`'s first argument.
	early := callAt(t, censuses["earlySlotSurvives"], source, "schedule(() =>")
	if len(argumentCallables(early, 0)) != 1 {
		t.Fatalf(
			"a slot before a spread carried %#v, want the arrow: its position is unchanged",
			early.ArgumentCallables,
		)
	}

	// Site 2 and 3, the callee-body walk: `shiftedForward`'s `cb` reaches
	// `addEventListener`'s third argument, which is read and never invoked, so
	// the chain out to its caller must credit nothing.
	shifted := callAt(t, censuses["shiftedForwardChain"], source, "shiftedForward(cb)")
	if len(shifted.CalleeInvokedParameters) != 0 ||
		len(shifted.CalleeStronglyInvokedParameters) != 0 ||
		len(shifted.CalleeDirectlyCalledParameters) != 0 ||
		len(shifted.CalleePendingInvocations) != 0 {
		t.Fatalf("a spread-shifted forward produced callee invocation facts %#v", shifted)
	}
	// The same chain with the spread *after* the reviewed slot is still exact:
	// slot 1 is the listener however many options follow it.
	exact := callAt(t, censuses["exactForwardChain"], source, "exactForward(cb)")
	if len(exact.CalleeInvokedParameters) != 1 || exact.CalleeInvokedParameters[0] != 0 {
		t.Fatalf("exact forward invoked = %#v, want [0]", exact.CalleeInvokedParameters)
	}
}

// TestASpreadWithholdsTheDeferredPremise pins the fourth site. The premise
// transmits an argument *count* as well as a slot, and a dialect answer can turn
// on it, so a spread anywhere in the list withholds the premise entirely — no
// prefix makes the runtime count knowable.
func TestASpreadWithholdsTheDeferredPremise(t *testing.T) {
	source := `import { effect } from "fixture-dialect";
declare const trailing: [number];
declare const leading: [() => void];
function callsInsideEffect(delay: () => number) { effect(() => { void delay(); }); }
function callsInsideSpreadEffect(delay: () => number) {
  effect(() => { void delay(); }, ...trailing);
}
function callsInsideDisplacedEffect(delay: () => number) {
  effect(...leading, () => { void delay(); });
}
function exactChain(callback: () => number) { callsInsideEffect(callback); }
function spreadChain(callback: () => number) { callsInsideSpreadEffect(callback); }
function displacedChain(callback: () => number) { callsInsideDisplacedEffect(callback); }
void exactChain;
void spreadChain;
void displacedChain;
`
	censuses := implementationTranscriptsForProject(
		t, source, []string{"exactChain", "spreadChain", "displacedChain"}, map[string]string{
			"fixture-dialect.d.ts": `declare module "fixture-dialect" {
  export function effect(run: () => void, ...rest: unknown[]): void;
}
`,
		},
	)

	// The control: no spread, so the premise is stated with an exact count.
	exact := callAt(t, censuses["exactChain"], source, "callsInsideEffect(callback)")
	if len(exact.CalleePendingInvocations) != 1 {
		t.Fatalf("exact chain pending = %#v, want one claim", exact.CalleePendingInvocations)
	}
	if len(exact.CalleePendingInvocations[0].Requires) != 1 ||
		exact.CalleePendingInvocations[0].Requires[0] != (typefacts.InvokingSlotPremise{
			Module: "fixture-dialect", Name: "effect", Slot: 0, ArgumentCount: 1,
		}) {
		t.Fatalf(
			"exact chain requirements = %#v, want the effect slot and count spelled exactly",
			exact.CalleePendingInvocations[0].Requires,
		)
	}

	// The same body with a trailing spread states nothing: slot 0 is still
	// written first, but `argumentCount` is not derivable, and a premise with an
	// unprovable field is not a premise.
	spread := callAt(t, censuses["spreadChain"], source, "callsInsideSpreadEffect(callback)")
	if len(spread.CalleePendingInvocations) != 0 ||
		len(spread.CalleeInvokedParameters) != 0 ||
		len(spread.CalleeStronglyInvokedParameters) != 0 {
		t.Errorf("a spread call deferred %#v, want nothing", spread)
	}

	// The composition site inside the callee body reads the same floor: the
	// closure is *written* at slot 1 and arrives at slot 1 + len(leading), so
	// no premise may name a slot for it at all. The runtime happens to agree
	// here, for a one-element tuple — which is exactly why the producer may not
	// decide it: the length is not a fact it can read.
	displaced := callAt(t, censuses["displacedChain"], source, "callsInsideDisplacedEffect(callback)")
	if len(displaced.CalleePendingInvocations) != 0 ||
		len(displaced.CalleeInvokedParameters) != 0 ||
		len(displaced.CalleeStronglyInvokedParameters) != 0 {
		t.Errorf("a displaced closure deferred %#v, want nothing", displaced)
	}
}

// TestCalleeFactsDoNotDependOnDemandOrder pins determinism.
//
// The callee memo answers per callee symbol *and depth*, because the depth is
// an input to the answer: the walk refuses below maxCalleeInvocationDepth, so a
// callee asked four bodies in has no headroom left where the same callee asked
// at the top has all of it. Keying by the symbol alone let a warm entry lend
// its headroom to a later, deeper question — and then `entry8` carried a fact
// when `entry1 … entry7` had been demanded before it in the same session, and
// carried none when it was demanded alone or first. Same source, same binaries,
// different answer. Nothing in a receipt, a gate-cache key or a proof digest
// names the demand list, so a package could certify in one run and be refused
// in the next with nothing changed.
//
// Three orderings over one program; the emitted facts must be byte-identical.
func TestCalleeFactsDoNotDependOnDemandOrder(t *testing.T) {
	var builder strings.Builder
	builder.WriteString("import { effect } from \"fixture-dialect\";\n")
	builder.WriteString("function hop0(fn: () => void) { effect(() => { fn(); }); }\n")
	for hop := 1; hop <= 8; hop++ {
		fmt.Fprintf(&builder, "function hop%d(fn: () => void) { hop%d(fn); }\n", hop, hop-1)
	}
	for entry := 1; entry <= 8; entry++ {
		fmt.Fprintf(
			&builder,
			"function entry%d(callback: () => void) { hop%d(callback); }\n",
			entry, entry,
		)
	}
	for entry := 1; entry <= 8; entry++ {
		fmt.Fprintf(&builder, "void entry%d;\n", entry)
	}
	source := builder.String()
	dir := t.TempDir()
	writeInvocationProject(t, dir, map[string]string{
		"facts.ts": source,
		"fixture-dialect.d.ts": `declare module "fixture-dialect" {
  export function effect(run: () => void): void;
}
`,
	})

	ascending := make([]string, 0, 8)
	for entry := 1; entry <= 8; entry++ {
		ascending = append(ascending, fmt.Sprintf("entry%d", entry))
	}
	deepestFirst := append([]string{"entry8"}, ascending[:7]...)

	orderings := map[string][]string{
		"alone":         {"entry8"},
		"ascending":     ascending,
		"deepest-first": deepestFirst,
	}
	// Each ordering opens its own project over the same files, so the only
	// thing that varies is the order the demands are answered in.
	observed := make(map[string]map[string]string, len(orderings))
	for label, names := range orderings {
		censuses := implementationTranscriptsInProjectDir(t, dir, source, names)
		facts := make(map[string]string, len(censuses))
		for name, census := range censuses {
			encoded, err := json.Marshal(census.Calls)
			if err != nil {
				t.Fatal(err)
			}
			facts[name] = string(encoded)
		}
		observed[label] = facts
	}

	for _, label := range []string{"alone", "deepest-first"} {
		for name, encoded := range observed[label] {
			if reference := observed["ascending"][name]; reference != encoded {
				t.Fatalf(
					"%s: %s facts depend on the demand order\n ascending: %s\n %9s: %s",
					label, name, reference, label, encoded,
				)
			}
		}
	}
	// And the answer they all agree on is the one a chain longer than the bound
	// deserves: no evidence. A memo hit must never buy depth the walk refused.
	deepest := observed["ascending"]["entry8"]
	if strings.Contains(deepest, "calleePendingInvocations") ||
		strings.Contains(deepest, "calleeInvokedParameters") {
		t.Fatalf("entry8 = %s, want no callee-body evidence past the depth bound", deepest)
	}
	// The bound is a bound and not a wall: the shallow chains still answer.
	shallow := observed["ascending"]["entry1"]
	if !strings.Contains(shallow, "calleePendingInvocations") {
		t.Fatalf("entry1 = %s, want the deferred premise one hop in", shallow)
	}
}
