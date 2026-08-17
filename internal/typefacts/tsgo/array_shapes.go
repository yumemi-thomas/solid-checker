package tsgo

import (
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/yumemi-thomas/solid-ts-facts/internal/typefacts"
)

// arrayShapeAtLocked classifies the type of exactly the demanded expression as
// array/tuple-shaped or not.
//
// The caller selects the node with exactExpressionAt, so the subject is the
// whole demanded expression and never a child of it. That matters here for the
// same reason it does for callResultDomain: an array of functions and a function
// returning an array are told apart only by which node is asked.
func (p *project) arrayShapeAtLocked(node *ast.Node, evidence *semanticEvidence) typefacts.ArrayShape {
	if node == nil {
		return typefacts.ArrayShapeUnknown
	}
	value := p.checker.GetTypeAtLocation(node)
	if value == nil {
		return typefacts.ArrayShapeUnknown
	}
	// The classification can turn on a type declared in another file — an
	// imported tuple alias is the motivating case — so a retained session must
	// invalidate this fact when that file changes. The descriptor helper records
	// exactly those alias declaration locations as dependencies.
	evidence.descriptor(p.typeDescriptorFor(value))
	return arrayShapeOfType(p.checker, value)
}

// openArrayShapeFlags are the type flags that leave an array/tuple claim
// unprovable in either direction. any and unknown admit an array; never and the
// error type describe no real value; an instantiable type stands for whatever a
// caller substitutes, and proving the predicate over every substitution through
// its constraint is a separate proof this fact does not attempt.
const openArrayShapeFlags = checker.TypeFlagsAny |
	checker.TypeFlagsUnknown |
	checker.TypeFlagsNever |
	checker.TypeFlagsIncludesError |
	checker.TypeFlagsInstantiable

func arrayShapeOfType(typeChecker *checker.Checker, value *checker.Type) typefacts.ArrayShape {
	if value == nil || value.Flags()&openArrayShapeFlags != 0 {
		return typefacts.ArrayShapeUnknown
	}
	constituents := value.Distributed()
	if len(constituents) == 0 {
		return typefacts.ArrayShapeUnknown
	}
	array, notArray := false, false
	for _, constituent := range constituents {
		if constituent == nil || constituent.Flags()&openArrayShapeFlags != 0 {
			return typefacts.ArrayShapeUnknown
		}
		if checker.Checker_isArrayOrTupleType(typeChecker, constituent) {
			array = true
		} else {
			notArray = true
		}
	}
	switch {
	case array && notArray:
		return typefacts.ArrayShapeMixed
	case array:
		return typefacts.ArrayShapeArray
	default:
		return typefacts.ArrayShapeNotArray
	}
}

// tupleShapeAtLocked describes the tuple at exactly the demanded expression, or
// nil when that type is not a tuple.
//
// Only a tuple has fixed, individually-typed element slots, which is what a
// consumer needs to decide whether a value satisfies an interface with numbered
// members. The global Array/ReadonlyArray types carry a number index signature
// instead and are deliberately excluded, as is a union: distributing a slot
// count over constituents would invent a shape none of them has.
func (p *project) tupleShapeAtLocked(node *ast.Node, evidence *semanticEvidence) *typefacts.TupleShape {
	if node == nil {
		return nil
	}
	value := p.checker.GetTypeAtLocation(node)
	if value == nil || value.Flags()&openArrayShapeFlags != 0 || !checker.IsTupleType(value) {
		return nil
	}
	target := value.TargetTupleType()
	if target == nil {
		return nil
	}
	evidence.descriptor(p.typeDescriptorFor(value))

	shape := typefacts.TupleShape{
		FixedLength: target.FixedLength(),
		ElementZero: typefacts.CallabilityUnknown,
	}
	for _, flags := range target.ElementFlags() {
		if flags&checker.ElementFlagsVariable != 0 {
			shape.HasRest = true
			break
		}
	}
	if elements := checker.Checker_getTypeArguments(p.checker, value); shape.FixedLength > 0 && len(elements) > 0 {
		shape.ElementZero = callabilityOfType(p.checker, elements[0])
		shape.ElementZeroMinimumParameters = minimumParameterCount(p.checker, elements[0])
	}
	return &shape
}

// minimumParameterCount is the fewest arguments any call signature of value
// requires. Overloads take the minimum, matching assignability: the checker
// needs only one compatible signature. A type with no call signatures answers
// zero, which callers must read together with the callability verdict.
func minimumParameterCount(typeChecker *checker.Checker, value *checker.Type) int {
	if value == nil {
		return 0
	}
	minimum := 0
	for index, signature := range typeChecker.GetSignaturesOfType(value, checker.SignatureKindCall) {
		if count := signature.MinArgumentCount(); index == 0 || count < minimum {
			minimum = count
		}
	}
	return minimum
}
