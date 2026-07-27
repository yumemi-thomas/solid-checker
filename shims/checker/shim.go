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

//go:linkname Checker_getBaseConstraintOfType github.com/microsoft/typescript-go/internal/checker.(*Checker).getBaseConstraintOfType
func Checker_getBaseConstraintOfType(recv *checker.Checker, t *checker.Type) *checker.Type

//go:linkname Checker_getAwaitedType github.com/microsoft/typescript-go/internal/checker.(*Checker).getAwaitedType
func Checker_getAwaitedType(recv *checker.Checker, t *checker.Type) *checker.Type

//go:linkname Checker_isTypeIdenticalTo github.com/microsoft/typescript-go/internal/checker.(*Checker).isTypeIdenticalTo
func Checker_isTypeIdenticalTo(recv *checker.Checker, source *checker.Type, target *checker.Type) bool

//go:linkname NewChecker github.com/microsoft/typescript-go/internal/checker.NewChecker
func NewChecker(program checker.Program, tracer *checker.Tracer) (*checker.Checker, *sync.Mutex)

const SignatureKindCall = checker.SignatureKindCall

type Type = checker.Type
