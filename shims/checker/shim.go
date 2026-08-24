// Package checker re-exports the slice of typescript-go's internal checker
// package that this repository uses — nothing more. Declarations are
// copied from oxc-project/tsgolint's generated shims (MIT); the module
// path claims the typescript-go prefix so the internal imports resolve.
// Regenerate by hand when a compiler bump moves an identifier: the
// compiler reports alias breaks, and the go:linkname signatures below
// must be re-verified against the target revision by eye.
package checker

import ast "github.com/microsoft/typescript-go/internal/ast"
import checker "github.com/microsoft/typescript-go/internal/checker"
import jsnum "github.com/microsoft/typescript-go/internal/jsnum"
import "math"
import "sync"
import _ "unsafe"

const CheckModeNormal = checker.CheckModeNormal

type Checker = checker.Checker

//go:linkname Checker_getResolvedSignature github.com/microsoft/typescript-go/internal/checker.(*Checker).getResolvedSignature
func Checker_getResolvedSignature(recv *checker.Checker, node *ast.Node, candidatesOutArray *[]*checker.Signature, checkMode checker.CheckMode) *checker.Signature

//go:linkname Checker_getBaseTypes github.com/microsoft/typescript-go/internal/checker.(*Checker).getBaseTypes
func Checker_getBaseTypes(recv *checker.Checker, t *checker.Type) []*checker.Type

//go:linkname Checker_getReturnTypeOfSignature github.com/microsoft/typescript-go/internal/checker.(*Checker).getReturnTypeOfSignature
func Checker_getReturnTypeOfSignature(recv *checker.Checker, sig *checker.Signature) *checker.Type

//go:linkname Checker_getTypeAtPosition github.com/microsoft/typescript-go/internal/checker.(*Checker).getTypeAtPosition
func Checker_getTypeAtPosition(recv *checker.Checker, signature *checker.Signature, pos int) *checker.Type

//go:linkname Checker_getBaseConstraintOfType github.com/microsoft/typescript-go/internal/checker.(*Checker).getBaseConstraintOfType
func Checker_getBaseConstraintOfType(recv *checker.Checker, t *checker.Type) *checker.Type

//go:linkname Checker_getAwaitedType github.com/microsoft/typescript-go/internal/checker.(*Checker).getAwaitedType
func Checker_getAwaitedType(recv *checker.Checker, t *checker.Type) *checker.Type

//go:linkname Checker_isTypeIdenticalTo github.com/microsoft/typescript-go/internal/checker.(*Checker).isTypeIdenticalTo
func Checker_isTypeIdenticalTo(recv *checker.Checker, source *checker.Type, target *checker.Type) bool

//go:linkname Checker_isContextSensitive github.com/microsoft/typescript-go/internal/checker.(*Checker).isContextSensitive
func Checker_isContextSensitive(recv *checker.Checker, node *ast.Node) bool

// Checker_isArrayOrTupleType is the compiler's own array/tuple predicate: a
// reference whose target is the global Array or ReadonlyArray type, or a
// reference to a tuple target. It deliberately excludes merely array-*like*
// types — an interface extending Array, or anything else assignable to
// ReadonlyArray<any> — because those are purpose-built types whose author
// chose them over an array, and the rules reading this fact honour that
// choice. Type aliases are transparent to it, which is the point: an aliased
// tuple answers the same as the tuple it names.
//
//go:linkname Checker_isArrayOrTupleType github.com/microsoft/typescript-go/internal/checker.(*Checker).isArrayOrTupleType
func Checker_isArrayOrTupleType(recv *checker.Checker, t *checker.Type) bool

// IsTupleType is the compiler's tuple predicate: a reference whose target
// carries the tuple object flag. It is narrower than isArrayOrTupleType, which
// also admits the global Array/ReadonlyArray types, and it is what separates a
// type with fixed, individually-typed element slots from one with only a number
// index signature.
//
//go:linkname IsTupleType github.com/microsoft/typescript-go/internal/checker.isTupleType
func IsTupleType(t *checker.Type) bool

// Checker_getTypeArguments returns a type reference's arguments; for a tuple
// reference those are its element types, in order.
//
//go:linkname Checker_getTypeArguments github.com/microsoft/typescript-go/internal/checker.(*Checker).getTypeArguments
func Checker_getTypeArguments(recv *checker.Checker, t *checker.Type) []*checker.Type

type TupleType = checker.TupleType

const ElementFlagsVariable = checker.ElementFlagsVariable
const ElementFlagsNonRequired = checker.ElementFlagsNonRequired

