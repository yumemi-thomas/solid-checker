package tsgo

import (
	"sort"

	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts"
)

// ADR 0153 item C: a run-time accessor installation bound to the exports that
// can operate on what it installs on.
//
// The closure's `runtime-accessor-installation` hazard withdraws `reads` from
// every export of a case, because a read through an installed accessor leaves
// no form behind. This census states, per installation site, the one shape in
// which that is too strong: the target is an allocation its installing
// function makes, and every use of the value anywhere in the package is one
// this census classifies. Then an export that cannot execute any operation on
// the target cannot read through what was installed on it.
//
// What the census admits as a use of the target, and nothing else:
//
//   - the target argument of an installation on it (the only use a nested
//     callable may make);
//   - a `return` of a named, synchronous function, whose call sites are then
//     uses in their turn, to a fixed point;
//   - an unwritten local binding initialized by it, whose references are uses;
//   - a plain prop of a dialect `createComponent` whose component is a local
//     named function reading its props only member by member;
//   - an expression statement, a condition, `typeof`, `void`, or a strict
//     equality, none of which runs code;
//   - an operation: a member read or write, a call, a construction, a spread,
//     an iteration, `in`, `instanceof`, a destructuring, an `await`, a
//     coercion. Each is recorded in Accesses with the named function it sits
//     in.
//
// Anything else -- the value stored in an object, an array or a binding that
// is written, passed to any other call, thrown, yielded, returned from an
// async function or a callable with no name, operated on inside an anonymous
// callable, or reaching module scope -- makes the site unbounded, and the
// consumer keeps the case-wide hazard.
//
// Readers closes Accesses backward: the named functions the accesses sit in,
// and every named function that calls one, where each is referenced only as a
// callee (or the component argument of `createComponent`), an export, or an
// import. A reader referenced any other way, or called at module scope or from
// an anonymous callable, makes the site unbounded too. An implementation that
// is not a reader cannot run an operation on the target, because nothing but
// a call by name reaches a reader. A caller-supplied callable, and a
// dependency that imports this package back, are the caller's and the
// dependency's, as everywhere else in this model.
//
// Everything is decided over the accepted program's runtime sources of the
// implementation's own installed package, and memoized per package.

// accessorInstallationsLocked is the census attached to one implementation,
// or nil when this producer has no opinion.
func (p *project) accessorInstallationsLocked(implementation *ast.Node) *typefacts.AccessorInstallationCensus {
	if p.formTwin != nil || implementation == nil {
		return nil
	}
	// A file in no installed package answers "": the program's own sources,
	// scanned as one package, as the context census does.
	packageDirectory := contextPackageDirectory(nodeLocation(implementation).Path)
	scan := p.accessorPackageScanLocked(packageDirectory)
	census := &typefacts.AccessorInstallationCensus{}
	for _, site := range scan.sites {
		stated := site.stated
		if site.stated.Kind == typefacts.AccessorInstallationFreshTarget {
			_, stated.Reached = site.readers[implementation]
		}
		census.Sites = append(census.Sites, stated)
	}
	return census
}

type accessorPackageScan struct {
	sites []accessorSiteAnalysis
}

type accessorSiteAnalysis struct {
	stated  typefacts.AccessorInstallationSite
	readers map[*ast.Node]struct{}
}

func (p *project) accessorPackageScanLocked(packageDirectory string) accessorPackageScan {
	if p.accessorInstallationProgram != p.program {
		p.accessorInstallationProgram = p.program
		p.accessorInstallationScans = make(map[string]accessorPackageScan)
	}
	if scan, computed := p.accessorInstallationScans[packageDirectory]; computed {
		return scan
	}
	files := p.accessorPackageFilesLocked(packageDirectory)
	index := p.accessorReferenceIndexLocked(files)
	var scan accessorPackageScan
	for _, sourceFile := range files {
		var visit func(*ast.Node)
		visit = func(node *ast.Node) {
			if node == nil {
				return
			}
			if site, ok := p.accessorSiteLocked(node); ok {
				analysis := accessorSiteAnalysis{stated: site.stated}
				if site.stated.Kind == typefacts.AccessorInstallationFreshTarget {
					analysis = p.accessorFlowLocked(site, index)
				}
				scan.sites = append(scan.sites, analysis)
			}
			node.ForEachChild(func(child *ast.Node) bool { visit(child); return false })
		}
		visit(sourceFile.AsNode())
	}
	p.accessorInstallationScans[packageDirectory] = scan
	return scan
}

