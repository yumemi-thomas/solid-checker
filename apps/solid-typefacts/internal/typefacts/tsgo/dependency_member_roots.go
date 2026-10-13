package tsgo

import (
	"github.com/microsoft/typescript-go/shim/ast"

	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts"
)

// dependencyMemberRootLocked answers the imported binding an identifier names,
// for ADR 0104's premise, or nil.
//
// **What this states, and what it deliberately does not.** It says only that
// the identifier is a name this module imported, by that name, from that
// specifier, and that this module never writes it. Whether the named export is
// an object whose own properties are data properties — which is the premise a
// consumer needs before it may excuse a property read of it — is not stated
// here and cannot be: the producer is looking at the *importing* module, whose
// source says nothing about what the exporting one built. That is a reviewed
// question, and it lives with the certifier, exactly as ADR 0103 leaves the
// reviewed default-library members there.
//
// Each condition below refuses rather than guesses:
//
//   - **A namespace import** (`import * as ns`) states nothing. `ns.member` is
//     a read of the module namespace object, whose properties are accessors
//     the specification installs, and the member this resolves to is a second
//     question on top of the one being asked.
//   - **A default import** states nothing. What a module's default export
//     holds is decided by the exporting module's own expression, and the name
//     `default` does not identify a reviewable member the way a named export
//     does.
//   - **An export-from re-export** (`export { x } from "m"`) is not an import
//     binding in this module at all and never reaches here.
//   - **A written local binding** states nothing. An import binding cannot be
//     assigned in conforming code, but the census does not rely on the
//     program being conforming where a cheap check is available.
//   - **A relative or absolute specifier** states nothing. It names a file of
//     this same artifact, which the artifact's own census already walks; the
//     premise is about a *dependency*, and handing a consumer "./util.js"
//     invites it to match a table entry against a path.
func (p *project) dependencyMemberRootLocked(node *ast.Node) *typefacts.ImportedModuleMember {
	if node == nil || !ast.IsIdentifier(node) {
		return nil
	}
	// The raw symbol, before the alias chain is walked: canonicalSymbol
	// answers the original declaration, so every import would look local.
	raw := p.formChecker().GetSymbolAtLocation(node)
	if raw == nil || raw.Flags&ast.SymbolFlagsAlias == 0 || len(raw.Declarations) == 0 {
		return nil
	}
	declaration := raw.Declarations[0]
	if declaration == nil || nodeKindName(declaration) != "ImportSpecifier" {
		return nil
	}
	if p.symbolIsAssignedLocked(raw, declaration) {
		return nil
	}
	// `import { a as b }` carries a propertyName of `a`; a plain `import { a }`
	// carries none and the name itself is the export's.
	name := declaration.Name()
	if propertyName := declaration.PropertyName(); propertyName != nil {
		name = propertyName
	}
	if name == nil || !ast.IsIdentifier(name) {
		return nil
	}
	specifier := importSpecifierModuleText(declaration)
	if specifier == "" || specifier[0] == '.' || specifier[0] == '/' {
		return nil
	}
	return &typefacts.ImportedModuleMember{Specifier: specifier, Name: name.Text()}
}

// importSpecifierModuleText answers the module specifier text of the import
// declaration an ImportSpecifier belongs to, or "" when the shape is not the
// plain `import { … } from "…"` this premise covers.
func importSpecifierModuleText(specifier *ast.Node) string {
	// ImportSpecifier -> NamedImports -> ImportClause -> ImportDeclaration.
	node := specifier.Parent
	for range 3 {
		if node == nil {
			return ""
		}
		if ast.IsImportDeclaration(node) {
			break
		}
		node = node.Parent
	}
	if node == nil || !ast.IsImportDeclaration(node) {
		return ""
	}
	moduleSpecifier := node.AsImportDeclaration().ModuleSpecifier
	if moduleSpecifier == nil || !ast.IsStringLiteral(moduleSpecifier) {
		return ""
	}
	return moduleSpecifier.Text()
}
