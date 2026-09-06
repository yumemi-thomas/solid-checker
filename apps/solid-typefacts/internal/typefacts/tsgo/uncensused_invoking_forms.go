package tsgo

import (
	"strings"

	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts"
)

// nonInvokingNodeKinds is the reviewed list of *non-token* node kinds that
// provably cannot invoke user code by being evaluated. It is the whole of what
// keeps the classifier's default at refusal: a kind that is neither classified
// into a named UncensusedInvokingFormKind nor listed here is recorded as
// UncensusedUnclassifiedInvokingForm, so a kind a future compiler revision
// adds refuses on arrival instead of passing in silence.
//
// Membership is a claim, one kind at a time, that evaluating a node of this
// kind reaches no user code *of its own accord*. It says nothing about the
// node's children: they are walked, and each is classified on its own. So a
// CallExpression is listed — the call census records it — and an
// ObjectLiteralExpression is listed even though `{ x: a.x }` invokes a getter,
// because that getter is the PropertyAccessExpression child's row.
//
// Membership is also a claim about *every position* a node of the kind can
// occupy, which is why three kinds the compiler reinterprets in
// assignment-target position are absent: ArrayLiteralExpression,
// PropertyAssignment, and ShorthandPropertyAssignment are read as
// destructuring patterns there (checker.checkDestructuringAssignment) and do
// invoke, so classifyInvokingFormLocked's switch asks which position the node
// is in before answering. YieldExpression is absent for the neighbouring
// reason: `yield*` drives an iterator and a plain `yield` does not, so the
// switch decides on the asterisk.
//
// Names are the compiler's own, with the "Kind" prefix removed, which is what
// nodeKindName answers. Only *non-token* kinds belong here: a kind below
// ast.KindFirstNode is already cleared by classifyInvokingFormLocked's first
// step, so listing one would be dead weight that reads as a reviewed claim.
// There are 123 entries.
var nonInvokingNodeKinds = map[string]struct{}{
	// Names and type syntax. None of it is evaluated at runtime.
	"QualifiedName":               {},
	"TypeParameter":               {},
	"PropertySignature":           {},
	"MethodSignature":             {},
	"CallSignature":               {},
	"ConstructSignature":          {},
	"IndexSignature":              {},
	"TypePredicate":               {},
	"TypeReference":               {},
	"FunctionType":                {},
	"ConstructorType":             {},
	"TypeQuery":                   {},
	"TypeLiteral":                 {},
	"ArrayType":                   {},
	"TupleType":                   {},
	"OptionalType":                {},
	"RestType":                    {},
	"UnionType":                   {},
	"IntersectionType":            {},
	"ConditionalType":             {},
	"InferType":                   {},
	"ParenthesizedType":           {},
	"ThisType":                    {},
	"TypeOperator":                {},
	"IndexedAccessType":           {},
	"MappedType":                  {},
	"LiteralType":                 {},
	"NamedTupleMember":            {},
	"TemplateLiteralType":         {},
	"TemplateLiteralTypeSpan":     {},
	"ImportType":                  {},
	"ExpressionWithTypeArguments": {},

	// Declarations. Declaring a callable does not run it; its body is walked
	// as part of this implementation with Captured set, exactly as the call
	// census walks it. A get/set accessor *declaration* is listed for the same
	// reason a function declaration is: the invocation is at the access site,
	// which is where UncensusedGetAccessor is recorded.
	"Parameter":                   {},
	"PropertyDeclaration":         {},
	"MethodDeclaration":           {},
	"ClassStaticBlockDeclaration": {},
	"Constructor":                 {},
	"GetAccessor":                 {},
	"SetAccessor":                 {},
	"FunctionDeclaration":         {},
	"FunctionExpression":          {},
	"ArrowFunction":               {},
	"ClassDeclaration":            {},
	"ClassExpression":             {},
	"InterfaceDeclaration":        {},
	"TypeAliasDeclaration":        {},
	"EnumDeclaration":             {},
	"EnumMember":                  {},
	"ModuleDeclaration":           {},
	"ModuleBlock":                 {},
	"VariableDeclaration":         {},
	"MissingDeclaration":          {},
	"SemicolonClassElement":       {},
	"HeritageClause":              {},
	"JSTypeAliasDeclaration":      {},

	// Module syntax. Never evaluated inside a function body, and listed so a
	// nested `declare module` block cannot refuse a whole transcript.
	"NamespaceExportDeclaration": {},
	"ImportEqualsDeclaration":    {},
	"ImportDeclaration":          {},
	"ImportClause":               {},
	"NamespaceImport":            {},
	"NamedImports":               {},
	"ImportSpecifier":            {},
	"ExportAssignment":           {},
	"ExportDeclaration":          {},
	"NamedExports":               {},
	"NamespaceExport":            {},
	"ExportSpecifier":            {},
	"ExternalModuleReference":    {},
	"ImportAttributes":           {},
	"ImportAttribute":            {},
	"JSImportDeclaration":        {},

	// Expressions that evaluate their children and combine the results
	// without reaching any user code themselves.
	//
	// ObjectLiteralExpression is listed for *both* of its positions. As a value
	// it combines its members' results. As an assignment pattern,
	// `({ a } = src)`, each member performs its own Get on the source value,
	// and that read is the member's row — PropertyAssignment and
	// ShorthandPropertyAssignment are classified in the switch above, exactly
	// as an object pattern's reads are its BindingElements' rows. What the
	// literal itself adds in that position is at most a null check.
	// ArrayLiteralExpression is deliberately *not* listed: in assignment-target
	// position it drives the iteration protocol, which is nobody else's row.
	"ObjectLiteralExpression": {},
	"ObjectBindingPattern":    {},
	"ConditionalExpression":   {},
	"OmittedExpression":       {},
	"MetaProperty":            {},
	"TemplateSpan":            {},
	// The call census owns both of these. They are the only two kinds listed
	// here because another census records them rather than because they invoke
	// nothing.
	"CallExpression": {},
	"NewExpression":  {},
	// Erased at runtime: the value is the operand's, unchanged.
	"TypeAssertionExpression": {},
	"AsExpression":            {},
	"SatisfiesExpression":     {},
	"NonNullExpression":       {},
	"ParenthesizedExpression": {},
	// `typeof x` and `void x` read no property of the operand's value.
	// `delete obj.x` reaches a proxy's deleteProperty trap and nothing else;
	// proxy traps are out of the producer's reach entirely (see
	// UncensusedInvokingForm's doc comment), and the property access itself is
	// the operand's own row.
	"TypeOfExpression": {},
	"VoidExpression":   {},
	"DeleteExpression": {},

	// Statements and clauses. Control flow reaches no user code of its own;
	// `switch` compares with strict equality, which performs no coercion.
	"Block":               {},
	"EmptyStatement":      {},
	"VariableStatement":   {},
	"ExpressionStatement": {},
	"IfStatement":         {},
	"DoStatement":         {},
	"WhileStatement":      {},
	"ForStatement":        {},
	// `for…in` enumerates own and inherited enumerable string keys. On a plain
	// object that reaches nothing; on a proxy it reaches the ownKeys trap,
	// which is the proxy limit recorded on UncensusedInvokingForm and not
	// something a marker here could establish either way.
	"ForInStatement":    {},
	"ContinueStatement": {},
	"BreakStatement":    {},
	"ReturnStatement":   {},
	"SwitchStatement":   {},
	"CaseBlock":         {},
	"CaseClause":        {},
	"DefaultClause":     {},
	"LabeledStatement":  {},
	"ThrowStatement":    {},
	"TryStatement":      {},
	"CatchClause":       {},
	"DebuggerStatement": {},
	"SourceFile":        {},
	"SyntaxList":        {},

	// JSX structure. The enclosing element, self-closing element, or fragment
	// already carries an UncensusedJSXElement row, which is the refusal; its
	// scaffolding adds nothing. A JsxSpreadAttribute is deliberately absent —
	// it reads every own enumerable property of a value the producer cannot
	// enumerate, so it is classified as an unresolved accessor access.
	// JsxText and JsxTextAllWhiteSpaces are absent for the opposite reason:
	// they are *token* kinds, below ast.KindFirstNode, and the token boundary
	// clears them before this list is consulted.
	"JsxOpeningElement":  {},
	"JsxClosingElement":  {},
	"JsxOpeningFragment": {},
	"JsxClosingFragment": {},
	"JsxAttributes":      {},
	"JsxAttribute":       {},
	"JsxExpression":      {},
	"JsxNamespacedName":  {},

	// Transform artifacts a parsed body never contains, listed so that their
	// presence in some future synthetic tree is not a refusal.
	"SyntheticExpression":          {},
	"NotEmittedStatement":          {},
	"NotEmittedTypeElement":        {},
	"PartiallyEmittedExpression":   {},
	"SyntheticReferenceExpression": {},
}

