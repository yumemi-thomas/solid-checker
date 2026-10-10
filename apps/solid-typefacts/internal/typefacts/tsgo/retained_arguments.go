package tsgo

import (
	"github.com/microsoft/typescript-go/shim/ast"

	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts"
)

// retainedArgumentsLocked is ADR 0139's producer census of a class export's
// construction: the constructor parameters it keeps on the instance, under a
// fixed key, for its own members to call later (handshake protocol 65).
//
// It answers only for a constructor classConstructorAt already admitted --
// no heritage, no field initializer, no static block, no computed member name,
// no decorator, no parameter property -- and states a parameter only when the
// bytes prove all of this, answering nothing at all when the class is not
// exact:
//
//   - **The class is exact.** No static member; no nested class; every `this`
//     anywhere in the class body is the object of a property access or of an
//     element access with a literal key, so the instance never escapes as a
//     value (an escaping instance lets code that never received the
//     constructed value reach what it keeps); no member access of `this` has
//     a computed key; the constructor returns no value, so `new` hands back
//     the instance; and every reference to the class in the program is an
//     export specifier or the callee of a `new`, so nothing augments its
//     prototype (`C.prototype.m = …`), extends it or passes it on.
//   - **Its members are its own.** Every write of a member of `this` stores a
//     value no hidden code can hang on the instance: a constructor parameter,
//     a literal, an object or array literal, a `new` expression, a template,
//     or -- only as a top-level statement of the constructor -- a function
//     literal, which is then an installed member. A function stored anywhere
//     else, an identifier, a call result or a member read could be a function
//     this census cannot see, run later as a member with the instance as
//     `this`.
//   - **The parameter is kept, once.** It is a plain identifier binding, not
//     defaulted, not rest; its one store is a top-level `this.<key> = p`
//     statement of the constructor; and its every other use is a direct call
//     in the constructor's own frame (ADR 0100's item, which the call census
//     confirms on its own).
//   - **Only members call it, after construction.** The key names no class
//     element, is not `__proto__`, and is written nowhere else; every other
//     access of `this.<key>` is the callee of a call; and none of those calls
//     lies in code a construction reaches -- the constructor itself, any
//     closure it creates that it does not install, and, transitively, every
//     member whose key reached code names on `this`. A construction-time call
//     through a member (router-core's `this.update(…)` → `this.getStoreConfig`)
//     is not stated: that is a call item this census cannot yet follow to one
//     closure.
func (p *project) retainedArgumentsLocked(constructor *ast.Node) []typefacts.RetainedArgument {
	if constructor == nil || constructor.Body() == nil {
		return nil
	}
	class := constructor.Parent
	if class == nil || !(ast.IsClassDeclaration(class) || ast.IsClassExpression(class)) {
		return nil
	}
	// The runtime binding the class value is reached through: its own name for
	// `class C {}`, the declarator's for the bundler's `var C = class {}`. The
	// demand's symbol may be a declaration file's, which no runtime code
	// references.
	binding := p.classRuntimeBindingLocked(class)
	if binding == nil {
		return nil
	}
	heritage, members := classHeritageAndMembers(class)
	if heritage {
		return nil
	}
	// Class elements by key, and the bodies of the callable ones.
	declared := make(map[string]struct{})
	bodies := make(map[string][]*ast.Node)
	for _, member := range members {
		if member == nil {
			continue
		}
		kind := nodeKindName(member)
		if kind == "Constructor" {
			if member != constructor {
				// An overload signature, or a second body: not the one exact
				// constructor this census reads.
				if member.Body() != nil {
					return nil
				}
			}
			continue
		}
		if kind == "SemicolonClassElement" {
			continue
		}
		if hasStaticKeyword(member) {
			return nil
		}
		key, ok := classMemberKey(member.Name())
		if !ok {
			return nil
		}
		declared[key] = struct{}{}
		switch kind {
		case "MethodDeclaration", "GetAccessor", "SetAccessor":
			if member.Body() != nil {
				bodies[key] = append(bodies[key], member)
			}
		case "PropertyDeclaration":
			// classConstructorAt refused any initializer; a bare declaration
			// defines an own `undefined` and runs nothing.
		default:
			return nil
		}
	}
	// Installed members: top-level `this.<key> = <function literal>`
	// statements of the constructor.
	installations := make(map[*ast.Node]string)
	// The `this.<key>` an installation writes: defining a member runs none.
	installing := make(map[*ast.Node]struct{})
	statements := constructor.Body().AsBlock().Statements.Nodes
	for _, statement := range statements {
		key, left, right := thisStoreStatement(statement)
		if key == "" || right == nil {
			continue
		}
		if ast.IsArrowFunction(right) || ast.IsFunctionExpression(right) {
			if _, twice := bodies[key]; twice {
				return nil
			}
			if _, declaredKey := declared[key]; declaredKey {
				return nil
			}
			bodies[key] = append(bodies[key], right)
			installations[right] = key
			installing[left] = struct{}{}
		}
	}
	// One walk over the whole class body.
	var accesses []thisAccess
	exact := true
	var visit func(node *ast.Node, functions int)
	visit = func(node *ast.Node, functions int) {
		if node == nil || !exact {
			return
		}
		if node != class && (ast.IsClassDeclaration(node) || ast.IsClassExpression(node)) {
			exact = false
			return
		}
		if nodeKindName(node) == "ThisKeyword" {
			parent := node.Parent
			switch {
			case parent != nil && ast.IsPropertyAccessExpression(parent) && parent.Expression() == node:
			case parent != nil && nodeKindName(parent) == "ElementAccessExpression" &&
				parent.Expression() == node && exactElementAccessKey(parent) != nil:
			default:
				exact = false
				return
			}
		}
		if ast.IsPropertyAccessExpression(node) || nodeKindName(node) == "ElementAccessExpression" {
			if object := node.Expression(); object != nil && nodeKindName(object) == "ThisKeyword" {
				key, ok := thisAccessKey(node)
				if !ok {
					exact = false
					return
				}
				accesses = append(accesses, thisAccess{node: node, key: key})
			}
		}
		if ast.IsReturnStatement(node) && functions == 0 && node.Expression() != nil {
			// `return value` in the constructor's own frame: `new` may hand the
			// caller that value instead of the instance.
			exact = false
			return
		}
		nested := functions
		if node != constructor && ast.IsFunctionLikeDeclaration(node) {
			nested++
		}
		node.ForEachChild(func(child *ast.Node) bool {
			visit(child, nested)
			return !exact
		})
	}
	for _, member := range members {
		if member == constructor {
			visit(member, 0)
		} else if member != nil {
			visit(member, 1)
		}
	}
	if !exact {
		return nil
	}
	// Every write of a member of `this` stores a value no hidden code can hang
	// on the instance.
	parameters := constructor.Parameters()
	parameterSymbols := make(map[*ast.Symbol]int, len(parameters))
	for index, parameter := range parameters {
		if parameter == nil || parameter.Name() == nil || !ast.IsIdentifier(parameter.Name()) {
			continue
		}
		if symbol := p.canonicalSymbol(p.checker.GetSymbolAtLocation(parameter.Name())); symbol != nil {
			parameterSymbols[symbol] = index
		}
	}
	for _, access := range accesses {
		target := ast.GetAssignmentTarget(access.node)
		if target == nil {
			continue
		}
		if access.key == "__proto__" {
			return nil
		}
		if !ast.IsBinaryExpression(target) || identityPreservingUnwrap(target.AsBinaryExpression().Left) != access.node {
			// A destructuring element, a `for … of` head, an update: a value
			// this census cannot name is stored. `this.count++` stores a
			// number, but no reviewed shape needs it.
			return nil
		}
		right := identityPreservingUnwrap(target.AsBinaryExpression().Right)
		if ast.IsArrowFunction(right) || ast.IsFunctionExpression(right) {
			if _, installed := installations[right]; !installed {
				return nil
			}
			continue
		}
		if !p.storesNoHiddenMemberLocked(right, parameterSymbols) {
			return nil
		}
	}
	// Every reference to the class in the program publishes it or constructs
	// it.
	if !p.classReferencesAreExactLocked(binding) {
		return nil
	}
	reached := constructionReachedRegions(constructor, bodies, installations, installing, accesses)
	var retained []typefacts.RetainedArgument
	for index, parameter := range parameters {
		argument, ok := p.retainedArgumentLocked(
			constructor, statements, parameter, index, declared, bodies, accesses, reached,
		)
		if ok {
			retained = append(retained, argument)
		}
	}
	return retained
}