func (p *project) accessorPackageFilesLocked(packageDirectory string) []*ast.SourceFile {
	var files []*ast.SourceFile
	for _, sourceFile := range p.program.SourceFiles() {
		if sourceFile.IsDeclarationFile || p.program.IsSourceFileDefaultLibrary(sourceFile.Path()) ||
			contextPackageDirectory(sourceFile.FileName()) != packageDirectory {
			continue
		}
		files = append(files, sourceFile)
	}
	return files
}

// accessorReferenceIndex maps each canonical symbol to every identifier in the
// package's runtime sources that names it, a shorthand property's value
// included. Declaration names are left out.
type accessorReferenceIndex map[*ast.Symbol][]*ast.Node

func (p *project) accessorReferenceIndexLocked(files []*ast.SourceFile) accessorReferenceIndex {
	index := make(accessorReferenceIndex)
	for _, sourceFile := range files {
		var visit func(*ast.Node)
		visit = func(node *ast.Node) {
			if node == nil {
				return
			}
			if ast.IsIdentifier(node) && !ast.IsPartOfTypeNode(node) {
				parent := node.Parent
				if parent != nil && ast.IsShorthandPropertyAssignment(parent) {
					if value := p.canonicalSymbol(checker.Checker_GetShorthandAssignmentValueSymbol(p.checker, parent)); value != nil {
						index[value] = append(index[value], node)
					}
				} else if !ast.IsDeclarationNameOrImportPropertyName(node) {
					if symbol := p.canonicalSymbol(p.checker.GetSymbolAtLocation(node)); symbol != nil {
						index[symbol] = append(index[symbol], node)
					}
				}
			}
			node.ForEachChild(func(child *ast.Node) bool { visit(child); return false })
		}
		visit(sourceFile.AsNode())
	}
	return index
}

// accessorSite is one detected installation, with what the flow analysis
// needs: the initial occurrences of the target's value, and the binding that
// holds it when there is one.
type accessorSite struct {
	stated      typefacts.AccessorInstallationSite
	function    *ast.Node
	occurrences []*ast.Node
	binding     *ast.Symbol
}

