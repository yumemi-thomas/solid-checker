package tsgo

import (
	"path/filepath"
	"strings"

	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts"
)

// ADR 0153: a member of a package-owned context value.
//
// Everything here states positions and never decides a disposition. The
// consumer binds every chain call to a row its own census walked, asks the
// audited dialect whether each named call is its `createContext`,
// `useContext` or `createComponent`, and refuses a context that escapes. So
// the names below are only how this side chooses which calls to describe: a
// local function spelled `useContext` resolves to a runtime-source
// declaration, is not a declaration-file binding, and is never described as a
// read.

const (
	contextCreateName = "createContext"
	contextReadName   = "useContext"
	contextRenderName = "createComponent"
	contextChainDepth = 8
)

// accessorInstallationNames are the members that can turn an existing data
// property into an accessor. Matched by spelling on purpose: an occurrence
// this scan cannot place in a checked call over-refuses, which is the safe
// direction.
var accessorInstallationNames = map[string]bool{
	"defineProperty":   true,
	"defineProperties": true,
	"__defineGetter__": true,
	"__defineSetter__": true,
}

// A `delete` of the member is harmless on its own: the read then reaches the
// literal's prototype, `Object.prototype`, whose one accessor is the engine's
// `__proto__` (and an accessor installed *there* under the member's key is an
// installation the scan above refuses). It matters only beside a change of
// prototype, so the scan refuses every spelling that can change an existing
// object's prototype instead: `setPrototypeOf` anywhere, and a write to
// `__proto__`.
const prototypeMutationName = "setPrototypeOf"

// contextMemberPremiseLocked states ADR 0153's premise for one accessor form,
// or nil. Only the accepted program is asked: under a premise twin the
// checker is a different program with different symbols, and this census
// reads every runtime file of the accepted one.
func (p *project) contextMemberPremiseLocked(node *ast.Node, subject *ast.Node) *typefacts.ContextMemberPremise {
	if p.formTwin != nil || node == nil || subject == nil {
		return nil
	}
	member, ok := contextMemberKey(node)
	if !ok {
		return nil
	}
	chain, read := p.contextValueChainLocked(subject)
	if read == nil {
		return nil
	}
	arguments := read.AsCallExpression().Arguments
	if arguments == nil || len(arguments.Nodes) != 1 {
		return nil
	}
	argument := identityPreservingUnwrap(arguments.Nodes[0])
	if argument == nil || !ast.IsIdentifier(argument) {
		return nil
	}
	contextSymbol := p.canonicalSymbol(p.checker.GetSymbolAtLocation(argument))
	provision := p.contextProvisionLocked(contextSymbol)
	if provision == nil {
		return nil
	}
	readLocation := nodeLocation(read)
	found := false
	for _, candidate := range provision.census.Reads {
		if candidate.Call == readLocation {
			found = true
			break
		}
	}
	if !found {
		return nil
	}
	premise := &typefacts.ContextMemberPremise{
		Member: member,
		Chain:  chain,
		Read:   readLocation,
		Context: typefacts.ContextProvision{
			Declaration: provision.census.Declaration,
			Initializer: provision.census.Initializer,
			Reads:       provision.census.Reads,
			Helpers:     provision.census.Helpers,
			Exports:     provision.census.Exports,
		},
	}
	for _, provider := range provision.providers {
		stated := provider.stated
		stated.Literals = nil
		for _, literal := range provider.literals {
			location, ok := contextLiteralMember(literal, member)
			if !ok {
				return nil
			}
			stated.Literals = append(stated.Literals, typefacts.ContextLiteral{
				Literal: nodeLocation(literal), Member: location,
			})
		}
		premise.Context.Providers = append(premise.Context.Providers, stated)
	}
	installations, ok := p.contextInstallationsLocked(contextPackageDirectory(premise.Context.Declaration.Path))
	if !ok {
		return nil
	}
	for _, installation := range installations {
		for _, key := range installation.Keys {
			if key == member {
				return nil
			}
		}
	}
	premise.Installations = installations
	return premise
}

