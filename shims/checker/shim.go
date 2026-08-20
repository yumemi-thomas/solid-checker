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