// accessorSiteLocked places a node the closure's hazard detectors name. The
// spellings are theirs (`solid_facts::ast`'s `RuntimeAccessorInstallation`):
// the global `Proxy`, a static member named `defineProperty`,
// `defineProperties`, `setPrototypeOf`, `__defineGetter__`, `__defineSetter__`
// or `__proto__`, a computed member of the global `Object` or `Reflect`, a
// `__proto__:` literal key, and `Object.create` with two arguments.
func (p *project) accessorSiteLocked(node *ast.Node) (accessorSite, bool) {
	unbounded := func(at *ast.Node, reason string) (accessorSite, bool) {
		return accessorSite{stated: typefacts.AccessorInstallationSite{
			Site: nodeLocation(at), Kind: typefacts.AccessorInstallationUnbounded, Refusal: reason,
		}}, true
	}
	switch {
	case ast.IsIdentifier(node) && node.Text() == "Proxy":
		parent := node.Parent
		if parent == nil || ast.IsDeclarationNameOrImportPropertyName(node) ||
			(ast.IsPropertyAccessExpression(parent) && parent.Name() == node) ||
			nodeKindName(parent) == "PropertyAssignment" || nodeKindName(parent) == "BindingElement" ||
			ast.IsPartOfTypeNode(node) || !p.accessorGlobalLocked(node) {
			return accessorSite{}, false
		}
		if !ast.IsNewExpression(parent) || parent.Expression() != node {
			return unbounded(node, "the Proxy constructor is used other than as the callee of `new`")
		}
		return p.accessorFreshSiteLocked(parent, parent, nil)
	case ast.IsPropertyAccessExpression(node):
		name := node.Name()
		if name == nil || !ast.IsIdentifier(name) {
			return accessorSite{}, false
		}
		switch name.Text() {
		case "defineProperty", "defineProperties", "setPrototypeOf", "__defineGetter__", "__defineSetter__":
		case "__proto__":
			return unbounded(node, "a `__proto__` member access can change a prototype")
		default:
			return accessorSite{}, false
		}
		call := node.Parent
		if call == nil || !ast.IsCallExpression(call) || identityPreservingUnwrap(call.Expression()) != node ||
			call.QuestionDotToken() != nil {
			return unbounded(node, "the installing member is used other than as a callee")
		}
		arguments := call.AsCallExpression().Arguments
		if name.Text() == "__defineGetter__" || name.Text() == "__defineSetter__" {
			return p.accessorFreshSiteLocked(call, identityPreservingUnwrap(node.Expression()), nil)
		}
		receiver := identityPreservingUnwrap(node.Expression())
		if receiver == nil || !ast.IsIdentifier(receiver) || !p.accessorGlobalLocked(receiver) ||
			(receiver.Text() != "Object" && receiver.Text() != "Reflect") ||
			(receiver.Text() == "Reflect" && name.Text() == "defineProperties") {
			return unbounded(call, "the installing member is not the intrinsic Object's or Reflect's")
		}
		if arguments == nil || len(arguments.Nodes) == 0 || nodeKindName(arguments.Nodes[0]) == "SpreadElement" {
			return unbounded(call, "the installation names no target")
		}
		// `Object.defineProperty` hands its target back; `Reflect`'s answers a
		// boolean.
		var result *ast.Node
		if receiver.Text() == "Object" {
			result = call
		}
		return p.accessorFreshSiteLocked(call, identityPreservingUnwrap(arguments.Nodes[0]), result)
	case nodeKindName(node) == "ElementAccessExpression":
		receiver := identityPreservingUnwrap(node.Expression())
		if receiver != nil && ast.IsIdentifier(receiver) && (receiver.Text() == "Object" || receiver.Text() == "Reflect") &&
			p.accessorGlobalLocked(receiver) {
			return unbounded(node, "a computed member of an intrinsic namespace can spell any installation")
		}
	case nodeKindName(node) == "PropertyAssignment":
		name := node.Name()
		if name == nil || nodeKindName(name) == "ComputedPropertyName" || contextPropertyNameText(name) != "__proto__" {
			return accessorSite{}, false
		}
		value := identityPreservingUnwrap(objectLiteralPropertyValue(node))
		if value != nil && nodeKindName(value) == "NullKeyword" {
			return accessorSite{stated: typefacts.AccessorInstallationSite{
				Site: nodeLocation(node), Kind: typefacts.AccessorInstallationNullPrototype,
			}}, true
		}
		return unbounded(node, "a `__proto__:` literal key gives the object a prototype that may carry an accessor")
	case ast.IsCallExpression(node):
		callee := identityPreservingUnwrap(node.Expression())
		if callee == nil || !ast.IsPropertyAccessExpression(callee) || callee.Name() == nil ||
			callee.Name().Text() != "create" {
			return accessorSite{}, false
		}
		receiver := identityPreservingUnwrap(callee.Expression())
		arguments := node.AsCallExpression().Arguments
		if receiver == nil || !ast.IsIdentifier(receiver) || receiver.Text() != "Object" || !p.accessorGlobalLocked(receiver) ||
			arguments == nil || len(arguments.Nodes) < 2 {
			return accessorSite{}, false
		}
		return p.accessorFreshSiteLocked(node, node, nil)
	}
	return accessorSite{}, false
}

// accessorGlobalLocked is whether an identifier names the intrinsic of its
// spelling: nothing in the program's own sources declares it.
func (p *project) accessorGlobalLocked(identifier *ast.Node) bool {
	symbol := p.checker.GetSymbolAtLocation(identifier)
	if symbol == nil {
		return true
	}
	for _, declaration := range symbol.Declarations {
		file := ast.GetSourceFileOfNode(declaration)
		if file == nil || !p.program.IsSourceFileDefaultLibrary(file.Path()) {
			return false
		}
	}
	return true
}