// contextMemberKey is the member a read-position property access or
// literal-keyed element access names. A write, a compound or update, and a
// `delete` state nothing.
func contextMemberKey(node *ast.Node) (string, bool) {
	if ast.GetAssignmentTarget(node) != nil {
		return "", false
	}
	if parent := node.Parent; parent != nil && nodeKindName(parent) == "DeleteExpression" {
		return "", false
	}
	switch {
	case ast.IsPropertyAccessExpression(node):
		name := node.Name()
		if name == nil || !ast.IsIdentifier(name) {
			return "", false
		}
		return name.Text(), true
	case nodeKindName(node) == "ElementAccessExpression":
		key := exactElementAccessKey(node)
		if key == nil || !(ast.IsStringLiteral(key) || ast.IsNoSubstitutionTemplateLiteral(key)) {
			return "", false
		}
		return key.Text(), true
	}
	return "", false
}

// contextValueChainLocked reduces a subject to the dialect read it is the
// value of, through unwritten local aliases, `result` steps and `identity`
// steps. Nil read when any link is something else.
func (p *project) contextValueChainLocked(subject *ast.Node) ([]typefacts.ContextChainStep, *ast.Node) {
	var chain []typefacts.ContextChainStep
	seen := make(map[*ast.Node]bool)
	node := identityPreservingUnwrap(subject)
	for depth := 0; node != nil && depth < contextChainDepth; depth++ {
		if seen[node] {
			return nil, nil
		}
		seen[node] = true
		switch {
		case ast.IsIdentifier(node):
			declaration := p.singleUnwrittenLocalInitializerLocked(node)
			if declaration == nil {
				return nil, nil
			}
			node = identityPreservingUnwrap(declaration.Initializer())
			continue
		case ast.IsCallExpression(node):
			if node.QuestionDotToken() != nil {
				return nil, nil
			}
			if name, _ := p.contextDialectCallLocked(node); name == contextReadName {
				return chain, node
			}
			callee := p.formRuntimeCalleeDeclarationLocked(node)
			if callee == nil || !plainSynchronousCallable(callee) {
				return nil, nil
			}
			if returns, ok := p.identityHelperReturnsLocked(callee); ok {
				arguments := node.AsCallExpression().Arguments
				if arguments == nil || len(arguments.Nodes) == 0 || nodeKindName(arguments.Nodes[0]) == "SpreadElement" {
					return nil, nil
				}
				chain = append(chain, typefacts.ContextChainStep{
					Kind: typefacts.ContextChainIdentity, Call: nodeLocation(node),
					Callee: nodeLocation(callee), Returns: returns,
				})
				node = identityPreservingUnwrap(arguments.Nodes[0])
				continue
			}
			expression, returns := singleCompletionExpression(callee)
			if expression == nil {
				return nil, nil
			}
			chain = append(chain, typefacts.ContextChainStep{
				Kind: typefacts.ContextChainResult, Call: nodeLocation(node),
				Callee: nodeLocation(callee), Returns: returns,
			})
			node = identityPreservingUnwrap(expression)
			continue
		}
		return nil, nil
	}
	return nil, nil
}

// plainSynchronousCallable excludes the completion wrappers: an `async`
// function hands back a promise and a generator an iterator, whatever its
// `return` names, and `arguments` or `eval` can rebind a parameter unseen.
func plainSynchronousCallable(callee *ast.Node) bool {
	if ast.HasSyntacticModifier(callee, ast.ModifierFlagsAsync) || mentionsArgumentsOrEval(callee) {
		return false
	}
	if ast.IsFunctionDeclaration(callee) && callee.AsFunctionDeclaration().AsteriskToken != nil {
		return false
	}
	if ast.IsFunctionExpression(callee) && callee.AsFunctionExpression().AsteriskToken != nil {
		return false
	}
	return true
}