// coercingBinaryOperators are the binary operators that apply ToPrimitive (or
// ToNumber/ToString, which go through it) to their operands. `===`/`!==` and
// the logical and comma operators are absent because they coerce nothing;
// `instanceof` is absent because it has its own kind; `in` is absent because
// it reaches only a proxy's `has` trap, which is out of reach.
//
// Compound assignments are included: `total += obj` coerces exactly as
// `total + obj` does.
var coercingBinaryOperators = map[string]struct{}{
	"PlusToken":                                    {},
	"MinusToken":                                   {},
	"AsteriskToken":                                {},
	"AsteriskAsteriskToken":                        {},
	"SlashToken":                                   {},
	"PercentToken":                                 {},
	"LessThanToken":                                {},
	"GreaterThanToken":                             {},
	"LessThanEqualsToken":                          {},
	"GreaterThanEqualsToken":                       {},
	"EqualsEqualsToken":                            {},
	"ExclamationEqualsToken":                       {},
	"LessThanLessThanToken":                        {},
	"GreaterThanGreaterThanToken":                  {},
	"GreaterThanGreaterThanGreaterThanToken":       {},
	"AmpersandToken":                               {},
	"BarToken":                                     {},
	"CaretToken":                                   {},
	"PlusEqualsToken":                              {},
	"MinusEqualsToken":                             {},
	"AsteriskEqualsToken":                          {},
	"AsteriskAsteriskEqualsToken":                  {},
	"SlashEqualsToken":                             {},
	"PercentEqualsToken":                           {},
	"LessThanLessThanEqualsToken":                  {},
	"GreaterThanGreaterThanEqualsToken":            {},
	"GreaterThanGreaterThanGreaterThanEqualsToken": {},
	"AmpersandEqualsToken":                         {},
	"BarEqualsToken":                               {},
	"CaretEqualsToken":                             {},
}

// coercingUnaryOperators are the prefix operators that coerce their operand.
// `!` is absent (ToBoolean reaches no user code), `typeof` and `void` and
// `delete` are their own node kinds.
var coercingUnaryOperators = map[string]struct{}{
	"PlusToken":       {},
	"MinusToken":      {},
	"TildeToken":      {},
	"PlusPlusToken":   {},
	"MinusMinusToken": {},
}

func nodeKindName(node *ast.Node) string {
	if node == nil {
		return ""
	}
	return strings.TrimPrefix(node.KindString(), "Kind")
}