// accessorFreshSiteLocked decides whether an installation's target is an
// allocation its function makes. `target` is the target expression (or, for
// `new Proxy` and `Object.create`, the allocating expression itself);
// `result` is the installation's own value when it is the target.
func (p *project) accessorFreshSiteLocked(site *ast.Node, target *ast.Node, result *ast.Node) (accessorSite, bool) {
	stated := typefacts.AccessorInstallationSite{Site: nodeLocation(site), Kind: typefacts.AccessorInstallationUnbounded}
	fail := func(reason string) (accessorSite, bool) {
		stated.Refusal = reason
		return accessorSite{stated: stated}, true
	}
	if target == nil {
		return fail("the installation names no target")
	}
	var allocation, holder *ast.Node
	var binding *ast.Symbol
	occurrences := []*ast.Node{}
	switch {
	case target == site:
		// `new Proxy(…)`, `Object.create(…)`: the site allocates.
		allocation = site
		occurrences = append(occurrences, site)
	case p.accessorFreshExpressionLocked(target):
		allocation = target
		if result == nil {
			return fail("the installation's fresh target is discarded by an installation that does not return it")
		}
	case ast.IsIdentifier(target):
		declaration := p.singleUnwrittenLocalInitializerLocked(target)
		if declaration == nil || !p.accessorFreshExpressionLocked(identityPreservingUnwrap(declaration.Initializer())) {
			return fail("the installation's target is not an allocation of the installing function")
		}
		binding = p.canonicalSymbol(p.checker.GetSymbolAtLocation(declaration.Name()))
		if binding == nil {
			return fail("the installation's target binding has no symbol")
		}
		allocation = declaration
		holder = declaration
	default:
		return fail("the installation's target is not an allocation of the installing function")
	}
	if result != nil {
		occurrences = append(occurrences, result)
	}
	scope := allocation
	if holder != nil {
		scope = holder
	}
	function := accessorEnclosingFunction(scope)
	if function == nil {
		return fail("the installation's target is allocated at module scope")
	}
	location := nodeLocation(function)
	allocated := nodeLocation(allocation)
	stated.Kind = typefacts.AccessorInstallationFreshTarget
	stated.Function = &location
	stated.Target = &allocated
	return accessorSite{stated: stated, function: function, occurrences: occurrences, binding: binding}, true
}

// accessorFreshExpressionLocked is whether evaluating an expression allocates
// a new object nothing else holds: an object or array literal, `new Proxy`,
// `Object.create`, a conditional between two such, or an `Object`
// installation over one (which hands its target back).
func (p *project) accessorFreshExpressionLocked(expression *ast.Node) bool {
	expression = identityPreservingUnwrap(expression)
	if expression == nil {
		return false
	}
	switch {
	case ast.IsObjectLiteralExpression(expression), nodeKindName(expression) == "ArrayLiteralExpression":
		return true
	case ast.IsConditionalExpression(expression):
		conditional := expression.AsConditionalExpression()
		return p.accessorFreshExpressionLocked(conditional.WhenTrue) && p.accessorFreshExpressionLocked(conditional.WhenFalse)
	case ast.IsNewExpression(expression):
		callee := identityPreservingUnwrap(expression.Expression())
		return callee != nil && ast.IsIdentifier(callee) && callee.Text() == "Proxy" && p.accessorGlobalLocked(callee)
	case ast.IsCallExpression(expression):
		callee := identityPreservingUnwrap(expression.Expression())
		if callee == nil || !ast.IsPropertyAccessExpression(callee) || callee.Name() == nil {
			return false
		}
		receiver := identityPreservingUnwrap(callee.Expression())
		if receiver == nil || !ast.IsIdentifier(receiver) || receiver.Text() != "Object" || !p.accessorGlobalLocked(receiver) {
			return false
		}
		arguments := expression.AsCallExpression().Arguments
		switch callee.Name().Text() {
		case "create":
			return true
		case "defineProperty", "defineProperties", "setPrototypeOf":
			return arguments != nil && len(arguments.Nodes) != 0 && p.accessorFreshExpressionLocked(arguments.Nodes[0])
		}
	}
	return false
}

// accessorEnclosingFunction is the innermost function-like node or class
// member body that evaluates `node`, or nil at module scope. A class field
// initializer and a static block evaluate outside any function this census
// names, so they answer themselves and are never named.
func accessorEnclosingFunction(node *ast.Node) *ast.Node {
	for current := node.Parent; current != nil; current = current.Parent {
		switch nodeKindName(current) {
		case "FunctionDeclaration", "FunctionExpression", "ArrowFunction", "MethodDeclaration",
			"GetAccessor", "SetAccessor", "Constructor", "PropertyDeclaration", "ClassStaticBlockDeclaration":
			return current
		case "SourceFile":
			return nil
		}
	}
	return nil
}

// accessorNamedFunctionLocked is the binding a callable is reached through by
// name, or nil: a named function declaration, or a function or arrow
// expression initializing an unwritten binding declared once.
func (p *project) accessorNamedFunctionLocked(function *ast.Node) *ast.Symbol {
	if function == nil {
		return nil
	}
	switch nodeKindName(function) {
	case "FunctionDeclaration":
		name := function.Name()
		if name == nil {
			return nil
		}
		return p.canonicalSymbol(p.checker.GetSymbolAtLocation(name))
	case "FunctionExpression", "ArrowFunction":
		declarator := function.Parent
		for declarator != nil && ast.IsParenthesizedExpression(declarator) {
			declarator = declarator.Parent
		}
		if declarator == nil || !ast.IsVariableDeclaration(declarator) ||
			identityPreservingUnwrap(declarator.Initializer()) != function ||
			declarator.Name() == nil || !ast.IsIdentifier(declarator.Name()) {
			return nil
		}
		symbol := p.canonicalSymbol(p.checker.GetSymbolAtLocation(declarator.Name()))
		if symbol == nil || len(symbol.Declarations) != 1 || p.symbolIsAssignedLocked(symbol, declarator) {
			return nil
		}
		return symbol
	}
	return nil
}

