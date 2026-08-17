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
// nil when that type does not resolve to one.
//
// Only a tuple has fixed, individually-typed element slots, which is what a
// consumer needs to decide whether a value satisfies an interface with numbered
// members. The global Array/ReadonlyArray types carry a number index signature
// instead and are deliberately excluded.
//
// A union answers when every constituent that can hold a value is a tuple. The
// result is their conservative meet -- the slots they all have, and the strictest
// demand any of them makes -- so a consumer reading it learns only what holds
// whichever constituent the value turns out to be. Nullish constituents carry no
// structure and are skipped; a consumer that also needs the value to be present
// should read runtimeValueDomain.
func (p *project) tupleShapeAtLocked(node *ast.Node, evidence *semanticEvidence) *typefacts.TupleShape {
	if node == nil {
		return nil
	}
	value := p.checker.GetTypeAtLocation(node)
	if value == nil || value.Flags()&openArrayShapeFlags != 0 {
		return nil
	}
	var merged *typefacts.TupleShape
	for _, constituent := range value.Distributed() {
		if constituent == nil || constituent.Flags()&openArrayShapeFlags != 0 {
			return nil
		}
		if constituent.Flags()&(checker.TypeFlagsUndefined|checker.TypeFlagsNull|checker.TypeFlagsVoid) != 0 {
			continue
		}
		shape := tupleShapeOfType(p.checker, constituent)
		if shape == nil {
			return nil
		}
		if merged == nil {
			merged = shape
			continue
		}
		*merged = meetTupleShapes(*merged, *shape)
	}
	if merged == nil {
		return nil
	}
	evidence.descriptor(p.typeDescriptorFor(value))
	return merged
}

// tupleShapeOfType describes one non-union type, or nil when it is not a tuple.
func tupleShapeOfType(typeChecker *checker.Checker, value *checker.Type) *typefacts.TupleShape {
	if !checker.IsTupleType(value) {
		return nil
	}
	target := value.TargetTupleType()
	if target == nil {
		return nil
	}
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
	if elements := checker.Checker_getTypeArguments(typeChecker, value); shape.FixedLength > 0 && len(elements) > 0 {
		shape.ElementZero = callabilityOfType(typeChecker, elements[0])
		shape.ElementZeroMinimumParameters = minimumParameterCount(typeChecker, elements[0])
	}
	return &shape
}

// meetTupleShapes keeps only what both shapes guarantee: a slot exists if both
// have it, a rest tail only if both carry one, the first slot is callable only
// if it is in both, and the argument requirement is the larger of the two, since
// a caller has to satisfy whichever constituent it gets.
func meetTupleShapes(left, right typefacts.TupleShape) typefacts.TupleShape {
	met := typefacts.TupleShape{
		FixedLength: min(left.FixedLength, right.FixedLength),
		HasRest:     left.HasRest && right.HasRest,
		ElementZero: left.ElementZero,
		ElementZeroMinimumParameters: max(
			left.ElementZeroMinimumParameters,
			right.ElementZeroMinimumParameters,
		),
	}
	switch {
	case left.ElementZero == right.ElementZero:
	case left.ElementZero == typefacts.CallabilityUnknown || right.ElementZero == typefacts.CallabilityUnknown:
		met.ElementZero = typefacts.CallabilityUnknown
	default:
		met.ElementZero = typefacts.CallabilityMixed
	}
	return met
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
