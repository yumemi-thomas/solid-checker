package tsgo

import (
	"path/filepath"

	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts"
)

// Host constants (ADR 0166, handshake protocol 72).
//
// A certification that declares a host sends, per program module, the named
// imports whose value that host's resolution fixes: `isServer` from
// `@solidjs/web` is `false` in every runtime file its `exports` map selects for
// `browser` and `true` in every one it selects for `node`. The program here
// resolves the same import to the declaration file, whose `export declare
// const isServer: boolean` says nothing, so the value can only arrive as a
// premise. The Rust client proves it from the runtime bytes; this side decides
// only which references it applies to.
//
// It applies to a reference the checker binds to exactly that import
// specifier: an identifier whose symbol is the alias declared by a named,
// value (not type-only) import specifier of the importing module, importing
// that name from that specifier. A shadowing local, a namespace member, a
// default import, or any other module's reference is untouched. A `const`
// initialized from the import is folded through literalTruthinessLocked's
// existing const indirection, and a written binding is not.

type hostConstantKey struct {
	importer  string
	specifier string
	name      string
}

var _ typefacts.HostConstantScoped = (*project)(nil)

// SetHostConstants replaces the host constants the next answers read.
func (p *project) SetHostConstants(constants []typefacts.HostConstant) {
	p.mu.Lock()
	defer p.mu.Unlock()
	if len(constants) == 0 {
		p.hostConstants = nil
		return
	}
	folds := make(map[hostConstantKey]bool, len(constants))
	for _, constant := range constants {
		key := hostConstantKey{
			importer:  filepath.Clean(constant.Importer),
			specifier: constant.Specifier,
			name:      constant.Name,
		}
		if previous, seen := folds[key]; seen && previous != constant.Value {
			// Two answers for one import: fold neither.
			delete(folds, key)
			continue
		}
		folds[key] = constant.Value
	}
	p.hostConstants = folds
}

// hostConstantTruthinessLocked is the value of `identifier` when it names a
// host-constant import binding, and whether it does.
func (p *project) hostConstantTruthinessLocked(identifier *ast.Node) (value bool, known bool) {
	if len(p.hostConstants) == 0 || identifier == nil || !ast.IsIdentifier(identifier) {
		return false, false
	}
	symbol := p.checker.GetSymbolAtLocation(identifier)
	if symbol == nil || symbol.Flags&ast.SymbolFlagsAlias == 0 || len(symbol.Declarations) != 1 {
		return false, false
	}
	declaration := symbol.Declarations[0]
	if !ast.IsImportSpecifier(declaration) {
		return false, false
	}
	specifier := declaration.AsImportSpecifier()
	if specifier.IsTypeOnly {
		return false, false
	}
	importedName := ""
	if specifier.PropertyName != nil {
		importedName = specifier.PropertyName.Text()
	} else if name := specifier.Name(); name != nil {
		importedName = name.Text()
	}
	owner := declaration.Parent
	for owner != nil && !ast.IsImportDeclaration(owner) {
		if ast.IsImportClause(owner) && owner.IsTypeOnly() {
			return false, false
		}
		owner = owner.Parent
	}
	if owner == nil {
		return false, false
	}
	moduleSpecifier := owner.AsImportDeclaration().ModuleSpecifier
	if moduleSpecifier == nil || !ast.IsStringLiteral(moduleSpecifier) {
		return false, false
	}
	importer := ast.GetSourceFileOfNode(declaration)
	if importer == nil || importer != ast.GetSourceFileOfNode(identifier) {
		return false, false
	}
	value, known = p.hostConstants[hostConstantKey{
		importer:  filepath.Clean(importer.FileName()),
		specifier: moduleSpecifier.Text(),
		name:      importedName,
	}]
	return value, known
}

// mayExitEarlyLocked reports whether a statement holds a `return`, `break` or
// `continue` of the enclosing callable -- one outside every callable nested in
// it (a `break` whose target lies inside the statement is counted too; the
// over-refusal claims nothing) -- read through decided conditions:
// an `if` whose condition literalTruthinessLocked decides contributes its
// condition and its live arm only. `if (isServer) return;` under a browser
// host has no path out (ADR 0166); `if (false) return;` never had one.
func (p *project) mayExitEarlyLocked(node *ast.Node) bool {
	if node == nil {
		return false
	}
	if ast.IsReturnStatement(node) || ast.IsBreakStatement(node) || isContinueStatement(node) {
		return true
	}
	if ast.IsIfStatement(node) {
		statement := node.AsIfStatement()
		if truthy, known := p.literalTruthinessLocked(statement.Expression, 0); known {
			if p.mayExitEarlyLocked(statement.Expression) {
				return true
			}
			if truthy {
				return p.mayExitEarlyLocked(statement.ThenStatement)
			}
			return p.mayExitEarlyLocked(statement.ElseStatement)
		}
	}
	found := false
	node.ForEachChild(func(child *ast.Node) bool {
		if found || isCallableDeclaration(child) {
			return false
		}
		if p.mayExitEarlyLocked(child) {
			found = true
		}
		return false
	})
	return found
}