// accessorFlow is one fresh target's flow in progress.
type accessorFlow struct {
	project   *project
	index     accessorReferenceIndex
	site      accessorSite
	queued    map[*ast.Node]struct{}
	work      []*ast.Node
	returns   map[*ast.Node]struct{}
	aliases   map[*ast.Symbol]struct{}
	props     map[*ast.Node]map[string]struct{}
	accesses  []typefacts.AccessorTargetAccess
	readers   map[*ast.Node]struct{}
	refusal   string
	functions map[*ast.Node]*ast.Symbol
}

func (p *project) accessorFlowLocked(site accessorSite, index accessorReferenceIndex) accessorSiteAnalysis {
	flow := &accessorFlow{
		project:   p,
		index:     index,
		site:      site,
		queued:    make(map[*ast.Node]struct{}),
		returns:   make(map[*ast.Node]struct{}),
		aliases:   make(map[*ast.Symbol]struct{}),
		props:     make(map[*ast.Node]map[string]struct{}),
		readers:   make(map[*ast.Node]struct{}),
		functions: make(map[*ast.Node]*ast.Symbol),
	}
	for _, occurrence := range site.occurrences {
		flow.push(occurrence)
	}
	if site.binding != nil {
		flow.alias(site.binding)
	}
	for len(flow.work) != 0 && flow.refusal == "" {
		occurrence := flow.work[len(flow.work)-1]
		flow.work = flow.work[:len(flow.work)-1]
		flow.classify(occurrence)
	}
	if flow.refusal == "" {
		flow.closeReaders()
	}
	stated := site.stated
	if flow.refusal != "" {
		stated.Kind = typefacts.AccessorInstallationUnbounded
		stated.Function = nil
		stated.Target = nil
		stated.Refusal = flow.refusal
		return accessorSiteAnalysis{stated: stated}
	}
	for function := range flow.returns {
		stated.Returns = append(stated.Returns, nodeLocation(function))
	}
	for function := range flow.readers {
		stated.Readers = append(stated.Readers, nodeLocation(function))
	}
	sortLocations(stated.Returns)
	sortLocations(stated.Readers)
	sort.SliceStable(flow.accesses, func(i, j int) bool {
		return locationLess(flow.accesses[i].Location, flow.accesses[j].Location)
	})
	stated.Accesses = flow.accesses
	return accessorSiteAnalysis{stated: stated, readers: flow.readers}
}

func sortLocations(locations []typefacts.Location) {
	sort.SliceStable(locations, func(i, j int) bool { return locationLess(locations[i], locations[j]) })
}

func locationLess(a, b typefacts.Location) bool {
	if a.Path != b.Path {
		return a.Path < b.Path
	}
	if a.StartByte != b.StartByte {
		return a.StartByte < b.StartByte
	}
	return a.EndByte < b.EndByte
}

func (flow *accessorFlow) refuse(reason string) {
	if flow.refusal == "" {
		flow.refusal = reason
	}
}

func (flow *accessorFlow) push(node *ast.Node) {
	if node == nil {
		return
	}
	if _, seen := flow.queued[node]; seen {
		return
	}
	flow.queued[node] = struct{}{}
	flow.work = append(flow.work, node)
}

// alias makes every reference of an unwritten binding an occurrence.
func (flow *accessorFlow) alias(symbol *ast.Symbol) {
	if _, seen := flow.aliases[symbol]; seen {
		return
	}
	flow.aliases[symbol] = struct{}{}
	for _, reference := range flow.index[symbol] {
		flow.push(reference)
	}
}

// named is the binding of a named function, memoized, or nil.
func (flow *accessorFlow) named(function *ast.Node) *ast.Symbol {
	if symbol, computed := flow.functions[function]; computed {
		return symbol
	}
	symbol := flow.project.accessorNamedFunctionLocked(function)
	flow.functions[function] = symbol
	return symbol
}