// uncensusedInvokingFormCensusLocked classifies every node of an
// implementation body that can invoke user code and that
// implementationCallCensusLocked does not record.
//
// It walks the *same* body with the *same* walker as the call census, so the
// two never disagree about which callable frame a position sits in or whether
// invoking the export reaches it. What it does not share is the call census's
// jump withholding: dropping a row there keeps an over-optimistic Reach off
// the wire, and here dropping a row is silence — the exact failure this census
// exists to prevent. An over-optimistic Reach on a marker can only make a
// consumer refuse a form that might not have run, which is the safe direction.
//
// The empty slice and the nil slice mean the same thing on the wire and both
// mean "every form I walked was a call, a construction, or provably
// non-invoking". A consumer distinguishes that positive claim from a producer
// that has no opinion by the handshake protocol, never by the field's
// emptiness.
func (p *project) uncensusedInvokingFormCensusLocked(
	implementation *ast.Node,
) []typefacts.UncensusedInvokingForm {
	var forms []typefacts.UncensusedInvokingForm
	roots := p.parameterSubjectRootsLocked(implementation)
	p.walkImplementationBodyLocked(
		implementation,
		func(node *ast.Node, enclosing *ast.Node, reach typefacts.Reachability) {
			// walkImplementationBodyLocked starts at implementation.Body(), so
			// the implementation node itself is never observed and needs no
			// filter here.
			kind, recorded := p.classifyInvokingFormLocked(node)
			if !recorded {
				return
			}
			form := typefacts.UncensusedInvokingForm{
				Kind:     kind,
				NodeKind: nodeKindName(node),
				Location: nodeLocation(node),
				Reach:    reach,
				Captured: enclosing != nil,
			}
			if enclosing != nil {
				enclosingLocation := nodeLocation(enclosing)
				form.EnclosingCallable = &enclosingLocation
			}
			form.SubjectParameter = p.accessorFormSubjectParameterLocked(node, kind, roots)
			forms = append(forms, form)
		},
	)
	return forms
}

// parameterSubjectRoots is the ADR 0034 premise set for one declaration: the
// parameters a form's subject may be rooted at, or nil when the declaration
// admits none.
//
// A parameter qualifies when its binding is a plain identifier with no
// initializer and no rest token, and the identifier is written nowhere in its
// file. A defaulted parameter is excluded because the default value is an
// object *this* code created, not one the caller handed over; a rest parameter
// because the array is the engine's; a destructured element because the
// pattern already read a property the caller's object may compute. The whole
// declaration is excluded when it mentions `arguments` or `eval`, either of
// which can rebind a parameter without a visible assignment.
type parameterSubjectRoots struct {
	byParameterSymbol map[*ast.Symbol]int
}

func (p *project) parameterSubjectRootsLocked(implementation *ast.Node) *parameterSubjectRoots {
	if implementation == nil || mentionsArgumentsOrEval(implementation) {
		return nil
	}
	roots := &parameterSubjectRoots{byParameterSymbol: make(map[*ast.Symbol]int)}
	for index, parameter := range implementation.Parameters() {
		name := parameter.Name()
		if name == nil || !ast.IsIdentifier(name) || parameter.Initializer() != nil ||
			parameter.AsParameterDeclaration().DotDotDotToken != nil {
			continue
		}
		symbol := p.canonicalSymbol(p.formChecker().GetSymbolAtLocation(name))
		if symbol == nil || p.parameterIsWrittenLocked(implementation, index, symbol) {
			continue
		}
		roots.byParameterSymbol[symbol] = index
	}
	if len(roots.byParameterSymbol) == 0 {
		return nil
	}
	return roots
}

// parameterIsWrittenLocked answers the ADR 0029 write question for one
// parameter. Under a premise twin (ADR 0038) it is asked of the *accepted*
// program's node for the same parameter: the twin is the same file with one
// comment added, so the answer is the same, and the accepted program has
// already walked that file's assignment targets once — where the twin's cold
// checker would walk the whole file again for every twin built over it.
func (p *project) parameterIsWrittenLocked(implementation *ast.Node, index int, symbol *ast.Symbol) bool {
	if twin := p.formTwin; twin != nil && implementation == twin.implementation && twin.original != nil {
		originals := twin.original.Parameters()
		if index >= len(originals) || originals[index].Name() == nil {
			return true
		}
		original := p.canonicalSymbol(p.checker.GetSymbolAtLocation(originals[index].Name()))
		return original == nil || p.symbolIsAssignedLocked(original, twin.original)
	}
	return p.symbolIsAssignedLocked(symbol, implementation)
}

// mentionsArgumentsOrEval reports whether any identifier in the subtree spells
// `arguments` or `eval`. Direct eval and mapped arguments can mutate a binding
// without a visible assignment to its symbol, so a premise about an unwritten
// binding refuses the mention rather than guessing strictness.
func mentionsArgumentsOrEval(root *ast.Node) bool {
	var dynamic bool
	var scan func(*ast.Node)
	scan = func(node *ast.Node) {
		if node == nil || dynamic {
			return
		}
		if ast.IsIdentifier(node) && (node.Text() == "eval" || node.Text() == "arguments") {
			dynamic = true
			return
		}
		node.ForEachChild(func(child *ast.Node) bool { scan(child); return dynamic })
	}
	scan(root)
	return dynamic
}

// subjectParameterLocked answers the qualifying parameter a subject expression
// is rooted at: the expression, after identity-preserving unwrapping, is either
// a reference to such a parameter or a chain of property, element and
// optional-chain reads whose innermost receiver is one. Anything else — a call
// result, a module binding, a nested callable's own parameter, a literal —
// answers nil.
func (p *project) subjectParameterLocked(subject *ast.Node, roots *parameterSubjectRoots) *int {
	if roots == nil {
		return nil
	}
	node := identityPreservingUnwrap(subject)
	for node != nil && (ast.IsPropertyAccessExpression(node) || nodeKindName(node) == "ElementAccessExpression") {
		node = identityPreservingUnwrap(node.Expression())
	}
	if node == nil || !ast.IsIdentifier(node) {
		return nil
	}
	symbol := p.canonicalSymbol(p.formChecker().GetSymbolAtLocation(node))
	if symbol == nil {
		return nil
	}
	index, ok := roots.byParameterSymbol[symbol]
	if !ok {
		return nil
	}
	return &index
}

// accessorFormSubjectParameterLocked states the subject parameter for exactly
// the two read-accessor kinds ADR 0034 admits, in read position, on a property
// or element access node. Every other form — a setter, a write position, a
// `delete`, a destructuring pattern, a spread, an iteration, a coercion —
// stays unstated, which the consumer reads as "refuse as before".
func (p *project) accessorFormSubjectParameterLocked(
	node *ast.Node,
	kind typefacts.UncensusedInvokingFormKind,
	roots *parameterSubjectRoots,
) *int {
	if roots == nil {
		return nil
	}
	if kind != typefacts.UncensusedGetAccessor && kind != typefacts.UncensusedPropertyAccessUnknownAccessor {
		return nil
	}
	if !ast.IsPropertyAccessExpression(node) && nodeKindName(node) != "ElementAccessExpression" {
		return nil
	}
	if ast.GetAssignmentTarget(node) != nil {
		return nil
	}
	if parent := node.Parent; parent != nil && nodeKindName(parent) == "DeleteExpression" {
		return nil
	}
	return p.subjectParameterLocked(node.Expression(), roots)
}