// retainedArgumentLocked states one parameter of an exact class's
// constructor, or nothing.
func (p *project) retainedArgumentLocked(
	constructor *ast.Node,
	statements []*ast.Node,
	parameter *ast.Node,
	index int,
	declared map[string]struct{},
	bodies map[string][]*ast.Node,
	accesses []thisAccess,
	reached []*ast.Node,
) (typefacts.RetainedArgument, bool) {
	var none typefacts.RetainedArgument
	if parameter == nil || parameter.Name() == nil || !ast.IsIdentifier(parameter.Name()) ||
		parameter.Initializer() != nil || parameter.AsParameterDeclaration().DotDotDotToken != nil {
		return none, false
	}
	symbol := p.canonicalSymbol(p.checker.GetSymbolAtLocation(parameter.Name()))
	if symbol == nil {
		return none, false
	}
	// The retaining statement: a top-level `this.<key> = p` of the constructor.
	var store *ast.Node
	var storeTarget *ast.Node
	key := ""
	for _, statement := range statements {
		statementKey, left, right := thisStoreStatement(statement)
		if statementKey == "" || right == nil || !ast.IsIdentifier(right) {
			continue
		}
		if p.canonicalSymbol(p.checker.GetSymbolAtLocation(right)) != symbol {
			continue
		}
		if store != nil {
			return none, false
		}
		store, storeTarget, key = right, left, statementKey
	}
	if store == nil || key == "__proto__" {
		return none, false
	}
	if _, conflict := declared[key]; conflict {
		return none, false
	}
	if _, conflict := bodies[key]; conflict {
		return none, false
	}
	// Every use of the parameter: the store, or a direct call in the
	// constructor's own frame.
	qualified := true
	var uses func(node *ast.Node, functions int)
	uses = func(node *ast.Node, functions int) {
		if node == nil || !qualified {
			return
		}
		if ast.IsIdentifier(node) && node != parameter.Name() &&
			!ast.IsDeclarationNameOrImportPropertyName(node) &&
			p.canonicalSymbol(p.checker.GetSymbolAtLocation(node)) == symbol {
			parent := node.Parent
			direct := functions == 0 && parent != nil && ast.IsCallExpression(parent) &&
				parent.Expression() == node
			if node != store && !direct {
				qualified = false
				return
			}
		}
		nested := functions
		if ast.IsFunctionLikeDeclaration(node) {
			nested++
		}
		node.ForEachChild(func(child *ast.Node) bool {
			uses(child, nested)
			return !qualified
		})
	}
	constructor.Body().ForEachChild(func(child *ast.Node) bool {
		uses(child, 0)
		return !qualified
	})
	if !qualified {
		return none, false
	}
	// Every other access of the key is a member call nothing at construction
	// reaches.
	var invocations []typefacts.Location
	for _, access := range accesses {
		if access.key != key || access.node == storeTarget {
			continue
		}
		parent := access.node.Parent
		if parent == nil || !ast.IsCallExpression(parent) || identityPreservingUnwrap(parent.Expression()) != access.node {
			return none, false
		}
		for _, region := range reached {
			if nodeContains(region, access.node) {
				return none, false
			}
		}
		invocations = append(invocations, nodeLocation(parent))
	}
	return typefacts.RetainedArgument{
		ParameterIndex: index,
		Key:            key,
		Store:          nodeLocation(store),
		Invocations:    invocations,
	}, true
}