// access records one operation on the target inside the named function that
// performs it.
func (flow *accessorFlow) access(at *ast.Node, occurrence *ast.Node, kind string) {
	function := accessorEnclosingFunction(occurrence)
	if function == nil {
		flow.refuse("the target is operated on at module scope")
		return
	}
	if flow.named(function) == nil {
		flow.refuse("the target is operated on inside a callable with no name")
		return
	}
	flow.accesses = append(flow.accesses, typefacts.AccessorTargetAccess{
		Location: nodeLocation(at), Kind: kind, Function: nodeLocation(function),
	})
	flow.readers[function] = struct{}{}
}

// installationTarget is whether an occurrence is the target argument of an
// installation on the target, which is the one use a nested callable may make.
// Its value, when the installation hands it back, is an occurrence in turn.
func (flow *accessorFlow) installationTarget(occurrence *ast.Node, parent *ast.Node) bool {
	if parent == nil {
		return false
	}
	if ast.IsPropertyAccessExpression(parent) && identityPreservingUnwrap(parent.Expression()) == occurrence {
		name := parent.Name()
		call := parent.Parent
		if name != nil && (name.Text() == "__defineGetter__" || name.Text() == "__defineSetter__") &&
			call != nil && ast.IsCallExpression(call) && identityPreservingUnwrap(call.Expression()) == parent {
			return true
		}
		return false
	}
	if !ast.IsCallExpression(parent) {
		return false
	}
	arguments := parent.AsCallExpression().Arguments
	if arguments == nil || len(arguments.Nodes) == 0 || identityPreservingUnwrap(arguments.Nodes[0]) != occurrence {
		return false
	}
	callee := identityPreservingUnwrap(parent.Expression())
	if callee == nil || !ast.IsPropertyAccessExpression(callee) || callee.Name() == nil {
		return false
	}
	receiver := identityPreservingUnwrap(callee.Expression())
	if receiver == nil || !ast.IsIdentifier(receiver) || !flow.project.accessorGlobalLocked(receiver) {
		return false
	}
	switch {
	case receiver.Text() == "Object" && (callee.Name().Text() == "defineProperty" ||
		callee.Name().Text() == "defineProperties" || callee.Name().Text() == "setPrototypeOf"):
		flow.push(parent)
		return true
	case receiver.Text() == "Reflect" && (callee.Name().Text() == "defineProperty" ||
		callee.Name().Text() == "setPrototypeOf"):
		return true
	}
	return false
}

