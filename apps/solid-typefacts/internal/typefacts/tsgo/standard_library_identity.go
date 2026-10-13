package tsgo

import (
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
)

// maxStandardLibraryReceiverDepth bounds the member chain
// standardLibraryIdentityLocked walks from a callee back to its root, and the
// chain of bindings a fresh value is followed through. Past it the answer is
// false, which only refuses more.
const maxStandardLibraryReceiverDepth = 8

// standardLibraryIdentityLocked answers whether the value a call invokes is
// the default-library declaration it resolves to **by identity**, rather than
// by the declared type of whatever binding the callee is read through (ADR
// 0149, handshake protocol 68).
//
// Resolution alone is not that proof. The checker resolves `m.min(x)` to
// `Math.min` whenever `m`'s type is `Math`, and in a JavaScript file `let m =
// Math; … m = { min: run };` keeps that type; so does `let s = ""; … s = {
// split: run }` for `s.split()`. The call then runs code the census never saw.
// The callee is the declaration only when its receiver is one of:
//
//   - nothing: an identifier callee (`setTimeout(…)`, `new Map()`) that resolves
//     to default-library declarations alone and that the file never writes,
//     deletes or lets escape;
//   - a default-library global: a non-computed, non-optional member chain
//     (`Math.min`, `Object.prototype.toString.call`) whose root identifier and
//     every member resolve to default-library declarations alone, none of them
//     written, deleted or escaped in the file (unstableLibrarySymbolsLocked);
//   - a fresh built-in value (freshBuiltinReceiverLocked), whose own member is
//     read, or a binding that holds only such values and that no other code
//     can reach (freshBindingLocked).
//
// Every other receiver -- a parameter, any other binding, a member read, an
// arbitrary call's result -- states nothing: its value is only as good as its
// type, and the type is only its declaration's.
func (p *project) standardLibraryIdentityLocked(call *ast.Node) bool {
	if call == nil || p.checker == nil || (ast.IsCallExpression(call) && call.QuestionDotToken() != nil) {
		return false
	}
	file := ast.GetSourceFileOfNode(call)
	callee := identityPreservingUnwrap(call.Expression())
	if file == nil || callee == nil {
		return false
	}
	if ast.IsIdentifier(callee) {
		return p.stableLibraryGlobalLocked(file, callee)
	}
	if !ast.IsPropertyAccessExpression(callee) {
		return false
	}
	unstable := p.unstableLibrarySymbolsLocked(file)
	var chain []*ast.Symbol
	node := callee
	for range maxStandardLibraryReceiverDepth {
		if !ast.IsPropertyAccessExpression(node) {
			break
		}
		if node.QuestionDotToken() != nil {
			return false
		}
		member := p.canonicalSymbol(p.formChecker().GetSymbolAtLocation(node))
		if member == nil || !p.isDefaultLibraryMemberLocked(member, member.Name, nil) {
			return false
		}
		if _, moved := unstable[member]; moved {
			return false
		}
		chain = append(chain, member)
		node = identityPreservingUnwrap(node.Expression())
		if node == nil {
			return false
		}
	}
	if ast.IsPropertyAccessExpression(node) {
		return false
	}
	if ast.IsIdentifier(node) {
		root := p.canonicalSymbol(p.formChecker().GetSymbolAtLocation(node))
		if root == nil {
			return false
		}
		if p.isDefaultLibraryMemberLocked(root, root.Name, nil) {
			_, moved := unstable[root]
			return !moved
		}
		return len(chain) == 1 && p.freshBindingLocked(file, root, 0)
	}
	// A fresh value's own member, one level deep: a deeper chain reads a
	// member of a member, which is not a value the expression made.
	return len(chain) == 1 && p.freshBuiltinReceiverLocked(file, node, 0)
}

// stableLibraryGlobalLocked answers whether an identifier resolves to
// default-library declarations alone and the file neither writes, deletes nor
// lets escape it.
func (p *project) stableLibraryGlobalLocked(file *ast.SourceFile, identifier *ast.Node) bool {
	symbol := p.canonicalSymbol(p.formChecker().GetSymbolAtLocation(identifier))
	if symbol == nil || !p.isDefaultLibraryMemberLocked(symbol, symbol.Name, nil) {
		return false
	}
	_, moved := p.unstableLibrarySymbolsLocked(file)[symbol]
	return !moved
}