// storesNoHiddenMemberLocked is whether a value stored on the instance cannot
// be a function this census does not see: a constructor parameter (kept, and
// censused as one), or a value that is never a function.
func (p *project) storesNoHiddenMemberLocked(value *ast.Node, parameters map[*ast.Symbol]int) bool {
	if value == nil {
		return false
	}
	switch nodeKindName(value) {
	case "StringLiteral", "NumericLiteral", "BigIntLiteral", "NoSubstitutionTemplateLiteral",
		"TemplateExpression", "TrueKeyword", "FalseKeyword", "NullKeyword",
		"RegularExpressionLiteral", "ObjectLiteralExpression", "ArrayLiteralExpression",
		"NewExpression", "VoidExpression", "TypeOfExpression":
		return true
	case "PrefixUnaryExpression":
		return value.AsPrefixUnaryExpression().Operator == ast.KindExclamationToken
	}
	if ast.IsIdentifier(value) {
		_, parameter := parameters[p.canonicalSymbol(p.checker.GetSymbolAtLocation(value))]
		return parameter
	}
	return false
}

// classRuntimeBindingLocked is the symbol of the binding a module-level class
// value is reached through, or nil.
func (p *project) classRuntimeBindingLocked(class *ast.Node) *ast.Symbol {
	if ast.IsClassDeclaration(class) {
		if name := class.Name(); name != nil {
			return p.canonicalSymbol(p.checker.GetSymbolAtLocation(name))
		}
		return nil
	}
	declarator := class.Parent
	if declarator == nil || !ast.IsVariableDeclaration(declarator) ||
		identityPreservingUnwrap(declarator.Initializer()) != class ||
		declarator.Name() == nil || !ast.IsIdentifier(declarator.Name()) {
		return nil
	}
	return p.canonicalSymbol(p.checker.GetSymbolAtLocation(declarator.Name()))
}

