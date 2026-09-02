package tsgo

import (
	"path/filepath"
	"strings"

	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts"
)

// runtimeBindingKindLocked closes the runtime kind of one exact identifier by
// following its canonical symbol to the runtime declaration and inspecting
// every direct write to that binding. It deliberately does not treat property
// mutation (value.x = ...) as reassignment of value. A write whose value is
// dynamic stays open; disagreement between closed writes stays mixed.
func (p *project) runtimeBindingKindLocked(node *ast.Node) typefacts.RuntimeBindingKind {
	if node == nil || !ast.IsIdentifier(node) {
		return typefacts.RuntimeBindingAbsent
	}
	symbol := p.checker.GetSymbolAtLocation(node)
	if symbol == nil {
		return typefacts.RuntimeBindingOpen
	}
	return p.runtimeSymbolKindLocked(symbol, make(map[*ast.Symbol]struct{}))
}

func mergeRuntimeBindingKinds(left, right typefacts.RuntimeBindingKind) typefacts.RuntimeBindingKind {
	if left == typefacts.RuntimeBindingAbsent {
		return right
	}
	if right == typefacts.RuntimeBindingAbsent || left == right {
		return left
	}
	if left == typefacts.RuntimeBindingOpen || right == typefacts.RuntimeBindingOpen {
		return typefacts.RuntimeBindingOpen
	}
	return typefacts.RuntimeBindingMixed
}

func (p *project) runtimeSymbolKindLocked(
	symbol *ast.Symbol,
	seen map[*ast.Symbol]struct{},
) typefacts.RuntimeBindingKind {
	for symbol != nil && symbol.Flags&ast.SymbolFlagsAlias != 0 {
		if imported, exact := p.runtimeImportedAliasKindLocked(symbol, seen); exact {
			return imported
		}
		next := p.checker.GetImmediateAliasedSymbol(symbol)
		if next == nil || next == symbol {
			return typefacts.RuntimeBindingOpen
		}
		symbol = next
	}
	if symbol == nil {
		return typefacts.RuntimeBindingOpen
	}
	if symbol.Flags&ast.SymbolFlagsValue == 0 {
		return typefacts.RuntimeBindingOpen
	}
	if _, cycling := seen[symbol]; cycling {
		return typefacts.RuntimeBindingOpen
	}
	seen[symbol] = struct{}{}
	defer delete(seen, symbol)

	declaration := symbol.ValueDeclaration
	if declaration == nil && len(symbol.Declarations) != 0 {
		declaration = symbol.Declarations[0]
	}
	if declaration == nil {
		return typefacts.RuntimeBindingOpen
	}
	sourceFile := ast.GetSourceFileOfNode(declaration)
	if sourceFile == nil || sourceFile.IsDeclarationFile {
		return p.runtimeExpressionKindLocked(declaration, seen)
	}

	kind := typefacts.RuntimeBindingAbsent
	switch {
	case ast.IsFunctionDeclaration(declaration), ast.IsClassDeclaration(declaration):
		kind = typefacts.RuntimeBindingCallable
	case ast.IsEnumDeclaration(declaration), ast.IsModuleDeclaration(declaration):
		// Both declarations materialize namespace-like objects at runtime.
		kind = typefacts.RuntimeBindingNonCallable
	case ast.IsVariableDeclaration(declaration):
		initializer := declaration.AsVariableDeclaration().Initializer
		if initializer == nil {
			// `var x` and `let x` begin as undefined. Later writes are folded in
			// below; retaining this state matters for conditionally written vars.
			kind = typefacts.RuntimeBindingNonCallable
		} else {
			kind = p.runtimeExpressionKindLocked(initializer, seen)
		}
	default:
		kind = p.runtimeExpressionKindLocked(declaration, seen)
	}

	// A module-scope exported binding cannot be reassigned from another source
	// file. Scan its complete declaring file, including nested closures, because
	// any such closure may execute after import. Symbol equality, not spelling,
	// distinguishes shadowed identifiers.
	var visit func(*ast.Node) bool
	visit = func(current *ast.Node) bool {
		if ast.IsIdentifier(current) {
			candidate := p.canonicalSymbol(p.checker.GetSymbolAtLocation(current))
			if candidate == symbol {
				target := ast.GetAssignmentTarget(current)
				if target != nil {
					write := typefacts.RuntimeBindingOpen
					switch {
					case ast.IsBinaryExpression(target):
						binary := target.AsBinaryExpression()
						if ast.IsIdentifier(binary.Left) && binary.Left == current {
							if binary.OperatorToken.Kind == ast.KindEqualsToken ||
								ast.IsLogicalOrCoalescingAssignmentOperator(binary.OperatorToken.Kind) {
								write = p.runtimeExpressionKindLocked(binary.Right, seen)
							} else {
								// Arithmetic/bitwise/update assignment always writes a
								// primitive, even when its operands were any.
								write = typefacts.RuntimeBindingNonCallable
							}
						}
					case ast.IsPrefixUnaryExpression(target), ast.IsPostfixUnaryExpression(target):
						write = typefacts.RuntimeBindingNonCallable
					}
					kind = mergeRuntimeBindingKinds(kind, write)
				}
			}
		}
		current.ForEachChild(visit)
		return false
	}
	for _, statement := range sourceFile.Statements.Nodes {
		visit(statement)
	}
	if kind == typefacts.RuntimeBindingAbsent {
		return typefacts.RuntimeBindingOpen
	}
	return kind
}

