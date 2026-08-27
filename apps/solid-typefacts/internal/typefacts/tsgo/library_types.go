package tsgo

import (
	"sort"

	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
)

// libraryTypesAtLocked names the standard-library types the type at exactly the
// demanded expression is built from, at its top level.
//
// "Top level" means the type itself, its union and intersection constituents,
// and one array-element unwrap — not the properties of an object type, and not a
// generic's type arguments beyond that unwrap. A consumer asking "is this value
// one of these well-known runtime types" gets an answer that does not depend on
// how the type was spelled: an alias, an import, and the built-in written
// directly all resolve to the same name.
//
// Only declarations the compiler considers default-library files count, so a
// user-defined `Map` is not the global `Map`. Names are sorted and deduplicated
// so the fact is a set, and absence means nothing at the top level came from the
// standard library.
func (p *project) libraryTypesAtLocked(node *ast.Node, evidence *semanticEvidence) []string {
	if node == nil {
		return nil
	}
	value := p.checker.GetTypeAtLocation(node)
	if value == nil {
		return nil
	}
	names := make(map[string]struct{})
	for _, constituent := range value.Distributed() {
		p.collectLibraryTypesLocked(constituent, names, true)
	}
	if len(names) == 0 {
		return nil
	}
	evidence.descriptor(p.typeDescriptorFor(value))
	sorted := make([]string, 0, len(names))
	for name := range names {
		sorted = append(sorted, name)
	}
	sort.Strings(sorted)
	return sorted
}

// collectLibraryTypesLocked records value's own standard-library name, then
// descends exactly once: through intersection members, which are as top-level as
// union members, and through an array's element type, since `Date[]` carries
// Dates just as plainly as `Date` does.
func (p *project) collectLibraryTypesLocked(
	value *checker.Type,
	names map[string]struct{},
	descend bool,
) {
	if value == nil {
		return
	}
	if flags := value.Flags(); flags&checker.TypeFlagsIntersection != 0 {
		for _, member := range value.Types() {
			p.collectLibraryTypesLocked(member, names, descend)
		}
		return
	}
	if name := p.libraryTypeNameLocked(value); name != "" {
		names[name] = struct{}{}
	}
	if !descend {
		return
	}
	if checker.Checker_isArrayOrTupleType(p.checker, value) {
		for _, element := range checker.Checker_getTypeArguments(p.checker, value) {
			for _, constituent := range element.Distributed() {
				p.collectLibraryTypesLocked(constituent, names, false)
			}
		}
	}
}

// libraryTypeNameLocked is value's declared name when its declaration lives in a
// default-library file, and "" otherwise. Anonymous object and function types
// have no symbol and answer "" — which is why a nested `{ when: Date }` reports
// nothing without any special case.
func (p *project) libraryTypeNameLocked(value *checker.Type) string {
	symbol := value.Symbol()
	if symbol == nil {
		return ""
	}
	for _, declaration := range symbol.Declarations {
		sourceFile := ast.GetSourceFileOfNode(declaration)
		if sourceFile == nil {
			continue
		}
		if p.program.IsSourceFileDefaultLibrary(sourceFile.Path()) {
			return symbol.Name
		}
	}
	return ""
}
