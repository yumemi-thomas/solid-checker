package tsgo

import (
	"github.com/microsoft/typescript-go/shim/ast"

	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts"
)

// defaultLibraryAliasFactLocked answers ADR 0103's question for one exported
// binding: is this export *the same function object* as a member of a
// default-library container?
//
//	export const keys = Object.keys;
//	export const floor = Math.floor;
//
// Both are re-exports of a built-in by identity, and both leave the export's
// call transcript open today -- `keys` with `callSignatureNotUnique` because
// `Object.keys` is overloaded and no signature can be selected, `floor` with
// `implementationUnavailable` because its only declaration is a body-less
// signature in `lib.es5.d.ts`. Neither refusal is about the runtime value,
// which is fully determined: it is the built-in.
//
// What is proven here, and nothing more:
//
//   - the binding is a variable declaration with an initializer, never
//     assigned anywhere in the file (`symbolIsAssignedLocked`);
//   - its initializer, after identity-preserving unwrapping, is a property
//     access whose object is a plain identifier -- a computed member states
//     nothing, because which member it reads is not syntactically fixed;
//   - the member symbol and the container symbol are both declared *only* in
//     default-library files, and the member's declaration sits inside the
//     named container (`isDefaultLibraryMemberLocked` with a container set of
//     one), so a local `const Object = {...}` shadowing the global answers
//     nothing;
//   - neither the container nor the member is written to, deleted, or used
//     as anything but a read or a call anywhere in the file
//     (`immutableAliasLibrarySourceIsStable`), so this file did not rewrite
//     the value it is aliasing before the alias was taken.
//
// What is deliberately *not* decided here is whether the named member is safe
// for a consumer to close a call domain on. `Object.keys` allocates an array
// and invokes nothing of its caller's; `Array.prototype.map` invokes a
// caller-supplied callback on every element. Both would be stated identically
// by this function, so the reviewed table lives with the consumer that draws
// the conclusion, and an unnamed member is "not stated" there.
func (p *project) defaultLibraryAliasFactLocked(target *ast.Symbol) *typefacts.DefaultLibraryAlias {
	if target == nil || target.ValueDeclaration == nil || !ast.IsVariableDeclaration(target.ValueDeclaration) {
		return nil
	}
	binding := target.ValueDeclaration
	if p.symbolIsAssignedLocked(target, binding) {
		return nil
	}
	initializer := identityPreservingUnwrap(binding.Initializer())
	if initializer == nil || !ast.IsPropertyAccessExpression(initializer) {
		return nil
	}
	object := identityPreservingUnwrap(initializer.Expression())
	if object == nil || !ast.IsIdentifier(object) || initializer.Name() == nil {
		return nil
	}
	file := ast.GetSourceFileOfNode(binding)
	if file == nil {
		return nil
	}
	container := p.canonicalSymbol(p.checker.GetSymbolAtLocation(object))
	if container == nil || !p.isDefaultLibraryMemberLocked(container, container.Name, nil) {
		return nil
	}
	member := p.canonicalSymbol(p.checker.GetSymbolAtLocation(initializer))
	if member == nil {
		return nil
	}
	// The member has to be declared inside *this* container, not merely
	// somewhere in the default library: `Object.keys` and a same-named member
	// of another interface are different values, and the container set is how
	// that is checked.
	containers := map[string]struct{}{containerInterfaceName(container.Name): {}}
	if !p.isDefaultLibraryMemberLocked(member, initializer.Name().Text(), containers) {
		return nil
	}
	if !p.immutableAliasLibrarySourceIsStable(file, member, container) {
		return nil
	}
	return &typefacts.DefaultLibraryAlias{
		Container: container.Name,
		Member:    initializer.Name().Text(),
	}
}

// containerInterfaceName maps a default-library value's name to the interface
// its members are declared on. `Object.keys` is declared on `ObjectConstructor`
// rather than on `Object`; `Math.floor` is declared on `Math`, which is both
// the value and the interface. A name with no known mapping answers itself,
// which then fails the container check rather than passing it loosely.
func containerInterfaceName(name string) string {
	switch name {
	case "Object":
		return "ObjectConstructor"
	case "Array":
		return "ArrayConstructor"
	case "Number":
		return "NumberConstructor"
	case "String":
		return "StringConstructor"
	case "JSON":
		return "JSON"
	case "Math":
		return "Math"
	default:
		return name
	}
}