// runtimeImportedAliasKindLocked follows only an explicit relative runtime
// module specifier to the exact source file already held by the configured
// program. TypeScript commonly redirects `./x.mjs` to `./x.d.mts`; using that
// declaration target would turn published typings into runtime proof. The
// literal runtime path plus program membership avoids that substitution. No
// extension probing, package lookup, basename pairing, or name-only search is
// performed, and anything outside this exact case stays open.
func (p *project) runtimeImportedAliasKindLocked(
	symbol *ast.Symbol,
	seen map[*ast.Symbol]struct{},
) (typefacts.RuntimeBindingKind, bool) {
	if symbol == nil || symbol.Flags&ast.SymbolFlagsAlias == 0 {
		return typefacts.RuntimeBindingAbsent, false
	}
	for _, declaration := range symbol.Declarations {
		importedName := ""
		switch {
		case ast.IsImportSpecifier(declaration):
			specifier := declaration.AsImportSpecifier()
			if specifier.IsTypeOnly {
				continue
			}
			if specifier.PropertyName != nil {
				importedName = specifier.PropertyName.Text()
			} else if name := specifier.Name(); name != nil {
				importedName = name.Text()
			}
		case ast.IsImportClause(declaration):
			if declaration.Name() == nil {
				continue
			}
			importedName = "default"
		case ast.IsNamespaceImport(declaration):
			// A module namespace exotic object is noncallable by language
			// definition, independently of any member it exposes.
			return typefacts.RuntimeBindingNonCallable, true
		default:
			continue
		}
		if importedName == "" {
			continue
		}
		owner := declaration.Parent
		for owner != nil && !ast.IsImportDeclaration(owner) {
			owner = owner.Parent
		}
		if owner == nil {
			continue
		}
		moduleSpecifier := owner.AsImportDeclaration().ModuleSpecifier
		if moduleSpecifier == nil {
			continue
		}
		text := moduleSpecifier.Text()
		if (!strings.HasPrefix(text, "./") && !strings.HasPrefix(text, "../")) ||
			filepath.Ext(text) == "" {
			continue
		}
		importer := ast.GetSourceFileOfNode(declaration)
		if importer == nil {
			continue
		}
		targetPath := filepath.Clean(filepath.Join(filepath.Dir(importer.FileName()), text))
		targetSource := p.program.GetSourceFile(targetPath)
		if targetSource == nil || targetSource.IsDeclarationFile || targetSource.Symbol == nil {
			continue
		}
		for _, exported := range p.checker.GetExportsOfModule(targetSource.Symbol) {
			if exported.Name == importedName {
				return p.runtimeSymbolKindLocked(exported, seen), true
			}
		}
		return typefacts.RuntimeBindingOpen, true
	}
	return typefacts.RuntimeBindingAbsent, false
}