// completionReturns is every `return` of a callable's own body, outside
// nested callables, or nil when a return carries no expression. The bool is
// false when a normal completion can fall off the end: the last statement is
// not a `return`.
func completionReturns(callee *ast.Node) ([]*ast.Node, bool) {
	body := callee.Body()
	if body == nil {
		return nil, false
	}
	if !ast.IsBlock(body) {
		return nil, false
	}
	statements := body.AsBlock().Statements
	if statements == nil {
		return nil, false
	}
	// The last statement that runs must be a `return`. A function declaration
	// after it is hoisted, not executed, so `return {…}; function helper() {}`
	// ends in its return.
	last := len(statements.Nodes) - 1
	for last >= 0 && ast.IsFunctionDeclaration(statements.Nodes[last]) {
		last--
	}
	if last < 0 || !ast.IsReturnStatement(statements.Nodes[last]) {
		return nil, false
	}
	var returns []*ast.Node
	valid := true
	var walk func(*ast.Node)
	walk = func(node *ast.Node) {
		if !valid || node == nil || node != body && literalResultBoundary(node) {
			return
		}
		if ast.IsReturnStatement(node) {
			if node.Expression() == nil {
				valid = false
				return
			}
			returns = append(returns, node)
			return
		}
		node.ForEachChild(func(child *ast.Node) bool { walk(child); return false })
	}
	walk(body)
	return returns, valid && len(returns) != 0
}

// singleCompletionExpression is the one expression every normal completion of
// a callable hands back: an arrow's expression body, or the one `return` of a
// block body that ends in it.
func singleCompletionExpression(callee *ast.Node) (*ast.Node, []typefacts.Location) {
	body := callee.Body()
	if body == nil {
		return nil, nil
	}
	if !ast.IsBlock(body) {
		return body, []typefacts.Location{nodeLocation(body)}
	}
	returns, ok := completionReturns(callee)
	if !ok || len(returns) != 1 {
		return nil, nil
	}
	return returns[0].Expression(), []typefacts.Location{nodeLocation(returns[0])}
}

// identityHelperReturnsLocked answers the returns of a callable whose every
// completion hands back its unwritten, plain first parameter (`invariant`).
func (p *project) identityHelperReturnsLocked(callee *ast.Node) ([]typefacts.Location, bool) {
	parameters := callee.Parameters()
	if len(parameters) == 0 {
		return nil, false
	}
	first := parameters[0]
	declaration := first.AsParameterDeclaration()
	name := first.Name()
	if declaration == nil || name == nil || !ast.IsIdentifier(name) || declaration.DotDotDotToken != nil || declaration.Initializer != nil {
		return nil, false
	}
	symbol := p.checker.GetSymbolAtLocation(name)
	if symbol == nil || p.symbolIsAssignedLocked(symbol, callee) {
		return nil, false
	}
	var expressions []*ast.Node
	var locations []typefacts.Location
	body := callee.Body()
	if body == nil {
		return nil, false
	}
	if !ast.IsBlock(body) {
		expressions = []*ast.Node{body}
		locations = []typefacts.Location{nodeLocation(body)}
	} else {
		returns, ok := completionReturns(callee)
		if !ok {
			return nil, false
		}
		for _, statement := range returns {
			expressions = append(expressions, statement.Expression())
			locations = append(locations, nodeLocation(statement))
		}
	}
	for _, expression := range expressions {
		returned := identityPreservingUnwrap(expression)
		if returned == nil || !ast.IsIdentifier(returned) || p.checker.GetSymbolAtLocation(returned) != symbol {
			return nil, false
		}
	}
	return locations, true
}

// contextDialectCallLocked names the declaration-file binding a call's plain
// identifier callee resolves to, or "" for anything else.
func (p *project) contextDialectCallLocked(call *ast.Node) (string, *typefacts.ContextDialectCall) {
	if call == nil || !ast.IsCallExpression(call) {
		return "", nil
	}
	callee := identityPreservingUnwrap(call.Expression())
	if callee == nil || !ast.IsIdentifier(callee) {
		return "", nil
	}
	_, name, _, declaration := p.implementationCallTargetLocked(callee)
	if declaration == nil || name == "" {
		return "", nil
	}
	symbol := p.canonicalSymbol(p.checker.GetSymbolAtLocation(callee))
	if symbol == nil || len(symbol.Declarations) == 0 {
		return "", nil
	}
	for _, candidate := range symbol.Declarations {
		file := ast.GetSourceFileOfNode(candidate)
		if file == nil || !file.IsDeclarationFile {
			return "", nil
		}
	}
	return name, &typefacts.ContextDialectCall{
		Call: nodeLocation(call), TargetName: name, Declaration: *declaration,
	}
}