// thisProtocolCallLocked states the receiver of a `.call` or `.apply` whose
// resolved callee is the default library's Function.prototype member, together
// with the parameter its `this` argument is rooted at (ADR 0034). The consumer
// decides which receivers it has reviewed; this side only states the facts.
func (p *project) thisProtocolCallLocked(
	node *ast.Node,
	callee *typefacts.ResolvedDeclaration,
	roots *parameterSubjectRoots,
) (*typefacts.ResolvedDeclaration, *int) {
	if node == nil || !ast.IsCallExpression(node) || callee == nil || !callee.StandardLibrary {
		return nil, nil
	}
	if callee.Name != "call" && callee.Name != "apply" {
		return nil, nil
	}
	expression := identityPreservingUnwrap(node.Expression())
	if expression == nil || !ast.IsPropertyAccessExpression(expression) {
		return nil, nil
	}
	_, _, _, receiver := p.implementationCallTargetLocked(expression.Expression())
	if receiver == nil {
		return nil, nil
	}
	var thisParameter *int
	if arguments := node.Arguments(); len(arguments) > 0 && exactArgumentSlots(node) > 0 {
		thisParameter = p.subjectParameterLocked(arguments[0], roots)
	}
	return receiver, thisParameter
}

// classifyInvokingFormLocked answers the marker kind for one node, or false
// when the node provably invokes nothing of its own accord.
//
// The order of the switch is the order of the vocabulary, and the *final*
// arm is the one that matters: anything that is neither a token, nor a
// classified form, nor a reviewed non-invoking kind is
// UncensusedUnclassifiedInvokingForm. There is no "ignore" default.
func (p *project) classifyInvokingFormLocked(
	node *ast.Node,
) (typefacts.UncensusedInvokingFormKind, bool) {
	// Tokens — trivia, literals, punctuation, identifiers, keywords — carry no
	// evaluation of their own. Taking the boundary from the compiler rather
	// than listing the 166 kinds below it means a keyword a future revision
	// adds is covered without an edit here.
	if node.Kind < ast.KindFirstNode {
		return "", false
	}
	name := nodeKindName(node)
	switch name {
	case "TaggedTemplateExpression":
		return typefacts.UncensusedTaggedTemplate, true
	case "Decorator":
		return typefacts.UncensusedDecorator, true
	case "JsxElement", "JsxSelfClosingElement", "JsxFragment":
		return typefacts.UncensusedJSXElement, true
	case "ForOfStatement", "SpreadElement", "ArrayBindingPattern":
		// All three drive `Symbol.iterator` (or `Symbol.asyncIterator`, for
		// `for await…of`, which is a ForOfStatement carrying an await
		// modifier) and then the iterator's own `next` and `return`.
		return p.iterationFormLocked(node)
	case "YieldExpression":
		if yield := node.AsYieldExpression(); yield != nil && yield.AsteriskToken != nil {
			// `yield*` drives the delegate's iterator exactly as `for…of`
			// does, over the operand's own type.
			return p.iterationProtocolFormLocked(yield.Expression)
		}
		// A plain `yield` suspends; it invokes nothing. Whether a generator's
		// body runs at all is a reachability question about the caller's `next`
		// calls, which this census does not answer and does not pretend to.
		return "", false
	case "ArrayLiteralExpression":
		// As a *value* an array literal only combines its elements' results:
		// `[a.x]` invokes a getter, but that getter is the child access's own
		// row. In assignment-target position the compiler reinterprets the same
		// node kind as a destructuring pattern (checkDestructuringAssignment),
		// and `[a, b] = src` drives src's `Symbol.iterator` and that
		// iterator's `next`/`return` exactly as an ArrayBindingPattern does in
		// a declaration. `=` is an assignment, not a coercing operator, so
		// nothing else in this classifier would see it.
		//
		// ast.GetAssignmentTarget is what separates the two positions — the
		// same question the parameter-use census asks — and it walks up through
		// nested patterns, parentheses, and non-null assertions, so
		// `[{ a }] = src` and `for ([a] of pairs)` are both answered here.
		//
		// Unlike the three kinds above, this arm asks no type question and
		// records unconditionally. It has no operand to ask about: the iterated
		// value is the assignment's right-hand side, or — inside
		// `for ([a] of pairs)` — the element type of a *different* node's
		// iteration, and GetTypeAtLocation on the literal answers with the
		// shape of the pattern rather than of the source, which is the same
		// trap objectAssignmentPatternMemberFormLocked documents. Deriving the
		// source here is its own premise, so the form stands until one is
		// written.
		if ast.GetAssignmentTarget(node) != nil {
			return typefacts.UncensusedIterationProtocol, true
		}
		return "", false
	case "PropertyAssignment", "ShorthandPropertyAssignment":
		return p.objectAssignmentPatternMemberFormLocked(node)
	case "VariableDeclarationList":
		if node.Flags&ast.NodeFlagsUsing != 0 {
			// Both `using x = …` and `await using x = …`: scope exit reaches
			// `Symbol.dispose` or `Symbol.asyncDispose` on every value the
			// list declared.
			return typefacts.UncensusedUsingDispose, true
		}
		return "", false
	case "SpreadAssignment", "JsxSpreadAttribute":
		// Object spread and JSX prop spread copy every own enumerable
		// property of a value whose runtime shape the producer cannot
		// enumerate, invoking each getter among them. That is precisely "the
		// checker cannot tell whether the member is an accessor", so it is
		// recorded under that kind rather than as an unclassified form.
		return typefacts.UncensusedPropertyAccessUnknownAccessor, true
	case "PropertyAccessExpression", "ElementAccessExpression":
		return p.accessorFormLocked(node)
	case "BindingElement":
		// A binding element in an *object* pattern names a property, and
		// reading it invokes that property's getter. In an array pattern it is
		// positional, and the ArrayBindingPattern row above already covers the
		// iteration it performs.
		parent := node.Parent
		if parent == nil || nodeKindName(parent) != "ObjectBindingPattern" {
			return "", false
		}
		return p.bindingElementAccessorFormLocked(node, parent)
	case "ComputedPropertyName":
		// The key expression is coerced to a property key, which reaches
		// `Symbol.toPrimitive`/`toString` on an object-typed key.
		return p.coercionFormLocked(node.Expression())
	case "AwaitExpression":
		return p.awaitFormLocked(node)
	case "TemplateExpression":
		return p.templateCoercionFormLocked(node)
	case "BinaryExpression":
		return p.binaryFormLocked(node)
	case "PrefixUnaryExpression", "PostfixUnaryExpression":
		return p.unaryFormLocked(node)
	}
	if _, listed := nonInvokingNodeKinds[name]; listed {
		return "", false
	}
	// Every JSDoc node kind, by prefix. They are comment content: the compiler
	// may attach them to a tree but nothing in them is evaluated. The prefix
	// is used rather than today's thirty-six names so that a JSDoc tag a future
	// revision adds does not become an unclassified invoking form.
	if strings.HasPrefix(name, "JSDoc") {
		return "", false
	}
	return typefacts.UncensusedUnclassifiedInvokingForm, true
}