// unstableLibrarySymbolsLocked answers, once per file, the default-library
// symbols the file could have replaced: every one it writes or deletes -- as a
// global (`setTimeout = f`) or as a member (`Math.min = f`, `delete
// Array.prototype.push`) -- and every global identifier it hands out as a
// value (`f(Math)`, `const M = Math`), since whoever receives it may replace a
// member. A global used as the object of a member read, as the callee of a
// call or `new`, as the right operand of `instanceof`, or as the operand of
// `typeof` hands nothing out.
func (p *project) unstableLibrarySymbolsLocked(file *ast.SourceFile) map[*ast.Symbol]struct{} {
	if unstable, known := p.libraryStability[file]; known {
		return unstable
	}
	unstable := make(map[*ast.Symbol]struct{})
	written := func(node *ast.Node) bool {
		return ast.GetAssignmentTarget(node) != nil ||
			(node.Parent != nil && nodeKindName(node.Parent) == "DeleteExpression")
	}
	library := func(node *ast.Node) *ast.Symbol {
		symbol := p.canonicalSymbol(p.formChecker().GetSymbolAtLocation(node))
		if symbol == nil || !p.isDefaultLibraryMemberLocked(symbol, symbol.Name, nil) {
			return nil
		}
		return symbol
	}
	var visit func(*ast.Node)
	visit = func(node *ast.Node) {
		if node == nil || ast.IsPartOfTypeNode(node) {
			return
		}
		switch {
		case ast.IsPropertyAccessExpression(node):
			if written(node) {
				if symbol := library(node); symbol != nil {
					unstable[symbol] = struct{}{}
				}
			}
		case ast.IsIdentifier(node):
			parent := node.Parent
			if parent != nil && ast.IsPropertyAccessExpression(parent) && parent.Name() == node {
				break
			}
			symbol := library(node)
			if symbol == nil {
				break
			}
			if written(node) || !inertLibraryUse(node) {
				unstable[symbol] = struct{}{}
			}
		}
		node.ForEachChild(func(child *ast.Node) bool {
			visit(child)
			return false
		})
	}
	visit(file.AsNode())
	if p.libraryStability == nil {
		p.libraryStability = make(map[*ast.SourceFile]map[*ast.Symbol]struct{})
	}
	p.libraryStability[file] = unstable
	return unstable
}

// inertLibraryUse answers whether a global identifier's use hands out no
// reference to it: see unstableLibrarySymbolsLocked.
func inertLibraryUse(node *ast.Node) bool {
	parent := node.Parent
	for parent != nil && ast.IsParenthesizedExpression(parent) {
		node, parent = parent, parent.Parent
	}
	switch {
	case parent == nil:
		return false
	case ast.IsPropertyAccessExpression(parent):
		return parent.Expression() == node
	case ast.IsCallExpression(parent), ast.IsNewExpression(parent):
		return parent.Expression() == node
	case ast.IsBinaryExpression(parent):
		binary := parent.AsBinaryExpression()
		return binary != nil && binary.OperatorToken != nil && binary.Right == node &&
			nodeKindName(binary.OperatorToken) == "InstanceOfKeyword"
	}
	return nodeKindName(parent) == "TypeOfExpression"
}

// freshBuiltinReceiverLocked answers whether an expression makes a fresh
// built-in value there and then, so no other code runs between its creation
// and the member read: a primitive by grammar (primitiveBySyntaxLocked, which
// reaches a never-written `const` over one), an array or regular-expression
// literal, `new G(…)` of a built-in constructor by identity, or a call that itself
// invokes a built-in by identity and whose *declared* result -- the signature
// before any instantiation over the arguments' types -- is a primitive
// (`Date.now().toString(36)`: a primitive has no own members, so its method is
// the wrapper prototype's) or an array (`Array.from(items)`: a built-in hands
// back a fresh array, or its own receiver, which is then itself fresh).
func (p *project) freshBuiltinReceiverLocked(file *ast.SourceFile, node *ast.Node, depth int) bool {
	node = identityPreservingUnwrap(node)
	switch {
	case node == nil || depth > maxStandardLibraryReceiverDepth:
		return false
	case p.primitiveBySyntaxLocked(node, 0):
		return true
	case ast.IsArrayLiteralExpression(node), nodeKindName(node) == "RegularExpressionLiteral":
		return true
	case ast.IsNewExpression(node):
		// `new Map()` or `new Intl.Locale(…)`: the constructor is itself a
		// built-in by identity, and `new` hands back a fresh instance of it.
		return p.standardLibraryIdentityLocked(node)
	case ast.IsCallExpression(node):
		return node.QuestionDotToken() == nil && p.standardLibraryIdentityLocked(node) &&
			p.declaredFreshResultLocked(node)
	}
	return false
}