// contextProvisionResult is one context binding's census, memoized per
// symbol. A nil result means some reference fitted no class.
type contextProvisionResult struct {
	census    typefacts.ContextProvision
	providers []contextProvisionProvider
}

type contextProvisionProvider struct {
	stated   typefacts.ContextProvider
	literals []*ast.Node
}

func (p *project) contextProvisionLocked(symbol *ast.Symbol) *contextProvisionResult {
	if symbol == nil {
		return nil
	}
	if cached, computed := p.contextProvisions[symbol]; computed {
		return cached
	}
	result := p.computeContextProvisionLocked(symbol)
	if p.contextProvisions == nil {
		p.contextProvisions = make(map[*ast.Symbol]*contextProvisionResult)
	}
	p.contextProvisions[symbol] = result
	return result
}

func (p *project) computeContextProvisionLocked(symbol *ast.Symbol) *contextProvisionResult {
	if len(symbol.Declarations) != 1 || symbol.Declarations[0] == nil {
		return nil
	}
	declaration := symbol.Declarations[0]
	if !ast.IsVariableDeclaration(declaration) || declaration.Name() == nil || !ast.IsIdentifier(declaration.Name()) {
		return nil
	}
	file := ast.GetSourceFileOfNode(declaration)
	list := declaration.Parent
	if file == nil || file.IsDeclarationFile || !p.isCurrentSourceFile(file) || list == nil ||
		list.Flags&ast.NodeFlagsConst == 0 || list.Parent == nil || !ast.IsVariableStatement(list.Parent) ||
		list.Parent.Parent != file.AsNode() {
		return nil
	}
	initializer := identityPreservingUnwrap(declaration.Initializer())
	if initializer == nil || !ast.IsCallExpression(initializer) || initializer.QuestionDotToken() != nil {
		return nil
	}
	if arguments := initializer.AsCallExpression().Arguments; arguments != nil && len(arguments.Nodes) != 0 {
		return nil
	}
	name, create := p.contextDialectCallLocked(initializer)
	if name != contextCreateName {
		return nil
	}
	result := &contextProvisionResult{census: typefacts.ContextProvision{
		Declaration: nodeLocation(declaration),
		Initializer: *create,
	}}
	if ast.HasSyntacticModifier(list.Parent, ast.ModifierFlagsExport) {
		result.census.Exports = append(result.census.Exports, typefacts.ContextExport{
			Location: nodeLocation(list.Parent), Name: declaration.Name().Text(),
		})
	}
	reads := make(map[typefacts.Location]bool)
	addRead := func(call *typefacts.ContextDialectCall) {
		if !reads[call.Call] {
			reads[call.Call] = true
			result.census.Reads = append(result.census.Reads, *call)
		}
	}
	valid := true
	exported := make(map[*ast.Node]bool)
	packageDirectory := contextPackageDirectory(file.FileName())
	for _, sourceFile := range p.program.SourceFiles() {
		if !valid {
			break
		}
		if sourceFile.IsDeclarationFile || p.program.IsSourceFileDefaultLibrary(sourceFile.Path()) ||
			contextPackageDirectory(sourceFile.FileName()) != packageDirectory {
			continue
		}
		var visit func(*ast.Node)
		visit = func(node *ast.Node) {
			if !valid || node == nil {
				return
			}
			if ast.IsIdentifier(node) && node != declaration.Name() {
				if parent := node.Parent; parent != nil && ast.IsShorthandPropertyAssignment(parent) {
					// `{ C }` stores the binding's value in an object. The
					// checker answers the literal's *property* symbol for
					// that name, so the value symbol is asked separately;
					// either one naming the context refuses.
					value := checker.Checker_GetShorthandAssignmentValueSymbol(p.checker, parent)
					if p.canonicalSymbol(value) == symbol || p.canonicalSymbol(p.checker.GetSymbolAtLocation(node)) == symbol {
						valid = false
						return
					}
				} else if p.canonicalSymbol(p.checker.GetSymbolAtLocation(node)) == symbol {
					if !p.classifyContextReferenceLocked(node, symbol, result, addRead, exported) {
						valid = false
						return
					}
				}
			}
			node.ForEachChild(func(child *ast.Node) bool { visit(child); return !valid })
		}
		visit(sourceFile.AsNode())
	}
	if !valid {
		return nil
	}
	return result
}