// accessorFormLocked classifies a property or element access by what the
// checker knows about the member it reaches.
//
// A resolved symbol whose declarations include a get or set accessor is that
// accessor, and which of the two is decided by whether the access is an
// assignment target — the same question ast.GetAssignmentTarget answers for
// the parameter-use census. A resolved symbol with no accessor declaration is
// a plain data property or a method, and reading it invokes nothing: no row. An
// *unresolved* member is neither, and it is recorded, because absence of a
// symbol is not evidence of a plain property.
func (p *project) accessorFormLocked(
	node *ast.Node,
) (typefacts.UncensusedInvokingFormKind, bool) {
	queried := node.Name()
	if queried == nil {
		// An element access has no name node, and querying the access itself
		// resolves nothing at all — literal key or not. The compiler names the
		// accessed member from the *argument expression*, and only when that
		// expression is an exact string or numeric literal
		// (checker.getSymbolAtLocation's KindStringLiteral/KindNumericLiteral
		// arm). A computed key therefore has nothing to query, and refuses.
		queried = exactElementAccessKey(node)
	}
	if queried == nil {
		return typefacts.UncensusedPropertyAccessUnknownAccessor, true
	}
	symbol := p.canonicalSymbol(p.formChecker().GetSymbolAtLocation(queried))
	if symbol == nil {
		return typefacts.UncensusedPropertyAccessUnknownAccessor, true
	}
	return p.accessorKindForSymbolLocked(symbol, ast.GetAssignmentTarget(node) != nil)
}

// exactElementAccessKey answers the node to query for the member an element
// access reaches, or nil when the key is not statically a property name. Only
// an exact string or numeric literal is one: the compiler resolves the
// property from such a literal and from nothing else, so every other key —
// an identifier, a template with substitutions, an expression — refuses.
func exactElementAccessKey(node *ast.Node) *ast.Node {
	access := node.AsElementAccessExpression()
	if access == nil || access.ArgumentExpression == nil {
		return nil
	}
	switch nodeKindName(access.ArgumentExpression) {
	case "StringLiteral", "NoSubstitutionTemplateLiteral", "NumericLiteral":
		return access.ArgumentExpression
	}
	return nil
}

// objectAssignmentPatternMemberFormLocked classifies one member of an object
// *assignment* pattern — `({ a } = src)`, `({ a: x } = src)` — by what the
// checker knows about the property the member reads from the source value.
//
// In a value position the same two node kinds invoke nothing of their own
// accord: `const o = { a }` builds an object and reads no property of any
// other value. What separates the positions is ast.GetAssignmentTarget on the
// containing literal, which is the same question the parameter-use census
// asks, and which the compiler answers the same way by reinterpreting the
// literal through checkDestructuringAssignment.
//
// The property is resolved with the compiler's own
// GetPropertySymbolOfDestructuringAssignment rather than through
// GetTypeAtLocation on the literal: in an assignment pattern the literal's
// expression type is the shape of the *pattern*, not of the source, so
// resolving a member through it would answer about the wrong value. A member
// the compiler cannot resolve — a computed or non-identifier key, an `any`
// source, an index signature — is recorded as an unresolved accessor, because
// absence of a symbol is not evidence of a plain data property.
func (p *project) objectAssignmentPatternMemberFormLocked(
	member *ast.Node,
) (typefacts.UncensusedInvokingFormKind, bool) {
	pattern := member.Parent
	if pattern == nil || !ast.IsObjectLiteralExpression(pattern) ||
		ast.GetAssignmentTarget(pattern) == nil {
		return "", false
	}
	name := member.Name()
	if name == nil || !ast.IsIdentifier(name) {
		return typefacts.UncensusedPropertyAccessUnknownAccessor, true
	}
	symbol := p.canonicalSymbol(p.formChecker().GetPropertySymbolOfDestructuringAssignment(name))
	if symbol == nil {
		return typefacts.UncensusedPropertyAccessUnknownAccessor, true
	}
	// Destructuring only reads.
	return p.accessorKindForSymbolLocked(symbol, false)
}

