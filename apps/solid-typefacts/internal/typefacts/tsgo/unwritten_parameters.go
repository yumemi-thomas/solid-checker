package tsgo

import (
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts"
)

func (p *project) unwrittenParameterBindingsLocked(implementation *ast.Node, signature *typefacts.SelectedSignature, demand typefacts.Location) []typefacts.UnwrittenParameterBinding {
	var bindings []typefacts.UnwrittenParameterBinding
	completion := implementationCompletionForm(implementation)
	if signature == nil || (completion != typefacts.CompletionPlain && completion != typefacts.CompletionAsync) {
		return nil
	}
	for index, parameter := range implementation.Parameters() {
		// Reuse the source-identity predicate already used by returned parameters.
		// It rejects writes (including nested writes), defaults, rest, destructuring,
		// duplicate names, arguments/eval and generator implementations. Async
		// result wrapping is irrelevant to this lexical binding premise.
		identity := p.unwrittenParameterIdentityLocked(implementation, parameter.Name())
		if identity == nil || identity.ParameterIndex != index || len(identity.Path) != 0 {
			continue
		}
		// An exported alias or overload can select a signature whose declarations
		// are not this implementation's parameters. Such a signature cannot bind
		// this additional premise; retain the existing transcript without it.
		if index >= len(signature.Parameters) {
			continue
		}
		selected := signature.Parameters[index]
		location := nodeLocation(parameter.Name())
		if selected.Index != index || selected.Rest || selected.Defaulted || selected.Declaration == nil || selected.Declaration.Location != location || location.Path != demand.Path {
			continue
		}
		bindings = append(bindings, typefacts.UnwrittenParameterBinding{
			ParameterIndex: index, Declaration: location,
		})
	}
	return bindings
}