// classifyContextReferenceLocked places one reference to the context binding
// in its class, or answers false.
func (p *project) classifyContextReferenceLocked(
	reference *ast.Node,
	symbol *ast.Symbol,
	result *contextProvisionResult,
	addRead func(*typefacts.ContextDialectCall),
	exported map[*ast.Node]bool,
) bool {
	parent := reference.Parent
	if parent == nil {
		return false
	}
	switch nodeKindName(parent) {
	case "ImportSpecifier", "ImportClause":
		// An alias: its own references resolve to the same symbol and are
		// classified where they occur.
		return true
	case "ExportSpecifier":
		if exported[parent] {
			return true
		}
		exported[parent] = true
		specifier := parent.AsExportSpecifier()
		name := specifier.Name()
		if name == nil {
			return false
		}
		result.census.Exports = append(result.census.Exports, typefacts.ContextExport{
			Location: nodeLocation(parent), Name: name.Text(),
		})
		return true
	case "ExportAssignment":
		result.census.Exports = append(result.census.Exports, typefacts.ContextExport{
			Location: nodeLocation(parent), Name: "default",
		})
		return true
	}
	call := parent
	if !ast.IsCallExpression(call) || call.QuestionDotToken() != nil {
		return false
	}
	arguments := call.AsCallExpression().Arguments
	if arguments == nil {
		return false
	}
	position := -1
	for index, argument := range arguments.Nodes {
		if argument == reference {
			position = index
		}
	}
	if position < 0 {
		// The reference is the callee: `C(props)` runs the provider directly.
		return false
	}
	name, dialect := p.contextDialectCallLocked(call)
	switch name {
	case contextReadName:
		if position != 0 || len(arguments.Nodes) != 1 {
			return false
		}
		addRead(dialect)
		return true
	case contextRenderName:
		if position != 0 || len(arguments.Nodes) < 2 {
			return false
		}
		provider, ok := p.contextProviderLocked(arguments.Nodes[1])
		if !ok {
			return false
		}
		provider.stated.Render = *dialect
		result.providers = append(result.providers, provider)
		result.census.Providers = append(result.census.Providers, provider.stated)
		return true
	case "":
		// A local read helper: `useOptionalContext(C)`.
		helper := p.formRuntimeCalleeDeclarationLocked(call)
		if helper == nil || !plainSynchronousCallable(helper) {
			return false
		}
		helperReads, ok := p.contextReadHelperLocked(helper, position)
		if !ok {
			return false
		}
		for _, read := range helperReads {
			addRead(read)
		}
		result.census.Helpers = append(result.census.Helpers, typefacts.ContextReadHelper{
			Call: nodeLocation(call), Callee: nodeLocation(helper), Argument: position,
		})
		return true
	}
	return false
}

// contextReadHelperLocked answers the dialect reads a local helper performs
// with its parameter at `position`, when that parameter is a plain unwritten
// binding and every one of its references is the one argument of a dialect
// `useContext` call.
func (p *project) contextReadHelperLocked(helper *ast.Node, position int) ([]*typefacts.ContextDialectCall, bool) {
	parameters := helper.Parameters()
	if position >= len(parameters) {
		return nil, false
	}
	parameter := parameters[position]
	declaration := parameter.AsParameterDeclaration()
	name := parameter.Name()
	if declaration == nil || name == nil || !ast.IsIdentifier(name) || declaration.DotDotDotToken != nil || declaration.Initializer != nil {
		return nil, false
	}
	symbol := p.checker.GetSymbolAtLocation(name)
	if symbol == nil || p.symbolIsAssignedLocked(symbol, helper) {
		return nil, false
	}
	var reads []*typefacts.ContextDialectCall
	valid := true
	var visit func(*ast.Node)
	visit = func(node *ast.Node) {
		if !valid || node == nil {
			return
		}
		if ast.IsIdentifier(node) && node != name && p.checker.GetSymbolAtLocation(node) == symbol {
			call := node.Parent
			if call == nil || !ast.IsCallExpression(call) || call.QuestionDotToken() != nil {
				valid = false
				return
			}
			arguments := call.AsCallExpression().Arguments
			if arguments == nil || len(arguments.Nodes) != 1 || arguments.Nodes[0] != node {
				valid = false
				return
			}
			spelled, dialect := p.contextDialectCallLocked(call)
			if spelled != contextReadName {
				valid = false
				return
			}
			reads = append(reads, dialect)
		}
		node.ForEachChild(func(child *ast.Node) bool { visit(child); return !valid })
	}
	visit(helper.Body())
	return reads, valid && len(reads) != 0
}