// bindingElementAccessorFormLocked resolves the property one object-pattern
// binding element destructures, through the pattern's own type rather than
// through the element's local binding — the local name resolves to the new
// variable, which says nothing about the property being read.
func (p *project) bindingElementAccessorFormLocked(
	node *ast.Node,
	pattern *ast.Node,
) (typefacts.UncensusedInvokingFormKind, bool) {
	if element := node.AsBindingElement(); element == nil || element.DotDotDotToken != nil {
		// A rest element reads *every* remaining own enumerable property of the
		// source, invoking each getter among them, and it names none of them —
		// its own identifier names the new object. Resolving that identifier as
		// a property key would be a category error: on a source that happens to
		// carry a `rest` property, `const { ...rest } = src` would answer about
		// that property and could clear the row entirely.
		return typefacts.UncensusedPropertyAccessUnknownAccessor, true
	}
	name := node.PropertyName()
	if name == nil {
		name = node.Name()
	}
	if name == nil || !ast.IsIdentifier(name) {
		// A computed property name is not statically a key.
		return typefacts.UncensusedPropertyAccessUnknownAccessor, true
	}
	patternType := p.formChecker().GetTypeAtLocation(pattern)
	if patternType == nil {
		return typefacts.UncensusedPropertyAccessUnknownAccessor, true
	}
	symbol := p.canonicalSymbol(p.formChecker().GetPropertyOfType(patternType, name.Text()))
	if symbol == nil {
		return typefacts.UncensusedPropertyAccessUnknownAccessor, true
	}
	// Destructuring only reads.
	return p.accessorKindForSymbolLocked(symbol, false)
}

// accessorKindForSymbolLocked answers the accessor kind of one resolved member
// symbol, or false when reading that member provably invokes nothing.
//
// **The premise, stated exactly.** "The declarations include no accessor,
// therefore reading the member invokes nothing" holds only when the
// declarations inspected here *are* the bytes that run. A hand-written
// `index.d.ts` may declare `readonly value: number` over a published `.js`
// getter; the symbol resolved through that declaration carries no GetAccessor
// node at all, so the premise would silently certify a getter this census
// never saw. Every declaration must therefore sit in a file the analyzed
// snapshot carries as runtime source — a non-declaration file of the accepted
// program, which is exactly the set the producer publishes as Sources(). A
// declaration in any other file, or in no file at all, is recorded as an
// unresolved accessor.
//
// The default library is the one admissible exception, and it is not a
// weakening: `lib.*.d.ts` describes the engine, whose implementation is not
// user code, so no `lib`-declared member can reach a user callable however the
// engine implements it. Without the exception every `arr.length` would refuse.
//
// Two limits remain, and neither is closed here. The premise is about
// *declarations*, so a subclass that redeclares a plain member as a getter is
// invisible when the static type names the base declaration; and a Proxy trap
// is outside every producer census, for the reason UncensusedInvokingForm's
// doc comment gives.
func (p *project) accessorKindForSymbolLocked(
	symbol *ast.Symbol,
	assignmentTarget bool,
) (typefacts.UncensusedInvokingFormKind, bool) {
	if len(symbol.Declarations) == 0 {
		return typefacts.UncensusedPropertyAccessUnknownAccessor, true
	}
	get, set := false, false
	for _, declaration := range symbol.Declarations {
		if !p.declarationCarriesRuntimeBytesLocked(declaration) {
			return typefacts.UncensusedPropertyAccessUnknownAccessor, true
		}
		switch nodeKindName(declaration) {
		case "GetAccessor":
			get = true
		case "SetAccessor":
			set = true
		}
	}
	switch {
	case assignmentTarget && set:
		return typefacts.UncensusedSetAccessor, true
	case assignmentTarget && get:
		// A get-only accessor written to. The write reaches no setter, but the
		// member is an accessor and the position is not one this census was
		// reviewed for, so it refuses under the accessor it does resolve.
		return typefacts.UncensusedGetAccessor, true
	case get:
		return typefacts.UncensusedGetAccessor, true
	case set:
		// A set-only accessor read answers undefined and invokes nothing, but
		// the member is an accessor; refuse rather than model the asymmetry.
		return typefacts.UncensusedSetAccessor, true
	}
	return "", false
}

// declarationCarriesRuntimeBytesLocked answers whether the accessor premise
// above may read a declaration node: the node sits either in the default
// library, whose declarations describe the engine rather than user code, or in
// a non-declaration file of the accepted program, which is the runtime source
// set the producer publishes as its snapshot.
//
// It is deliberately a statement about the *analyzed program* and not about
// package boundaries. A dependency's own `.ts`/`.js` file that the program
// pulled in is runtime source here, and a consumer whose claim needs a
// narrower artifact boundary must check the declaration's path itself.
func (p *project) declarationCarriesRuntimeBytesLocked(declaration *ast.Node) bool {
	sourceFile := ast.GetSourceFileOfNode(declaration)
	if sourceFile == nil {
		return false
	}
	if p.program.IsSourceFileDefaultLibrary(sourceFile.Path()) {
		return true
	}
	return !sourceFile.IsDeclarationFile && p.formIsRuntimeSourceFile(sourceFile)
}

// engineOwnedIterableContainers is the reviewed list of default-library
// interfaces whose declaration of `[Symbol.iterator]` is the engine's own
// iterator factory *and* whose values are objects the engine itself created, so
// that the iterator the factory returns — and therefore that iterator's `next`
// and `return` — is engine code as well. Nothing here can reach a user
// callable however the engine implements it, which is the same premise
// accessorKindForSymbolLocked states for `lib`-declared members.
//
// The two halves of that premise are why the *protocol* interfaces are
// deliberately absent, and their absence is the whole precision of this table:
// `Iterable`, `IterableIterator`, `IteratorObject`, `Iterator`, `ArrayIterator`,
// `MapIterator`, `SetIterator`, `StringIterator`, `RegExpStringIterator`,
// `SegmentIterator` and `Segments` all declare `[Symbol.iterator]` in the
// default library, but every one of them is a structural contract a user object
// satisfies — so the factory named by the declaration is not the factory that
// runs. `Generator` is the sharpest case and the reason to state this rather
// than infer it: a generator's `next` runs a user function body. This is
// exactly the `Promise` versus `PromiseLike` split that
// provablyEngineOwnedThenLocked draws, for the same reason.
//
// Every DOM and web-worker collection is absent too — `NodeList`,
// `URLSearchParams`, `Headers`, `FormData` and some forty others. Their
// iterators are engine code in fact, but they were not reviewed here, and "the
// browser probably owns it" is not a premise; defaultLibraryMemberInvokers
// keeps the same rule about growing a table.
//
// Two limits remain, and they are the ones every declaration-based premise in
// this file carries. A value whose static type is `Array<T>` while the runtime
// object is a subclass overriding `[Symbol.iterator]` answers from the base
// declaration, and a Proxy is outside every producer census. A constrained type
// parameter clears through its constraint's apparent type for the same reason
// `await value` does when `T extends Promise<number>`.
var engineOwnedIterableContainers = containerSet(
	// lib.es2015.iterable.d.ts.
	"Array", "ReadonlyArray", "String", "IArguments",
	"Set", "ReadonlySet", "Map", "ReadonlyMap",
	"Int8Array", "Uint8Array", "Uint8ClampedArray",
	"Int16Array", "Uint16Array", "Int32Array", "Uint32Array",
	"Float32Array", "Float64Array",
	// lib.es2020.bigint.d.ts and lib.es2025.float16.d.ts. A project whose lib
	// omits either declares no such global, and the name simply never resolves.
	"BigInt64Array", "BigUint64Array", "Float16Array",
)

