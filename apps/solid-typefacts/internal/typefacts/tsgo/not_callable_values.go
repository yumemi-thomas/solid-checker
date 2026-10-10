package tsgo

import (
	"github.com/microsoft/typescript-go/shim/checker"

	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts"
)

// notCallableValueFact answers ADR 0099's question for one export value type:
// does the type prove the runtime value has neither [[Call]] nor
// [[Construct]]? It reuses the parameter facts' callability classifier
// (callabilityOfType), which already refuses any, unknown, never and error
// types and already answers UntypedCallable for the `Function` interface, and
// adds what a call domain needs beyond it: no construct signature on any
// constituent (a class value is invoked by `new`), and no instantiable
// constituent -- a type parameter or a deferred indexed/conditional type has
// no signatures *yet*, which is not the same fact as having none.
//
// The answer is nil whenever any of that fails; a consumer reads nil as "not
// stated", never as "callable".
func notCallableValueFact(typeChecker *checker.Checker, value *checker.Type) *typefacts.NotCallableValue {
	if value == nil || callabilityOfType(typeChecker, value) != typefacts.CallabilityNonCallable {
		return nil
	}
	constituents := value.Distributed()
	if len(constituents) == 0 {
		return nil
	}
	// The shim exposes no TypeFlagsPrimitive; this is upstream's mask minus
	// EnumLike, which the shim does not export and which a string or number
	// enum member already carries as StringLike or NumberLike.
	const primitiveFlags = checker.TypeFlagsStringLike | checker.TypeFlagsNumberLike |
		checker.TypeFlagsBigIntLike | checker.TypeFlagsBooleanLike | checker.TypeFlagsESSymbolLike |
		checker.TypeFlagsVoid | checker.TypeFlagsUndefined | checker.TypeFlagsNull
	primitive := true
	for _, constituent := range constituents {
		if constituent == nil || constituent.Flags()&checker.TypeFlagsInstantiable != 0 {
			return nil
		}
		if len(typeChecker.GetSignaturesOfType(constituent, checker.SignatureKindConstruct)) != 0 {
			return nil
		}
		if constituent.Flags()&primitiveFlags == 0 {
			primitive = false
		}
	}
	kind := "object"
	if primitive {
		kind = "primitive"
	}
	return &typefacts.NotCallableValue{Kind: kind, Type: typeChecker.TypeToString(value)}
}