// Checker_isFunctionObjectType is the compiler's own "is this value a function
// object" predicate — the one its `typeof x === "function"` narrowing uses. It
// is true when the type has signatures of *either* kind, and otherwise when the
// type has a `bind` member and is a subtype of the global `Function` type. That
// second disjunct is the whole reason this is linknamed: it is the only place
// the compiler admits the signature-less `Function` supertype family
// (`Function`, `CallableFunction`, `NewableFunction`) as functions, and
// GetSignaturesOfType alone cannot see them.
//
// It calls resolveStructuredTypeMembers, which panics for a type carrying none
// of Object/Union/Intersection, so every caller must guard — the compiler's own
// call site at the globalFunctionType relation guards on TypeFlagsObject, and so
// does this repository's. Callers should also know that the `bind` quick-out
// reads the resolved members map, which the compiler leaves empty for every
// intersection by construction, so this predicate answers false for an
// intersection containing Function even though the call is permitted; that is
// why callability is derived from Checker_isUntypedFunctionCall instead and this
// identifier only pins conformance in tests.
//
//go:linkname Checker_isFunctionObjectType github.com/microsoft/typescript-go/internal/checker.(*Checker).isFunctionObjectType
func Checker_isFunctionObjectType(recv *checker.Checker, t *checker.Type) bool

// Checker_isTypeSubtypeOf is the compiler's subtype relation. It exists here to
// pin the Function-supertype boundary empirically in tests: the fallback inside
// isFunctionObjectType is `bind` member plus this relation against the global
// Function type, and a test that asserts which types satisfy it needs to ask
// the relation directly rather than infer it.
//
//go:linkname Checker_isTypeSubtypeOf github.com/microsoft/typescript-go/internal/checker.(*Checker).isTypeSubtypeOf
func Checker_isTypeSubtypeOf(recv *checker.Checker, source *checker.Type, target *checker.Type) bool

// Checker_isUntypedFunctionCall is the compiler's TS 1.0 §4.12 rule for whether
// a call whose callee exposes no signature is nonetheless legal: the callee is
// `any`, or a type parameter whose apparent type is `any`, or it has no
// signature of either kind, is not a union, is not never, and is assignable to
// the global `Function` type. The compiler resolves such a call to anySignature.
// It is what decides callability for the signature-less Function-supertype
// family, because it is what decides the call. Pass the apparent type as the
// second argument, as every compiler call site does; the two signature counts
// are the caller's already-computed ones.
//
//go:linkname Checker_isUntypedFunctionCall github.com/microsoft/typescript-go/internal/checker.(*Checker).isUntypedFunctionCall
func Checker_isUntypedFunctionCall(recv *checker.Checker, funcType *checker.Type, apparentFuncType *checker.Type, numCallSignatures int, numConstructSignatures int) bool

// Checker_getReducedApparentType is the type GetSignaturesOfType already looks
// through: reduction, then the apparent type, then reduction again. It exists
// here so the untyped-call rule above is asked with the same apparent type the
// compiler's own call sites pass it, rather than with the declared type.
//
//go:linkname Checker_getReducedApparentType github.com/microsoft/typescript-go/internal/checker.(*Checker).getReducedApparentType
func Checker_getReducedApparentType(recv *checker.Checker, t *checker.Type) *checker.Type

//go:linkname NewChecker github.com/microsoft/typescript-go/internal/checker.NewChecker
func NewChecker(program checker.Program, tracer *checker.Tracer) (*checker.Checker, *sync.Mutex)

const SignatureKindCall = checker.SignatureKindCall
const SignatureKindConstruct = checker.SignatureKindConstruct
const SignatureFlagsIsSignatureCandidateForOverloadFailure = checker.SignatureFlagsIsSignatureCandidateForOverloadFailure
const TypeFlagsAny = checker.TypeFlagsAny
const TypeFlagsUnknown = checker.TypeFlagsUnknown
const TypeFlagsNever = checker.TypeFlagsNever
const TypeFlagsUndefined = checker.TypeFlagsUndefined
const TypeFlagsVoid = checker.TypeFlagsVoid
const TypeFlagsNull = checker.TypeFlagsNull
const TypeFlagsStringLike = checker.TypeFlagsStringLike
const TypeFlagsNumberLike = checker.TypeFlagsNumberLike
const TypeFlagsNumberLiteral = checker.TypeFlagsNumberLiteral
const TypeFlagsBooleanLike = checker.TypeFlagsBooleanLike
const TypeFlagsBigIntLike = checker.TypeFlagsBigIntLike
const TypeFlagsESSymbolLike = checker.TypeFlagsESSymbolLike
const TypeFlagsTypeParameter = checker.TypeFlagsTypeParameter
const TypeFlagsInstantiable = checker.TypeFlagsInstantiable
const TypeFlagsUnion = checker.TypeFlagsUnion
const TypeFlagsIntersection = checker.TypeFlagsIntersection
const TypeFlagsIncludesError = checker.TypeFlagsIncludesError
const TypeFlagsObject = checker.TypeFlagsObject

type Type = checker.Type
type Signature = checker.Signature

// NumberLiteralIsFinite reads the compiler's canonical numeric literal value,
// not rendered type text. It is false for every non-literal number type.
func NumberLiteralIsFinite(value *checker.Type) bool {
	if value == nil || value.Flags()&checker.TypeFlagsNumberLiteral == 0 {
		return false
	}
	number, ok := value.AsLiteralType().Value().(jsnum.Number)
	return ok && !math.IsInf(float64(number), 0) && !math.IsNaN(float64(number))
}