// classify decides one occurrence of the target's value by the position it
// sits in.
func (flow *accessorFlow) classify(occurrence *ast.Node) {
	node := occurrence
	parent := node.Parent
	for parent != nil && (ast.IsParenthesizedExpression(parent) || ast.IsAsExpression(parent) ||
		ast.IsSatisfiesExpression(parent) || ast.IsNonNullExpression(parent)) {
		node = parent
		parent = node.Parent
	}
	if parent == nil {
		flow.refuse("the target reaches a position with no parent")
		return
	}
	if flow.installationTarget(occurrence, parent) {
		return
	}
	function := accessorEnclosingFunction(node)
	if function == nil {
		flow.refuse("the target reaches module scope")
		return
	}
	switch kind := nodeKindName(parent); kind {
	case "ExpressionStatement", "VoidExpression", "TypeOfExpression", "IfStatement", "WhileStatement",
		"DoStatement", "SwitchStatement", "CaseClause":
		return
	case "DeleteExpression":
		return
	case "ReturnStatement":
		flow.returned(function)
		return
	case "ArrowFunction":
		if parent.Body() == node {
			flow.returned(parent)
			return
		}
	case "ForStatement":
		// A `for` head's condition or update: ToBoolean or a discarded value.
		return
	case "VariableDeclaration":
		if parent.Initializer() == node {
			name := parent.Name()
			if name != nil && ast.IsIdentifier(name) {
				symbol := flow.project.canonicalSymbol(flow.project.checker.GetSymbolAtLocation(name))
				if symbol == nil || len(symbol.Declarations) != 1 || flow.project.symbolIsAssignedLocked(symbol, parent) {
					flow.refuse("the target is held by a binding that is written")
					return
				}
				flow.alias(symbol)
				return
			}
			flow.access(parent, node, "destructure")
			return
		}
	case "PropertyAccessExpression", "ElementAccessExpression":
		if parent.Expression() == node {
			flow.access(parent, node, "member")
			return
		}
		// An element key is converted to a property key.
		flow.access(parent, node, "coercion")
		return
	case "CallExpression", "NewExpression":
		if parent.Expression() == node {
			flow.access(parent, node, "call")
			return
		}
		flow.refuse("the target is passed to a call")
		return
	case "TaggedTemplateExpression":
		flow.access(parent, node, "call")
		return
	case "SpreadElement", "SpreadAssignment":
		flow.access(parent, node, "spread")
		return
	case "ForOfStatement", "ForInStatement":
		if parent.Expression() == node {
			flow.access(parent, node, "iteration")
			return
		}
	case "TemplateSpan":
		flow.access(parent, node, "coercion")
		return
	case "AwaitExpression":
		flow.access(parent, node, "await")
		return
	case "PrefixUnaryExpression", "PostfixUnaryExpression":
		if kind == "PrefixUnaryExpression" && parent.AsPrefixUnaryExpression().Operator == ast.KindExclamationToken {
			return
		}
		flow.access(parent, node, "coercion")
		return
	case "ConditionalExpression":
		conditional := parent.AsConditionalExpression()
		if conditional.Condition == node {
			return
		}
		flow.push(parent)
		return
	case "BinaryExpression":
		binary := parent.AsBinaryExpression()
		if binary.OperatorToken == nil {
			break
		}
		switch nodeKindName(binary.OperatorToken) {
		case "EqualsEqualsEqualsToken", "ExclamationEqualsEqualsToken":
			return
		case "AmpersandAmpersandToken", "BarBarToken", "QuestionQuestionToken":
			flow.push(parent)
			return
		case "CommaToken":
			if binary.Right == node {
				flow.push(parent)
			}
			return
		case "InKeyword":
			if binary.Right == node {
				flow.access(parent, node, "in")
			} else {
				flow.access(parent, node, "coercion")
			}
			return
		case "InstanceOfKeyword":
			flow.access(parent, node, "instanceof")
			return
		case "EqualsToken":
			flow.refuse("the target is assigned")
			return
		}
		if _, coerces := coercingBinaryOperators[nodeKindName(binary.OperatorToken)]; coerces {
			flow.access(parent, node, "coercion")
			return
		}
	case "PropertyAssignment", "ShorthandPropertyAssignment":
		flow.prop(parent)
		return
	}
	flow.refuse("the target reaches a " + nodeKindName(parent) + " position this census does not classify")
}

// returned makes a function's call sites occurrences: its completion may be
// the target.
func (flow *accessorFlow) returned(function *ast.Node) {
	if _, seen := flow.returns[function]; seen {
		return
	}
	symbol := flow.named(function)
	if symbol == nil || !plainSynchronousCallable(function) {
		flow.refuse("the target is returned from a callable that is not a named, synchronous function")
		return
	}
	flow.returns[function] = struct{}{}
	for _, reference := range flow.index[symbol] {
		site, ok := flow.referenceSite(reference)
		if !ok {
			flow.refuse("a function returning the target escapes as a value")
			return
		}
		if site != nil {
			flow.push(site)
		}
	}
}

// referenceSite classifies one reference to a named function this census
// follows: the call it is the callee of (or the `createComponent` it is the
// component of), nil for an export or import, and false for anything else.
func (flow *accessorFlow) referenceSite(reference *ast.Node) (*ast.Node, bool) {
	node := reference
	parent := node.Parent
	if parent != nil && ast.IsPropertyAccessExpression(parent) && parent.Name() == node {
		// `namespace.fn`.
		node = parent
		parent = node.Parent
	}
	for parent != nil && ast.IsParenthesizedExpression(parent) {
		node = parent
		parent = node.Parent
	}
	if parent == nil {
		return nil, false
	}
	switch nodeKindName(parent) {
	case "ExportSpecifier", "ExportAssignment", "ImportSpecifier", "ImportClause":
		return nil, true
	case "CallExpression":
		if parent.Expression() == node {
			return parent, true
		}
		arguments := parent.AsCallExpression().Arguments
		if arguments != nil && len(arguments.Nodes) >= 2 && arguments.Nodes[0] == node {
			if name, _ := flow.project.contextDialectCallLocked(parent); name == contextRenderName {
				return parent, true
			}
		}
	}
	return nil, false
}

