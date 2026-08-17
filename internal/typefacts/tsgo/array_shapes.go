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
