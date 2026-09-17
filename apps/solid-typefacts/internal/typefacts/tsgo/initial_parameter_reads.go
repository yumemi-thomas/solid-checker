package tsgo

import (
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts"
)

// This is an opening-prefix proof, not a general control-flow analysis. Each
// admitted statement is a const declaration with simple names and only plain
// identifier/property initializers. At the first plain parameter assignment,
// also admit only its leading RHS call receiver, evaluated before the store.
// Stop before every other statement or effect.
// Getters can run user code, so also exclude every nested callable, arguments,
// eval and parameter initializer: none can then expose or mutate these lexical
// parameter bindings before the recorded use. Later direct assignments do not
// retroactively replace a value that this prefix has already read.
func (p *project) initialParameterReadsLocked(implementation *ast.Node, signature *typefacts.SelectedSignature, demand typefacts.Location, uses []typefacts.ParameterUse) []typefacts.InitialParameterRead {
	if signature == nil || implementationCompletionForm(implementation) != typefacts.CompletionPlain || mentionsArgumentsOrEval(implementation) {
		return nil
	}
	body := implementation.Body()
	if body == nil || !ast.IsBlock(body) {
		return nil
	}
	nested := false
	var scan func(*ast.Node)
	scan = func(node *ast.Node) {
		if isCallableDeclaration(node) {
			nested = true
			return
		}
		node.ForEachChild(func(child *ast.Node) bool { scan(child); return nested })
	}
	scan(body)
	if nested {
		return nil
	}
	names := make(map[string]bool)
	bindings := make(map[int]typefacts.Location)
	// The file's own checker: a premise twin's file (ADR 0038) was bound by the
	// twin program, every other file by the accepted one.
	fileChecker := p.checker
	if sourceFile := ast.GetSourceFileOfNode(implementation); p.formTwin != nil && sourceFile == p.formTwin.file {
		fileChecker = p.formTwin.checker
	}
	symbols := make(map[int]*ast.Symbol)
	for index, parameter := range implementation.Parameters() {
		name := parameter.Name()
		if name == nil || !ast.IsIdentifier(name) || parameter.Initializer() != nil || parameter.AsParameterDeclaration().DotDotDotToken != nil || names[name.Text()] {
			return nil
		}
		names[name.Text()] = true
		if index >= len(signature.Parameters) {
			return nil
		}
		selected := signature.Parameters[index]
		location := nodeLocation(name)
		if selected.Index != index || selected.Rest || selected.Defaulted || selected.Declaration == nil || selected.Declaration.Location != location || location.Path != demand.Path {
			return nil
		}
		bindings[index] = location
		symbols[index] = p.canonicalSymbol(fileChecker.GetSymbolAtLocation(name))
	}
	cutoffs := p.positionalOriginalInputCutoffsLocked(body, symbols, fileChecker)
	defaults := p.undefinedParameterDefaultsLocked(body, symbols, fileChecker)
	prefixEnd := 0
	var firstReceiver *ast.Node
	firstIterationOnly := false
	for _, statement := range body.AsBlock().Statements.Nodes {
		if nodeKindName(statement) != "VariableStatement" {
			if loopBody := initialIterationBody(statement, names); loopBody != nil {
				statement = loopBody
				firstIterationOnly = true
			}
			// A plain binding assignment evaluates its RHS before storing it.
			// Only follow the leading callee/receiver chain, never arguments or
			// subsequent expressions, which may already observe a replacement.
			if nodeKindName(statement) == "ExpressionStatement" {
				expression := identityPreservingUnwrap(statement.Expression())
				if expression != nil && ast.IsBinaryExpression(expression) {
					assignment := expression.AsBinaryExpression()
					if assignment.OperatorToken.Kind == ast.KindEqualsToken && ast.IsIdentifier(assignment.Left) && names[assignment.Left.Text()] {
						firstReceiver = initialCallReceiver(assignment.Right)
					}
				}
			}
			break
		}
		var list *ast.Node
		statement.ForEachChild(func(child *ast.Node) bool {
			if nodeKindName(child) == "VariableDeclarationList" {
				list = child
			}
			return false
		})
		if list == nil {
			break
		}
		safe := true
		for _, declaration := range list.AsVariableDeclarationList().Declarations.Nodes {
			name := declaration.Name()
			if !ast.IsVarConst(declaration) || name == nil || !ast.IsIdentifier(name) || names[name.Text()] || !initialReadExpression(declaration.Initializer()) {
				safe = false
				break
			}
		}
		if !safe {
			break
		}
		prefixEnd = nodeLocation(statement).EndByte
	}
	var reads []typefacts.InitialParameterRead
	for _, use := range uses {
		declaration, ok := bindings[use.ParameterIndex]
		// A var redeclaration can initialize this binding before the observed
		// read. The positional store census does not include those declarators,
		// so require a unique declaration for the parameter being witnessed.
		// A redeclaration of a different parameter does not invalidate this one.
		if !ok || symbols[use.ParameterIndex] == nil || len(symbols[use.ParameterIndex].Declarations) != 1 ||
			len(use.BindingPath) != 0 || use.Kind != typefacts.ParameterUsePropertyAccess ||
			use.Captured || use.Alias || use.Location.Path != demand.Path {
			continue
		}
		inPrefix := use.Location.StartByte >= nodeLocation(body).StartByte && use.Location.EndByte <= prefixEnd
		isFirstReceiver := firstReceiver != nil && use.Location == nodeLocation(firstReceiver)
		conditional := isFirstReceiver && firstIterationOnly
		if (use.Reach == typefacts.Reachable || conditional && use.Reach == typefacts.ReachUnknown) && (inPrefix || isFirstReceiver) {
			reads = append(reads, typefacts.InitialParameterRead{ParameterIndex: use.ParameterIndex, Declaration: declaration, Use: use.Location, FirstIterationOnly: conditional})
			continue
		}
		// ADR 0069. A read this body's own stores cannot precede. The premise
		// is order, and it is about this exact use: a prefix row says some read
		// of the parameter saw the caller's value, while this one says *this*
		// read did. It is marked so, because only a consumer that binds the use
		// to its own operation may take the weaker reading. A parameter with no
		// cutoff is unwritten here, and `unwrittenParameterBindingsLocked`
		// states that stronger fact instead.
		// Reachable only. The client binds every row to exactly one census use
		// under that same filter, so a row it cannot bind is not a weaker
		// premise — it is an invalid transcript, and the export is lost rather
		// than refused.
		cutoff, written := cutoffs[use.ParameterIndex]
		if !written || use.Location.EndByte > cutoff || use.Reach != typefacts.Reachable {
			if fallback, ok := defaults[use.ParameterIndex]; ok &&
				use.Location == fallback.use && use.Reach == typefacts.Reachable {
				reads = append(reads, typefacts.InitialParameterRead{
					ParameterIndex: use.ParameterIndex, Declaration: declaration,
					Use: use.Location, UndefinedDefault: fallback.fact,
				})
			}
			continue
		}
		reads = append(reads, typefacts.InitialParameterRead{
			ParameterIndex: use.ParameterIndex,
			Declaration:    declaration,
			Use:            use.Location,
			Positional:     true,
		})
	}
	return reads
}