// prop follows the target into a local component's props, or refuses.
func (flow *accessorFlow) prop(property *ast.Node) {
	literal := property.Parent
	call := (*ast.Node)(nil)
	if literal != nil && ast.IsObjectLiteralExpression(literal) {
		call = literal.Parent
		for call != nil && ast.IsParenthesizedExpression(call) {
			call = call.Parent
		}
	}
	if call == nil || !ast.IsCallExpression(call) {
		flow.refuse("the target is stored in an object")
		return
	}
	arguments := call.AsCallExpression().Arguments
	if arguments == nil || len(arguments.Nodes) != 2 || identityPreservingUnwrap(arguments.Nodes[1]) != literal {
		flow.refuse("the target is stored in an object")
		return
	}
	if name, _ := flow.project.contextDialectCallLocked(call); name != contextRenderName {
		flow.refuse("the target is stored in an object")
		return
	}
	component := identityPreservingUnwrap(arguments.Nodes[0])
	var declaration *ast.Node
	if component != nil && ast.IsIdentifier(component) {
		symbol := flow.project.canonicalSymbol(flow.project.checker.GetSymbolAtLocation(component))
		if symbol != nil && len(symbol.Declarations) == 1 {
			candidate := symbol.Declarations[0]
			switch {
			case ast.IsFunctionDeclaration(candidate):
				declaration = candidate
			case ast.IsVariableDeclaration(candidate):
				initializer := identityPreservingUnwrap(candidate.Initializer())
				if initializer != nil && (ast.IsArrowFunction(initializer) || ast.IsFunctionExpression(initializer)) {
					declaration = initializer
				}
			}
		}
	}
	name := property.Name()
	if declaration == nil || flow.named(declaration) == nil || name == nil ||
		nodeKindName(name) == "ComputedPropertyName" || contextPropertyNameText(name) == "" ||
		mentionsArgumentsOrEval(declaration) {
		flow.refuse("the target is a prop of a component this census does not follow")
		return
	}
	file := ast.GetSourceFileOfNode(declaration)
	if file == nil || contextPackageDirectory(file.FileName()) != contextPackageDirectory(nodeLocation(flow.site.function).Path) {
		flow.refuse("the target is a prop of a component outside its package")
		return
	}
	key := contextPropertyNameText(name)
	if keys := flow.props[declaration]; keys != nil {
		if _, seen := keys[key]; seen {
			return
		}
	} else {
		flow.props[declaration] = make(map[string]struct{})
	}
	flow.props[declaration][key] = struct{}{}
	parameters := declaration.Parameters()
	if len(parameters) == 0 {
		return
	}
	parameter := parameters[0].AsParameterDeclaration()
	bound := parameters[0].Name()
	if parameter == nil || bound == nil || !ast.IsIdentifier(bound) || parameter.DotDotDotToken != nil || parameter.Initializer != nil {
		flow.refuse("a component receiving the target does not name its props plainly")
		return
	}
	symbol := flow.project.canonicalSymbol(flow.project.checker.GetSymbolAtLocation(bound))
	if symbol == nil || flow.project.symbolIsAssignedLocked(symbol, declaration) {
		flow.refuse("a component receiving the target writes its props binding")
		return
	}
	for _, reference := range flow.index[symbol] {
		access := reference.Parent
		if access == nil || !ast.IsPropertyAccessExpression(access) || access.Expression() != reference ||
			access.Name() == nil || !ast.IsIdentifier(access.Name()) {
			flow.refuse("a component receiving the target reads its props other than member by member")
			return
		}
		if access.Name().Text() == key {
			flow.push(access)
		}
	}
}

// closeReaders closes Readers backward over the call sites of each reader.
func (flow *accessorFlow) closeReaders() {
	work := make([]*ast.Node, 0, len(flow.readers))
	for reader := range flow.readers {
		work = append(work, reader)
	}
	for len(work) != 0 && flow.refusal == "" {
		reader := work[len(work)-1]
		work = work[:len(work)-1]
		symbol := flow.named(reader)
		if symbol == nil {
			flow.refuse("a function that operates on the target has no name")
			return
		}
		for _, reference := range flow.index[symbol] {
			site, ok := flow.referenceSite(reference)
			if !ok {
				flow.refuse("a function that operates on the target escapes as a value")
				return
			}
			if site == nil {
				continue
			}
			caller := accessorEnclosingFunction(site)
			if caller == nil {
				flow.refuse("a function that operates on the target runs at module scope")
				return
			}
			if flow.named(caller) == nil {
				flow.refuse("a function that operates on the target is called from a callable with no name")
				return
			}
			if _, seen := flow.readers[caller]; !seen {
				flow.readers[caller] = struct{}{}
				work = append(work, caller)
			}
		}
	}
}