// freshBindingLocked answers whether a binding holds only fresh built-in
// values that no other code can reach: declared exactly once, in the reading
// file, with a plain name, initialized by a fresh value
// (freshBuiltinReceiverLocked), written only by a plain `=` of another fresh
// value (`let stack = []; … stack = []`), and otherwise referenced only as the
// object of a non-computed member read that is itself neither written nor
// deleted -- so nothing can give the value an own member or a new prototype.
// `const values = Array.from(items); values.map(callback)` is one; `return
// values`, `f(values)`, `values[0] = x` and `values.map = f` each make it none.
func (p *project) freshBindingLocked(file *ast.SourceFile, symbol *ast.Symbol, depth int) bool {
	if depth > maxStandardLibraryReceiverDepth || symbol == nil || len(symbol.Declarations) != 1 {
		return false
	}
	if fresh, known := p.freshConsts[symbol]; known {
		return fresh
	}
	if p.freshConsts == nil {
		p.freshConsts = make(map[*ast.Symbol]bool)
	}
	// A binding whose freshness depends on itself is not proved fresh.
	p.freshConsts[symbol] = false
	fresh := p.freshBindingUncachedLocked(file, symbol, depth)
	p.freshConsts[symbol] = fresh
	return fresh
}

func (p *project) freshBindingUncachedLocked(file *ast.SourceFile, symbol *ast.Symbol, depth int) bool {
	declaration := symbol.Declarations[0]
	if declaration == nil || ast.GetSourceFileOfNode(declaration) != file ||
		!ast.IsVariableDeclaration(declaration) ||
		declaration.Name() == nil || !ast.IsIdentifier(declaration.Name()) ||
		!p.freshBuiltinReceiverLocked(file, declaration.Initializer(), depth+1) {
		return false
	}
	name := declaration.Name()
	contained := true
	var visit func(*ast.Node)
	visit = func(node *ast.Node) {
		if node == nil || !contained || ast.IsPartOfTypeNode(node) {
			return
		}
		if ast.IsIdentifier(node) && node != name &&
			p.canonicalSymbol(p.formChecker().GetSymbolAtLocation(node)) == symbol {
			parent := node.Parent
			switch {
			case parent == nil:
				contained = false
			case ast.IsBinaryExpression(parent) && parent.AsBinaryExpression().Left == node &&
				parent.AsBinaryExpression().OperatorToken != nil &&
				nodeKindName(parent.AsBinaryExpression().OperatorToken) == "EqualsToken":
				// A plain write of another fresh value.
				if !p.freshBuiltinReceiverLocked(file, parent.AsBinaryExpression().Right, depth+1) {
					contained = false
				}
			case ast.IsPropertyAccessExpression(parent) && parent.Expression() == node &&
				ast.GetAssignmentTarget(parent) == nil &&
				(parent.Parent == nil || nodeKindName(parent.Parent) != "DeleteExpression"):
			default:
				contained = false
			}
			if !contained {
				return
			}
		}
		node.ForEachChild(func(child *ast.Node) bool {
			visit(child)
			return false
		})
	}
	visit(file.AsNode())
	return contained
}

// declaredPrimitiveResultLocked is declaredFreshResultLocked's primitive half
// alone: an array is fresh, but coercing one runs `Array.prototype.join` over
// its elements, which is not a primitive's nothing.
func (p *project) declaredPrimitiveResultLocked(call *ast.Node) bool {
	returned, generic := p.declaredResultLocked(call)
	return returned != nil && !generic && !p.mayBeObjectTypedLocked(returned)
}

// declaredResultLocked answers the uninstantiated result type of the signature
// a call resolves to, and whether that signature declares type parameters.
func (p *project) declaredResultLocked(call *ast.Node) (*checker.Type, bool) {
	signature := checker.Checker_getResolvedSignature(p.formChecker(), call, nil, checker.CheckModeNormal)
	for range maxStandardLibraryReceiverDepth {
		if signature == nil {
			return nil, false
		}
		target := signature.Target()
		if target == nil || target == signature {
			break
		}
		signature = target
	}
	if signature == nil {
		return nil, false
	}
	return checker.Checker_getReturnTypeOfSignature(p.formChecker(), signature), len(signature.TypeParameters()) != 0
}

// operandProvedPrimitiveLocked answers whether a coercion's operand is a
// primitive by proof rather than by its type alone (the 2026-09-28 amendment
// to ADR 0149): coercing it then reaches no `Symbol.toPrimitive`, `valueOf` or
// `toString` of anyone's. The type is not that proof in a JavaScript file:
// `let v = 0; … v = { valueOf: run }; v - 1` types `v` as `number` and runs
// `run`. One of these must hold:
//
//   - the operand is a primitive by grammar (primitiveBySyntaxLocked), which
//     needs no type at all;
//   - its type is non-object and it is a call of a built-in by identity
//     (standardLibraryIdentityLocked) whose uninstantiated signature declares a
//     primitive result (`Math.min(a, b)`, `Date.now()`);
//   - its type is non-object and it is a plain parameter of the implementation
//     being classified, on a premised twin, where its type is the declared
//     signature's (ADR 0038) -- never on the original program, where a
//     JavaScript parameter is typed by its default (`times = 1` is `number`
//     whatever the caller passes) or by a JSDoc nothing checks a caller
//     against;
//   - it is a never-written `const` whose initializer is proved a primitive by
//     one of these (`const rand = Math.random().toString(36)`);
//   - its type is non-object and it sits in TypeScript source, where the
//     checker holds every write to a binding to its declared type (ADR 0113's
//     amendment, with the same `any`-write trust stated there).
//
// A member read, a binding, and any other call are not: their type is only
// their declaration's.
func (p *project) operandProvedPrimitiveLocked(operand *ast.Node) bool {
	return p.operandProvedPrimitiveAtLocked(operand, 0)
}