// Enter only one synchronous for-of, optionally guarded by a plain property
// expression. Its header cannot assign or shadow any parameter. With the
// enclosing no-closure/no-eval exclusions, external iterator code cannot mutate
// these lexical bindings. Only the first body's leading assignment is used;
// subsequent iterations expressly do not retain the caller-origin premise.
func initialIterationBody(node *ast.Node, parameters map[string]bool) *ast.Node {
	if nodeKindName(node) == "IfStatement" {
		branch := node.AsIfStatement()
		if branch.ElseStatement != nil || !initialReadExpression(branch.Expression) {
			return nil
		}
		node = branch.ThenStatement
	}
	if nodeKindName(node) != "ForOfStatement" {
		return nil
	}
	loop := node.AsForInOrOfStatement()
	if loop.AwaitModifier != nil || !initialIterationExpression(loop.Expression) || loop.Initializer == nil || nodeKindName(loop.Initializer) != "VariableDeclarationList" {
		return nil
	}
	declarations := loop.Initializer.AsVariableDeclarationList().Declarations.Nodes
	if len(declarations) != 1 || !ast.IsVarConst(declarations[0]) || declarations[0].Initializer() != nil || !initialIterationBinding(declarations[0].Name(), parameters) {
		return nil
	}
	body := loop.Statement
	if ast.IsBlock(body) {
		statements := body.AsBlock().Statements.Nodes
		if len(statements) == 0 {
			return nil
		}
		body = statements[0]
	}
	return body
}

func initialIterationBinding(node *ast.Node, parameters map[string]bool) bool {
	if node == nil {
		return false
	}
	if ast.IsIdentifier(node) {
		return !parameters[node.Text()]
	}
	if !ast.IsArrayBindingPattern(node) {
		return false
	}
	for _, element := range node.AsBindingPattern().Elements.Nodes {
		if !ast.IsBindingElement(element) || element.Initializer() != nil || element.AsBindingElement().DotDotDotToken != nil || !initialIterationBinding(element.Name(), parameters) {
			return false
		}
	}
	return true
}

func initialIterationExpression(node *ast.Node) bool {
	if initialReadExpression(node) {
		return true
	}
	node = identityPreservingUnwrap(node)
	if node == nil || !ast.IsCallExpression(node) || !initialReadExpression(node.Expression()) {
		return false
	}
	for _, argument := range node.AsCallExpression().Arguments.Nodes {
		if !initialReadExpression(argument) {
			return false
		}
	}
	return true
}

// This walk follows evaluation order only as far as the first identifier.
// Computed access, comma expressions, assignments and every other form stop it.
func initialCallReceiver(node *ast.Node) *ast.Node {
	node = identityPreservingUnwrap(node)
	if node == nil || !ast.IsCallExpression(node) {
		return nil
	}
	node = identityPreservingUnwrap(node.Expression())
	for node != nil {
		switch {
		case ast.IsIdentifier(node):
			return node
		case ast.IsPropertyAccessExpression(node), ast.IsCallExpression(node):
			node = identityPreservingUnwrap(node.Expression())
		default:
			return nil
		}
	}
	return nil
}

func initialReadExpression(node *ast.Node) bool {
	node = identityPreservingUnwrap(node)
	if node == nil {
		return false
	}
	if ast.IsIdentifier(node) {
		return true
	}
	return ast.IsPropertyAccessExpression(node) && initialReadExpression(node.Expression())
}