// classReferencesAreExactLocked is whether every reference to the class value
// anywhere in the program publishes it (an export specifier), constructs it
// (the callee of a `new`), or reads and discards it (`void C`, the shape of the
// verifier's own harness): nothing extends it, augments its prototype, or
// hands it on.
func (p *project) classReferencesAreExactLocked(binding *ast.Symbol) bool {
	canonical := p.canonicalSymbol(binding)
	if canonical == nil {
		return false
	}
	id := p.idFor(canonical)
	p.referenceIndex.ensure(p)
	p.referenceIndex.markUsed(id)
	for _, location := range p.referenceIndex.locations(id) {
		sourceFile := p.program.GetSourceFile(location.Path)
		if sourceFile == nil {
			return false
		}
		node := deepestNodeAt(sourceFile.AsNode(), location.StartByte)
		if node == nil || !ast.IsIdentifier(node) {
			return false
		}
		at := nodeLocation(node)
		if at.StartByte != location.StartByte || at.EndByte != location.EndByte {
			return false
		}
		parent := node.Parent
		switch {
		case parent != nil && nodeKindName(parent) == "ExportSpecifier":
		case parent != nil && ast.IsNewExpression(parent) && identityPreservingUnwrap(parent.Expression()) == node:
		case parent != nil && nodeKindName(parent) == "VoidExpression":
		default:
			return false
		}
	}
	return true
}