// contextProviderLocked reads a `createComponent(C, props)` second argument:
// an object literal with exactly one plain `value` member, no spread and no
// computed key, whose expression is an object literal or an unwritten local
// bound to a local factory whose every completion is an object literal.
func (p *project) contextProviderLocked(props *ast.Node) (contextProvisionProvider, bool) {
	props = identityPreservingUnwrap(props)
	if props == nil || !ast.IsObjectLiteralExpression(props) {
		return contextProvisionProvider{}, false
	}
	var value *ast.Node
	for _, property := range props.AsObjectLiteralExpression().Properties.Nodes {
		kind := nodeKindName(property)
		if kind == "SpreadAssignment" {
			return contextProvisionProvider{}, false
		}
		propertyName := property.Name()
		if propertyName == nil || nodeKindName(propertyName) == "ComputedPropertyName" {
			return contextProvisionProvider{}, false
		}
		if contextPropertyNameText(propertyName) != "value" {
			continue
		}
		if kind != "PropertyAssignment" || value != nil {
			return contextProvisionProvider{}, false
		}
		value = objectLiteralPropertyValue(property)
	}
	if value == nil {
		return contextProvisionProvider{}, false
	}
	provider := contextProvisionProvider{stated: typefacts.ContextProvider{Value: nodeLocation(value)}}
	expression := identityPreservingUnwrap(value)
	if expression != nil && ast.IsObjectLiteralExpression(expression) {
		provider.literals = []*ast.Node{expression}
		return provider, true
	}
	if expression == nil || !ast.IsIdentifier(expression) {
		return contextProvisionProvider{}, false
	}
	binding := p.singleUnwrittenLocalInitializerLocked(expression)
	if binding == nil {
		return contextProvisionProvider{}, false
	}
	initializer := identityPreservingUnwrap(binding.Initializer())
	if initializer != nil && ast.IsObjectLiteralExpression(initializer) {
		provider.literals = []*ast.Node{initializer}
		return provider, true
	}
	if initializer == nil || !ast.IsCallExpression(initializer) || initializer.QuestionDotToken() != nil {
		return contextProvisionProvider{}, false
	}
	factory := p.formRuntimeCalleeDeclarationLocked(initializer)
	if factory == nil || !plainSynchronousCallable(factory) {
		return contextProvisionProvider{}, false
	}
	var completions []*ast.Node
	if body := factory.Body(); body != nil && !ast.IsBlock(body) {
		completions = []*ast.Node{body}
	} else {
		returns, ok := completionReturns(factory)
		if !ok {
			return contextProvisionProvider{}, false
		}
		for _, statement := range returns {
			completions = append(completions, statement.Expression())
		}
	}
	for _, completion := range completions {
		literal := identityPreservingUnwrap(completion)
		if literal == nil || !ast.IsObjectLiteralExpression(literal) {
			return contextProvisionProvider{}, false
		}
		provider.literals = append(provider.literals, literal)
	}
	provider.stated.Factory = &typefacts.ContextFactory{
		Call: nodeLocation(initializer), Callee: nodeLocation(factory),
	}
	return provider, true
}

func contextPropertyNameText(name *ast.Node) string {
	switch {
	case ast.IsIdentifier(name), ast.IsStringLiteral(name), ast.IsNoSubstitutionTemplateLiteral(name):
		return name.Text()
	}
	return ""
}