// iterationFormLocked classifies one of the three syntaxes that drive the
// iteration protocol by what the checker knows about the value being iterated.
//
// `for await…of` is *always* recorded and is the one arm here that asks no type
// question. It resolves `Symbol.asyncIterator` first — declared in the default
// library only by `AsyncIterable`, `AsyncIterableIterator`, `AsyncGenerator`
// and `AsyncIteratorObject`, every one of them a structural contract whose
// `next` is a user function body — and when the value carries none of them it
// falls back to the sync protocol and `await`s each result, invoking whatever
// `then` those values carry. Neither half has an engine-owned case worth a
// table row, so the form stands.
func (p *project) iterationFormLocked(
	node *ast.Node,
) (typefacts.UncensusedInvokingFormKind, bool) {
	switch nodeKindName(node) {
	case "ForOfStatement":
		if statement := node.AsForInOrOfStatement(); statement == nil ||
			statement.AwaitModifier != nil {
			return typefacts.UncensusedIterationProtocol, true
		}
		return p.iterationProtocolFormLocked(node.Expression())
	case "SpreadElement":
		return p.iterationProtocolFormLocked(node.Expression())
	case "ArrayBindingPattern":
		// A binding pattern has no operand expression: the iterated value is
		// the declaration's initializer or the parameter's declared type, and
		// GetTypeAtLocation on the pattern answers with exactly that — the
		// same question bindingElementAccessorFormLocked asks of an object
		// pattern to resolve the property it reads. An *assignment* pattern is
		// spelled ArrayLiteralExpression, not this kind, and is answered in
		// classifyInvokingFormLocked without a type question.
		return p.iterationProtocolClearedLocked(p.formChecker().GetTypeAtLocation(node))
	}
	return typefacts.UncensusedIterationProtocol, true
}

// iterationProtocolFormLocked records the iteration protocol unless the
// operand's type is provably one whose iterator is the engine's.
func (p *project) iterationProtocolFormLocked(
	operand *ast.Node,
) (typefacts.UncensusedInvokingFormKind, bool) {
	if operand == nil {
		return typefacts.UncensusedIterationProtocol, true
	}
	return p.iterationProtocolClearedLocked(p.formChecker().GetTypeAtLocation(operand))
}

func (p *project) iterationProtocolClearedLocked(
	iterated *checker.Type,
) (typefacts.UncensusedInvokingFormKind, bool) {
	if p.provablyEngineOwnedIteratorLocked(iterated) {
		return "", false
	}
	return typefacts.UncensusedIterationProtocol, true
}

// provablyEngineOwnedIteratorLocked answers whether *every* constituent of a
// type carries a `[Symbol.iterator]` that engineOwnedIterableContainers vouches
// for.
//
// The quantifier is the whole of it, for the reason
// provablyEngineOwnedThenLocked's is: a union missing the member in one
// constituent carries it in another, `any` and `unknown` and an unconstrained
// type parameter enumerate no members at all, and an index-signature type such
// as `Record<string, unknown>` declares no iterator while permitting one at
// runtime. "The checker could not find `[Symbol.iterator]`" must never read as
// "iterating this reaches no user code" — which is why a nil lookup refuses
// here instead of clearing. That direction also means a non-iterable operand
// records a form it cannot actually reach; iterating a number is a `tsc` error
// and a runtime TypeError, and this census does not trade a fail-closed
// quantifier for silence on code that does not run.
//
// The key is asked of the compiler rather than spelled: a well-known-symbol
// member is stored under a name derived from the program's own
// `SymbolConstructor` declaration when it has one. See
// Checker_getPropertyNameForKnownSymbolName.
//
// An *optional* `[Symbol.iterator]?` refuses, matching the compiler's own
// iterable resolver, which requires the member to be non-optional before it
// will read the protocol off it.
func (p *project) provablyEngineOwnedIteratorLocked(iterated *checker.Type) bool {
	if iterated == nil {
		return false
	}
	constituents := iterated.Distributed()
	if len(constituents) == 0 {
		return false
	}
	key := checker.Checker_getPropertyNameForKnownSymbolName(p.formChecker(), "iterator")
	if key == "" {
		return false
	}
	for _, constituent := range constituents {
		if constituent == nil {
			return false
		}
		if constituent.Flags()&(checker.TypeFlagsAny|checker.TypeFlagsUnknown) != 0 {
			return false
		}
		iterator := p.formChecker().GetPropertyOfType(constituent, key)
		if iterator == nil || iterator.Flags&ast.SymbolFlagsOptional != 0 {
			return false
		}
		if !p.isDefaultLibraryMemberLocked(iterator, key, engineOwnedIterableContainers) {
			return false
		}
	}
	return true
}