func (p *project) operandProvedPrimitiveAtLocked(operand *ast.Node, depth int) bool {
	node := identityPreservingUnwrap(operand)
	if node == nil || depth > maxStandardLibraryReceiverDepth {
		return false
	}
	if p.primitiveBySyntaxLocked(node, 0) {
		return true
	}
	if p.mayBeObjectTypedLocked(p.formChecker().GetTypeAtLocation(node)) {
		return false
	}
	if isTypeScriptSourceFile(ast.GetSourceFileOfNode(node)) {
		return true
	}
	if ast.IsCallExpression(node) {
		return node.QuestionDotToken() == nil && p.standardLibraryIdentityLocked(node) &&
			p.declaredPrimitiveResultLocked(node)
	}
	if p.unwrittenOwnParameterLocked(node) {
		return true
	}
	initializer := p.unwrittenConstInitializerLocked(node)
	return initializer != nil && p.operandProvedPrimitiveAtLocked(initializer, depth+1)
}

// unwrittenConstInitializerLocked answers the initializer of the `const` an
// identifier names -- declared once, in the reading file, with a plain name,
// and written nowhere -- or nil.
func (p *project) unwrittenConstInitializerLocked(node *ast.Node) *ast.Node {
	if node == nil || !ast.IsIdentifier(node) {
		return nil
	}
	symbol := p.canonicalSymbol(p.formChecker().GetSymbolAtLocation(node))
	if symbol == nil || len(symbol.Declarations) != 1 {
		return nil
	}
	declaration := symbol.Declarations[0]
	if declaration == nil || ast.GetSourceFileOfNode(declaration) != ast.GetSourceFileOfNode(node) ||
		!ast.IsVariableDeclaration(declaration) || !ast.IsVarConst(declaration) ||
		declaration.Name() == nil || !ast.IsIdentifier(declaration.Name()) ||
		p.symbolIsAssignedLocked(symbol, declaration) {
		return nil
	}
	return declaration.Initializer()
}

// unwrittenOwnParameterLocked answers whether an identifier names, on a
// premised twin, a plain parameter of the implementation the forms census is
// classifying -- not a nested callable's, whose value its invoker chooses --
// declared once, whose
// default (if any) is a primitive by grammar, and which the file writes only
// with primitives (primitiveWriteLocked: `times += 1`, `--times`), so it holds
// the caller's argument or a primitive computed from one.
func (p *project) unwrittenOwnParameterLocked(node *ast.Node) bool {
	if node == nil || !ast.IsIdentifier(node) || p.formImplementation == nil ||
		p.formTwin == nil || ast.GetSourceFileOfNode(node) != p.formTwin.file {
		return false
	}
	symbol := p.canonicalSymbol(p.formChecker().GetSymbolAtLocation(node))
	if symbol == nil || len(symbol.Declarations) != 1 {
		return false
	}
	declaration := symbol.Declarations[0]
	if declaration == nil || !ast.IsParameterDeclaration(declaration) || declaration.Parent != p.formImplementation ||
		declaration.Name() == nil || !ast.IsIdentifier(declaration.Name()) {
		return false
	}
	if initializer := declaration.Initializer(); initializer != nil && !p.primitiveBySyntaxLocked(initializer, 0) {
		return false
	}
	if !p.symbolIsAssignedLocked(symbol, declaration) {
		return true
	}
	file := ast.GetSourceFileOfNode(declaration)
	return file != nil && p.everyWritePrimitiveLocked(file, symbol, declaration.Name(), 0)
}

// declaredFreshResultLocked answers whether the signature a call resolves to
// declares, before any instantiation, a primitive or an array result:
// `Date.now()`'s `number` and `Array.from`'s `T[]` do, `[x].at(0)`'s `T |
// undefined` does not, since its instantiated `number` would be only `x`'s
// declared type.
func (p *project) declaredFreshResultLocked(call *ast.Node) bool {
	returned, generic := p.declaredResultLocked(call)
	if returned == nil {
		return false
	}
	if checker.Checker_isArrayOrTupleType(p.formChecker(), returned) && !checker.IsTupleType(returned) {
		return true
	}
	return !generic && !p.mayBeObjectTypedLocked(returned)
}