// contextLiteralMember answers the location of the one non-accessor member a
// literal declares under `member`, when no accessor shares the name and no
// accessor has a computed key.
func contextLiteralMember(literal *ast.Node, member string) (typefacts.Location, bool) {
	var found *ast.Node
	for _, property := range literal.AsObjectLiteralExpression().Properties.Nodes {
		kind := nodeKindName(property)
		if kind == "SpreadAssignment" {
			continue
		}
		name := property.Name()
		if name == nil {
			return typefacts.Location{}, false
		}
		// A `__proto__:` member sets the literal's prototype rather than a
		// property, which the premise's `delete` argument reasons about.
		if kind == "PropertyAssignment" && contextPropertyNameText(name) == "__proto__" {
			return typefacts.Location{}, false
		}
		accessor := kind == "GetAccessor" || kind == "SetAccessor"
		if nodeKindName(name) == "ComputedPropertyName" {
			if accessor {
				return typefacts.Location{}, false
			}
			continue
		}
		if contextPropertyNameText(name) != member {
			continue
		}
		switch {
		case accessor:
			return typefacts.Location{}, false
		case kind == "PropertyAssignment", kind == "ShorthandPropertyAssignment", kind == "MethodDeclaration":
			if found != nil {
				return typefacts.Location{}, false
			}
			found = property
		default:
			return typefacts.Location{}, false
		}
	}
	if found == nil {
		return typefacts.Location{}, false
	}
	return nodeLocation(found), true
}

// contextPackageDirectory is the installed package a file belongs to: the
// path up to and including the last `node_modules/<name>` (two segments for a
// scoped name), or "" for a file in no package. Both scans below are about the
// context's own package: its references, and the calls in it that could
// install an accessor. A dependency does not hold the provided object unless
// the package hands it over, and an accessor a dependency installs on an
// object it was handed is ADR 0044's stated hole, not this census's.
func contextPackageDirectory(path string) string {
	normalized := filepath.ToSlash(path)
	index := strings.LastIndex(normalized, "/node_modules/")
	if index < 0 {
		return ""
	}
	rest := normalized[index+len("/node_modules/"):]
	segments := strings.SplitN(rest, "/", 3)
	count := 1
	if strings.HasPrefix(segments[0], "@") {
		count = 2
	}
	if len(segments) < count {
		return normalized
	}
	return normalized[:index+len("/node_modules/")] + strings.Join(segments[:count], "/")
}

// contextInstallationsLocked scans every runtime source file of one package
// once for the calls that can make a data property an accessor, and for
// every spelling that can change an existing object's prototype. It answers
// false when an occurrence is anything but a checked call naming its key(s)
// by string literal: an aliased `defineProperty`, a computed key, a
// descriptor map that is not a plain literal, `setPrototypeOf`, a `__proto__`
// write.
func (p *project) contextInstallationsLocked(packageDirectory string) ([]typefacts.ContextInstallation, bool) {
	if p.contextInstallationProgram != p.program {
		p.contextInstallationProgram = p.program
		p.contextInstallationScans = make(map[string]contextInstallationScan)
	}
	if scan, computed := p.contextInstallationScans[packageDirectory]; computed {
		return scan.installations, scan.valid
	}
	var installations []typefacts.ContextInstallation
	valid := true
	for _, sourceFile := range p.program.SourceFiles() {
		if !valid {
			break
		}
		if sourceFile.IsDeclarationFile || p.program.IsSourceFileDefaultLibrary(sourceFile.Path()) ||
			contextPackageDirectory(sourceFile.FileName()) != packageDirectory {
			continue
		}
		var visit func(*ast.Node)
		visit = func(node *ast.Node) {
			if !valid || node == nil {
				return
			}
			switch {
			case ast.IsIdentifier(node) && accessorInstallationNames[node.Text()]:
				installation, ok := contextInstallationCall(node)
				if !ok {
					valid = false
					return
				}
				installations = append(installations, installation)
			case (ast.IsStringLiteral(node) || ast.IsNoSubstitutionTemplateLiteral(node)) &&
				(accessorInstallationNames[node.Text()] || node.Text() == prototypeMutationName):
				// `Object["defineProperty"]`: an installation this scan does
				// not place.
				valid = false
				return
			case ast.IsIdentifier(node) && node.Text() == prototypeMutationName:
				valid = false
				return
			case contextPrototypeWrite(node):
				valid = false
				return
			}
			node.ForEachChild(func(child *ast.Node) bool { visit(child); return !valid })
		}
		visit(sourceFile.AsNode())
	}
	if !valid {
		installations = nil
	}
	p.contextInstallationScans[packageDirectory] = contextInstallationScan{installations: installations, valid: valid}
	return installations, valid
}

