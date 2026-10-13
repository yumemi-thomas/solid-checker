package tsgo

import (
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
)

// engineOwnedIndexedContainers is the reviewed list of default-library
// interfaces whose **numeric** index reaches an own data property of an object
// the engine itself created, so that reading `value[0]` runs nothing a user
// wrote. It is the indexed counterpart of engineOwnedIterableContainers and
// carries that table's discipline unchanged: a row is an act of review, never
// an inference from a name.
//
// The premise has two halves and needs both. An index signature in the default
// library *declares* no accessor — TypeScript cannot express one there — but a
// declaration is only evidence about the bytes that run when the object is the
// engine's own. A user object typed through a structural interface satisfies
// the declaration while carrying whatever getter it likes, so the declaration
// half alone proves nothing.
//
// That is why the **structural** interfaces are deliberately absent, and their
// absence is the whole precision of this table: `ArrayLike` and `ConcatArray`
// both declare `readonly [n: number]: T` in lib.es5.d.ts, and both are
// contracts an ordinary object satisfies — so the index named by the
// declaration is not the index that runs. This is exactly the `Iterable`
// versus `Array` split engineOwnedIterableContainers draws, for the same
// reason, and `Promise` versus `PromiseLike` before it.
//
// Every DOM and web-worker indexed collection is absent too — `NodeList`,
// `HTMLCollection`, `DOMTokenList`, `FileList`, `DataTransferItemList` and the
// rest. Their indices are engine code in fact, but they were not reviewed
// here, and "the browser probably owns it" is not a premise.
//
// `ReadonlyArray` is in, on the same footing the iterable table gave it: it is
// the ordinary spelling of a real array (`readonly T[]`), and a value assigned
// to it structurally carries the limit below rather than a new one.
//
// Two limits remain, and they are the two every declaration-based premise in
// this package carries. A value whose static type is `Array<T>` while the
// runtime object is a subclass overriding the index answers from the base
// declaration, and a Proxy is outside every producer census.
var engineOwnedIndexedContainers = containerSet(
	// lib.es5.d.ts.
	"Array", "ReadonlyArray", "String", "IArguments",
	"RegExpExecArray", "RegExpMatchArray", "TemplateStringsArray",
	// lib.es5.d.ts typed arrays.
	"Int8Array", "Uint8Array", "Uint8ClampedArray",
	"Int16Array", "Uint16Array", "Int32Array", "Uint32Array",
	"Float32Array", "Float64Array",
	// lib.es2020.bigint.d.ts and lib.es2025.float16.d.ts. A project whose lib
	// omits either declares no such global, and the name never resolves.
	"BigInt64Array", "BigUint64Array", "Float16Array",
)

// provablyEngineOwnedIndexedLocked reports whether **every** constituent of the
// subject's type is an engine-owned indexed container.
//
// The quantifier is per constituent for the reason awaitFormLocked's is: a
// union that is an array in one constituent is something else in another, and
// the read happens against whichever the value turns out to be.
//
// A `null` or `undefined` constituent therefore refuses, which is stricter than
// the argument requires — reading an index of either throws a TypeError and
// runs no user code, so an unnarrowed `RegExpExecArray | null` is safe in fact.
// It stays strict because "the form throws" is a claim about the whole form
// rather than about the member this function answers for, and it belongs to
// whatever decision wants to make it.
func (p *project) provablyEngineOwnedIndexedLocked(subject *ast.Node) bool {
	if subject == nil {
		return false
	}
	value := p.formChecker().GetTypeAtLocation(subject)
	if value == nil || value.Flags()&openArrayShapeFlags != 0 {
		return false
	}
	constituents := value.Distributed()
	if len(constituents) == 0 {
		return false
	}
	for _, constituent := range constituents {
		if constituent == nil || constituent.Flags()&openArrayShapeFlags != 0 {
			return false
		}
		// The apparent type, because a primitive is not its own interface: a
		// `string` subject carries TypeFlagsString and no symbol at all, while
		// the index it reaches is `String`'s. Instantiable types were already
		// refused above, so this never stands in for a type parameter's
		// constraint.
		apparent := checker.Checker_getReducedApparentType(p.formChecker(), constituent)
		if apparent == nil || !p.engineOwnedIndexedContainerLocked(apparent) {
			return false
		}
	}
	return true
}

// engineOwnedIndexedContainerLocked answers for one non-union type.
//
// It asks that *every* declaration of the type's own symbol be the default
// library's, rather than that there be one. That is the same all-declarations
// quantifier isDefaultLibraryMemberLocked applies, and it is what refuses a
// user interface that happens to be named `Array` and a `declare global`
// augmentation of the real one: a global the program also augments is not
// purely the engine's.
func (p *project) engineOwnedIndexedContainerLocked(value *checker.Type) bool {
	symbol := p.canonicalSymbol(value.Symbol())
	if symbol == nil || len(symbol.Declarations) == 0 {
		return false
	}
	if _, listed := engineOwnedIndexedContainers[symbol.Name]; !listed {
		return false
	}
	for _, declaration := range symbol.Declarations {
		sourceFile := ast.GetSourceFileOfNode(declaration)
		if sourceFile == nil || !p.program.IsSourceFileDefaultLibrary(sourceFile.Path()) {
			return false
		}
	}
	return true
}