// awaitFormLocked records an `await` unless *every* constituent of the awaited
// value's type is provably one the runtime resolves without entering user
// code: a primitive, which has no `then` to call, or a default-library
// `Promise`, whose `then` is the engine's own.
//
// A `PromiseLike` is recorded: its `then` is whatever the value carries, which
// is precisely a user callable the census cannot resolve.
//
// The quantifier is the whole of it, and it is per constituent for the same
// reason coercionFormLocked's is. "The type has no `then` member" is not proof
// that awaiting invokes nothing: a union missing `then` in one constituent
// carries it in another, an unconstrained type parameter has no members the
// checker can enumerate, and an index-signature type such as
// `Record<string, unknown>` declares no `then` while permitting one at
// runtime, whose Get would reach a getter and whose value `await` would call.
// An earlier form of this function read a nil `then` lookup as proof and
// stayed silent on all three. "The checker could not find a member" must never
// read as "no member is reached here".
func (p *project) awaitFormLocked(
	node *ast.Node,
) (typefacts.UncensusedInvokingFormKind, bool) {
	operand := node.Expression()
	if operand == nil {
		return typefacts.UncensusedAwaitThen, true
	}
	if p.provablyEngineOwnedThenLocked(p.formChecker().GetTypeAtLocation(operand)) {
		return "", false
	}
	return typefacts.UncensusedAwaitThen, true
}

func (p *project) provablyEngineOwnedThenLocked(value *checker.Type) bool {
	if value == nil {
		return false
	}
	constituents := value.Distributed()
	if len(constituents) == 0 {
		return false
	}
	for _, constituent := range constituents {
		if constituent == nil {
			return false
		}
		if constituent.Flags()&(checker.TypeFlagsAny|checker.TypeFlagsUnknown) != 0 {
			return false
		}
		if constituent.Flags()&provablyNonObjectFlags != 0 {
			// A primitive has no own `then`, and `await` on one resolves
			// immediately. `Promise.prototype.then` cannot be reached from it.
			continue
		}
		then := p.formChecker().GetPropertyOfType(constituent, "then")
		if then == nil {
			return false
		}
		if !p.isDefaultLibraryMemberLocked(then, "then", containerSet("Promise")) {
			return false
		}
	}
	return true
}

func (p *project) templateCoercionFormLocked(
	node *ast.Node,
) (typefacts.UncensusedInvokingFormKind, bool) {
	template := node.AsTemplateExpression()
	if template == nil || template.TemplateSpans == nil {
		return "", false
	}
	for _, span := range template.TemplateSpans.Nodes {
		if kind, recorded := p.coercionFormLocked(span.Expression()); recorded {
			return kind, recorded
		}
	}
	return "", false
}

func (p *project) binaryFormLocked(
	node *ast.Node,
) (typefacts.UncensusedInvokingFormKind, bool) {
	binary := node.AsBinaryExpression()
	if binary == nil || binary.OperatorToken == nil {
		return "", false
	}
	operator := nodeKindName(binary.OperatorToken)
	if operator == "InstanceOfKeyword" {
		// `x instanceof C` reaches C[Symbol.hasInstance] when C defines it.
		// Whether it does is a property of the runtime value, so the form is
		// always recorded.
		return typefacts.UncensusedInstanceOf, true
	}
	if (operator == "EqualsEqualsToken" || operator == "ExclamationEqualsToken") &&
		(binary.Left.Kind == ast.KindNullKeyword || binary.Right.Kind == ast.KindNullKeyword) {
		// IsLooselyEqual has a dedicated null/undefined arm. Comparing any
		// value to the exact null literal does not apply ToPrimitive to that
		// value (the browser's HTMLDDA special case does not invoke user code
		// either). Children are still walked, so an access or call used to
		// produce the other operand keeps its own row.
		return "", false
	}
	if _, coercing := coercingBinaryOperators[operator]; !coercing {
		return "", false
	}
	if kind, recorded := p.coercionFormLocked(binary.Left); recorded {
		return kind, recorded
	}
	return p.coercionFormLocked(binary.Right)
}

func (p *project) unaryFormLocked(
	node *ast.Node,
) (typefacts.UncensusedInvokingFormKind, bool) {
	var operator string
	var operand *ast.Node
	if ast.IsPrefixUnaryExpression(node) {
		prefix := node.AsPrefixUnaryExpression()
		operator = strings.TrimPrefix(prefix.Operator.String(), "Kind")
		operand = prefix.Operand
	} else if ast.IsPostfixUnaryExpression(node) {
		postfix := node.AsPostfixUnaryExpression()
		operator = strings.TrimPrefix(postfix.Operator.String(), "Kind")
		operand = postfix.Operand
	}
	if _, coercing := coercingUnaryOperators[operator]; !coercing {
		return "", false
	}
	return p.coercionFormLocked(operand)
}

// coercionFormLocked records a coercion unless the operand is provably not an
// object. `any`, `unknown`, a type parameter, and every other type the
// producer cannot decompose are *not* provably non-object, so they are
// recorded: this is the fail-closed direction, and "the checker could not tell"
// must never read as "no coercion happens here".
func (p *project) coercionFormLocked(
	operand *ast.Node,
) (typefacts.UncensusedInvokingFormKind, bool) {
	if operand == nil {
		return "", false
	}
	if !p.mayBeObjectTypedLocked(p.formChecker().GetTypeAtLocation(operand)) {
		return "", false
	}
	return typefacts.UncensusedCoercion, true
}

// provablyNonObjectFlags are the type flags that, on their own, establish that
// a value is a primitive and therefore has no `Symbol.toPrimitive`, `valueOf`,
// or `toString` a coercion could reach in user code.
const provablyNonObjectFlags = checker.TypeFlagsStringLike |
	checker.TypeFlagsNumberLike |
	checker.TypeFlagsBooleanLike |
	checker.TypeFlagsBigIntLike |
	checker.TypeFlagsESSymbolLike |
	checker.TypeFlagsUndefined |
	checker.TypeFlagsNull |
	checker.TypeFlagsVoid |
	checker.TypeFlagsNever

func (p *project) mayBeObjectTypedLocked(value *checker.Type) bool {
	if value == nil {
		return true
	}
	constituents := value.Distributed()
	if len(constituents) == 0 {
		return true
	}
	for _, constituent := range constituents {
		if constituent == nil {
			return true
		}
		if constituent.Flags()&provablyNonObjectFlags == 0 {
			return true
		}
	}
	return false
}