type contextInstallationScan struct {
	installations []typefacts.ContextInstallation
	valid         bool
}

// contextInstallationCall places one spelling of an installation member: it
// must be the name of `X.defineProperty(target, "key", …)`-shaped call.
func contextInstallationCall(name *ast.Node) (typefacts.ContextInstallation, bool) {
	access := name.Parent
	if access == nil || !ast.IsPropertyAccessExpression(access) || access.Name() != name {
		return typefacts.ContextInstallation{}, false
	}
	call := access.Parent
	if call == nil || !ast.IsCallExpression(call) || identityPreservingUnwrap(call.Expression()) != access {
		return typefacts.ContextInstallation{}, false
	}
	arguments := call.AsCallExpression().Arguments
	if arguments == nil {
		return typefacts.ContextInstallation{}, false
	}
	literal := func(node *ast.Node) (string, bool) {
		node = identityPreservingUnwrap(node)
		if node != nil && (ast.IsStringLiteral(node) || ast.IsNoSubstitutionTemplateLiteral(node)) {
			return node.Text(), true
		}
		return "", false
	}
	switch name.Text() {
	case "defineProperty":
		if len(arguments.Nodes) < 2 {
			return typefacts.ContextInstallation{}, false
		}
		key, ok := literal(arguments.Nodes[1])
		if !ok {
			return typefacts.ContextInstallation{}, false
		}
		return typefacts.ContextInstallation{Location: nodeLocation(call), Keys: []string{key}}, true
	case "__defineGetter__", "__defineSetter__":
		if len(arguments.Nodes) < 1 {
			return typefacts.ContextInstallation{}, false
		}
		key, ok := literal(arguments.Nodes[0])
		if !ok {
			return typefacts.ContextInstallation{}, false
		}
		return typefacts.ContextInstallation{Location: nodeLocation(call), Keys: []string{key}}, true
	case "defineProperties":
		if len(arguments.Nodes) < 2 {
			return typefacts.ContextInstallation{}, false
		}
		descriptors := identityPreservingUnwrap(arguments.Nodes[1])
		if descriptors == nil || !ast.IsObjectLiteralExpression(descriptors) {
			return typefacts.ContextInstallation{}, false
		}
		var keys []string
		for _, property := range descriptors.AsObjectLiteralExpression().Properties.Nodes {
			propertyName := property.Name()
			if nodeKindName(property) == "SpreadAssignment" || propertyName == nil {
				return typefacts.ContextInstallation{}, false
			}
			key := contextPropertyNameText(propertyName)
			if key == "" {
				return typefacts.ContextInstallation{}, false
			}
			keys = append(keys, key)
		}
		return typefacts.ContextInstallation{Location: nodeLocation(call), Keys: keys}, true
	}
	return typefacts.ContextInstallation{}, false
}

// contextPrototypeWrite reports a write to `__proto__`: an assignment whose
// target is `x.__proto__` or `x["__proto__"]`.
func contextPrototypeWrite(node *ast.Node) bool {
	var key string
	switch {
	case ast.IsPropertyAccessExpression(node):
		name := node.Name()
		if name == nil || !ast.IsIdentifier(name) {
			return false
		}
		key = name.Text()
	case nodeKindName(node) == "ElementAccessExpression":
		argument := exactElementAccessKey(node)
		if argument == nil || !(ast.IsStringLiteral(argument) || ast.IsNoSubstitutionTemplateLiteral(argument)) {
			return false
		}
		key = argument.Text()
	default:
		return false
	}
	return key == "__proto__" && ast.GetAssignmentTarget(node) != nil
}