func (p *project) runtimeExpressionKindLocked(
	node *ast.Node,
	seen map[*ast.Symbol]struct{},
) typefacts.RuntimeBindingKind {
	if node == nil {
		return typefacts.RuntimeBindingOpen
	}
	value := p.checker.GetTypeAtLocation(node)
	callability := callabilityOfType(p.checker, value)
	constructability := constructabilityOfType(p.checker, value)
	if callability == typefacts.CallabilityCallable ||
		callability == typefacts.CallabilityUntypedCallable ||
		constructability == typefacts.ConstructabilityConstructable {
		return typefacts.RuntimeBindingCallable
	}
	if callability == typefacts.CallabilityNonCallable &&
		constructability == typefacts.ConstructabilityNonConstructable {
		return typefacts.RuntimeBindingNonCallable
	}
	if callability == typefacts.CallabilityMixed || constructability == typefacts.ConstructabilityMixed {
		return typefacts.RuntimeBindingMixed
	}

	switch {
	case ast.IsParenthesizedExpression(node):
		return p.runtimeExpressionKindLocked(node.AsParenthesizedExpression().Expression, seen)
	case ast.IsAsExpression(node):
		return p.runtimeExpressionKindLocked(node.AsAsExpression().Expression, seen)
	case ast.IsSatisfiesExpression(node):
		return p.runtimeExpressionKindLocked(node.AsSatisfiesExpression().Expression, seen)
	case ast.IsNonNullExpression(node):
		return p.runtimeExpressionKindLocked(node.AsNonNullExpression().Expression, seen)
	case ast.IsArrowFunction(node), ast.IsFunctionExpression(node), ast.IsClassExpression(node):
		return typefacts.RuntimeBindingCallable
	case ast.IsObjectLiteralExpression(node), ast.IsArrayLiteralExpression(node),
		ast.IsStringLiteral(node), ast.IsNumericLiteral(node),
		ast.IsNoSubstitutionTemplateLiteral(node):
		return typefacts.RuntimeBindingNonCallable
	case ast.IsPrefixUnaryExpression(node), ast.IsPostfixUnaryExpression(node):
		return typefacts.RuntimeBindingNonCallable
	case ast.IsConditionalExpression(node):
		expression := node.AsConditionalExpression()
		return mergeRuntimeBindingKinds(
			p.runtimeExpressionKindLocked(expression.WhenTrue, seen),
			p.runtimeExpressionKindLocked(expression.WhenFalse, seen),
		)
	case ast.IsBinaryExpression(node):
		expression := node.AsBinaryExpression()
		switch expression.OperatorToken.Kind {
		case ast.KindEqualsToken, ast.KindCommaToken:
			return p.runtimeExpressionKindLocked(expression.Right, seen)
		case ast.KindAmpersandAmpersandToken:
			// `left && right` can expose left only when left is falsy. Every
			// callable/constructable JavaScript value is truthy, so the exposed
			// short-circuit partition is necessarily noncallable. This is what
			// closes chains such as `!!host && host.fn && host.fn.name === x`
			// without trusting the unknown host property.
			return mergeRuntimeBindingKinds(
				typefacts.RuntimeBindingNonCallable,
				p.runtimeExpressionKindLocked(expression.Right, seen),
			)
		case ast.KindBarBarToken, ast.KindQuestionQuestionToken:
			return mergeRuntimeBindingKinds(
				p.runtimeExpressionKindLocked(expression.Left, seen),
				p.runtimeExpressionKindLocked(expression.Right, seen),
			)
		default:
			// All remaining JavaScript binary operators produce primitives.
			return typefacts.RuntimeBindingNonCallable
		}
	case ast.IsIdentifier(node):
		return p.runtimeSymbolKindLocked(p.checker.GetSymbolAtLocation(node), seen)
	case ast.IsCallExpression(node):
		return p.runtimeIIFEKindLocked(node, seen)
	default:
		return typefacts.RuntimeBindingOpen
	}
}

func (p *project) runtimeIIFEKindLocked(
	call *ast.Node,
	seen map[*ast.Symbol]struct{},
) typefacts.RuntimeBindingKind {
	callee := call.Expression()
	for callee != nil && ast.IsParenthesizedExpression(callee) {
		callee = callee.AsParenthesizedExpression().Expression
	}
	if callee == nil || (!ast.IsArrowFunction(callee) && !ast.IsFunctionExpression(callee)) {
		// An external helper's result is intentionally not inferred from its
		// name, declaration text, or arguments.
		return typefacts.RuntimeBindingOpen
	}
	body := callee.Body()
	if body == nil {
		return typefacts.RuntimeBindingOpen
	}
	if !ast.IsBlock(body) {
		return p.runtimeExpressionKindLocked(body, seen)
	}

	kind := typefacts.RuntimeBindingAbsent
	var collect func(*ast.Node) bool
	collect = func(current *ast.Node) bool {
		if current != body && (ast.IsArrowFunction(current) || ast.IsFunctionExpression(current) ||
			ast.IsFunctionDeclaration(current) || ast.IsClassDeclaration(current)) {
			return false
		}
		if ast.IsReturnStatement(current) {
			if expression := current.Expression(); expression != nil {
				kind = mergeRuntimeBindingKinds(kind, p.runtimeExpressionKindLocked(expression, seen))
			} else {
				kind = mergeRuntimeBindingKinds(kind, typefacts.RuntimeBindingNonCallable)
			}
			return false
		}
		current.ForEachChild(collect)
		return false
	}
	body.ForEachChild(collect)
	statements := body.AsBlock().Statements.Nodes
	if len(statements) == 0 || !ast.IsReturnStatement(statements[len(statements)-1]) {
		// Without a terminal return, falling through to undefined remains a
		// possible noncallable value. This may over-refuse unreachable tails but
		// can never manufacture a closed kind.
		kind = mergeRuntimeBindingKinds(kind, typefacts.RuntimeBindingNonCallable)
	}
	if kind == typefacts.RuntimeBindingAbsent {
		return typefacts.RuntimeBindingNonCallable
	}
	return kind
}