// constructionReachedRegions answers the code a construction can run: the
// constructor body less the members it installs, and, to a fixed point, the
// body of every member whose key reached code names on `this`. Keys are matched
// by name, which can only reach more than runs.
func constructionReachedRegions(
	constructor *ast.Node,
	bodies map[string][]*ast.Node,
	installations map[*ast.Node]string,
	installing map[*ast.Node]struct{},
	accesses []thisAccess,
) []*ast.Node {
	regions := []*ast.Node{constructor}
	reachedKeys := make(map[string]struct{})
	for index := 0; index < len(regions); index++ {
		region := regions[index]
		for _, access := range accesses {
			if !nodeContains(region, access.node) {
				continue
			}
			if _, installs := installing[access.node]; installs {
				continue
			}
			if region == constructor {
				inInstalled := false
				for installed := range installations {
					if nodeContains(installed, access.node) {
						inInstalled = true
						break
					}
				}
				if inInstalled {
					continue
				}
			}
			if _, seen := reachedKeys[access.key]; seen {
				continue
			}
			reachedKeys[access.key] = struct{}{}
			regions = append(regions, bodies[access.key]...)
		}
	}
	// The constructor region excludes its installed members; report it as the
	// set of regions whose every position runs at construction, so a member
	// call inside an installed closure is reached only through its key.
	result := make([]*ast.Node, 0, len(regions))
	for _, region := range regions {
		if region == constructor {
			result = append(result, constructorOwnRegions(constructor, installations)...)
			continue
		}
		result = append(result, region)
	}
	return result
}

// constructorOwnRegions splits the constructor body into the statements that
// are not installations, and the non-function parts of those.
func constructorOwnRegions(constructor *ast.Node, installations map[*ast.Node]string) []*ast.Node {
	var regions []*ast.Node
	for _, statement := range constructor.Body().AsBlock().Statements.Nodes {
		_, _, right := thisStoreStatement(statement)
		if right != nil {
			if _, installed := installations[right]; installed {
				continue
			}
		}
		regions = append(regions, statement)
	}
	for _, parameter := range constructor.Parameters() {
		regions = append(regions, parameter)
	}
	return regions
}

// thisAccess is one member access of `this` in a class body and the key it
// names.
type thisAccess struct {
	node *ast.Node
	key  string
}

// thisStoreStatement reads a statement as `this.<key> = <right>` and answers
// the key, the target access and the unwrapped right-hand side, or the empty
// key.
func thisStoreStatement(statement *ast.Node) (string, *ast.Node, *ast.Node) {
	if statement == nil || nodeKindName(statement) != "ExpressionStatement" {
		return "", nil, nil
	}
	expression := identityPreservingUnwrap(statement.Expression())
	if expression == nil || !ast.IsBinaryExpression(expression) {
		return "", nil, nil
	}
	assignment := expression.AsBinaryExpression()
	if assignment.OperatorToken.Kind != ast.KindEqualsToken {
		return "", nil, nil
	}
	left := identityPreservingUnwrap(assignment.Left)
	if left == nil || !(ast.IsPropertyAccessExpression(left) || nodeKindName(left) == "ElementAccessExpression") {
		return "", nil, nil
	}
	if object := left.Expression(); object == nil || nodeKindName(object) != "ThisKeyword" {
		return "", nil, nil
	}
	key, ok := thisAccessKey(left)
	if !ok {
		return "", nil, nil
	}
	return key, left, identityPreservingUnwrap(assignment.Right)
}

// thisAccessKey is the property key a member access names exactly: a name
// (`#name` for a private one), or a string or numeric literal element key.
func thisAccessKey(access *ast.Node) (string, bool) {
	if ast.IsPropertyAccessExpression(access) {
		name := access.Name()
		if name == nil {
			return "", false
		}
		return name.Text(), true
	}
	key := exactElementAccessKey(access)
	if key == nil {
		return "", false
	}
	return key.Text(), true
}

// classMemberKey is the key a class element declares, when it names one
// exactly.
func classMemberKey(name *ast.Node) (string, bool) {
	if name == nil {
		return "", false
	}
	switch nodeKindName(name) {
	case "Identifier", "PrivateIdentifier", "StringLiteral", "NumericLiteral":
		return name.Text(), true
	}
	return "", false
}

func hasStaticKeyword(node *ast.Node) bool {
	modifiers := node.Modifiers()
	if modifiers == nil {
		return false
	}
	for _, modifier := range modifiers.Nodes {
		if modifier != nil && nodeKindName(modifier) == "StaticKeyword" {
			return true
		}
	}
	return false
}

func nodeContains(outer, inner *ast.Node) bool {
	if outer == nil || inner == nil {
		return false
	}
	return outer.Pos() <= inner.Pos() && inner.End() <= outer.End()
}
