package tsgo

import (
	"context"

	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/microsoft/typescript-go/shim/scanner"
	"github.com/yumemi-thomas/solid-check/internal/typefacts"
)

func asyncFactsDurable(facts []typefacts.AsyncFunctionFact) bool {
	for _, fact := range facts {
		if !durableSymbolID(fact.Symbol) || !durableSymbolID(fact.Target) {
			return false
		}
	}
	return true
}

func (p *project) SourceAsyncFunctions(ctx context.Context, path string) ([]typefacts.AsyncFunctionFact, error) {
	if err := ctx.Err(); err != nil {
		return nil, err
	}
	p.mu.Lock()
	defer p.mu.Unlock()
	if p.closed {
		return nil, ErrClosed
	}
	memo := p.memoFor(path)
	if memo != nil && memo.hasAsync {
		return append([]typefacts.AsyncFunctionFact(nil), memo.async...), nil
	}
	sourceFile, err := p.sourceFileFor(typefacts.Location{Path: path})
	if err != nil {
		return nil, err
	}
	facts := make([]typefacts.AsyncFunctionFact, 0)
	var visit func(*ast.Node) bool
	visit = func(node *ast.Node) bool {
		if isAsyncFunctionNode(node) {
			if fact, ok := p.asyncFunctionFact(path, sourceFile, node); ok {
				facts = append(facts, fact)
			}
		} else if ast.IsVariableDeclaration(node) {
			if fact, ok := p.asyncAliasFact(path, sourceFile, node); ok {
				facts = append(facts, fact)
			}
		}
		node.ForEachChild(visit)
		return false
	}
	for _, statement := range sourceFile.Statements.Nodes {
		visit(statement)
	}
	if memo != nil && asyncFactsDurable(facts) {
		memo.async = facts
		memo.hasAsync = true
	}
	return facts, nil
}

func isAsyncFunctionNode(node *ast.Node) bool {
	return ast.IsArrowFunction(node) ||
		ast.IsFunctionExpression(node) ||
		ast.IsFunctionDeclaration(node) ||
		ast.IsMethodDeclaration(node)
}

func (p *project) asyncFunctionFact(path string, sourceFile *ast.SourceFile, node *ast.Node) (typefacts.AsyncFunctionFact, bool) {
	body, symbol := asyncFunctionBodyAndSymbol(p, node)
	if body == nil {
		return typefacts.AsyncFunctionFact{}, false
	}
	fact := typefacts.AsyncFunctionFact{
		Expression:     typefacts.Location{Path: path, StartByte: scanner.SkipTrivia(sourceFile.Text(), node.Pos()), EndByte: node.End()},
		Symbol:         symbol,
		CanReturnAsync: ast.HasSyntacticModifier(node, ast.ModifierFlagsAsync),
	}
	if !fact.CanReturnAsync {
		functionType := p.checker.GetTypeAtLocation(node)
		for _, signature := range p.checker.GetSignaturesOfType(functionType, checker.SignatureKindCall) {
			if asyncReturnType(p.checker, p.checker.GetReturnTypeOfSignature(signature)) {
				fact.CanReturnAsync = true
				break
			}
		}
	}
	state := asyncFlowState{reachable: true}
	scanAsyncFlow(p, path, sourceFile, body, body, &state, &fact.CallsAfterAwait)
	return fact, true
}

func (p *project) asyncAliasFact(path string, sourceFile *ast.SourceFile, node *ast.Node) (typefacts.AsyncFunctionFact, bool) {
	declaration := node.AsVariableDeclaration()
	if !ast.IsIdentifier(declaration.Name()) ||
		declaration.Initializer == nil ||
		!ast.IsIdentifier(declaration.Initializer) {
		return typefacts.AsyncFunctionFact{}, false
	}
	alias := p.checker.GetSymbolAtLocation(declaration.Name())
	target := p.checker.GetSymbolAtLocation(declaration.Initializer)
	if alias == nil || target == nil {
		return typefacts.AsyncFunctionFact{}, false
	}
	return typefacts.AsyncFunctionFact{
		Expression: typefacts.Location{
			Path:      path,
			StartByte: scanner.SkipTrivia(sourceFile.Text(), declaration.Initializer.Pos()),
			EndByte:   declaration.Initializer.End(),
		},
		Symbol: p.idFor(p.canonicalSymbol(alias)),
		Target: p.idFor(p.canonicalSymbol(target)),
	}, true
}

func asyncFunctionBodyAndSymbol(p *project, node *ast.Node) (*ast.Node, typefacts.SymbolID) {
	var body, name *ast.Node
	switch {
	case ast.IsArrowFunction(node):
		body = node.AsArrowFunction().Body
		if parent := node.Parent; parent != nil && ast.IsVariableDeclaration(parent) {
			name = parent.AsVariableDeclaration().Name()
		}
	case ast.IsFunctionExpression(node):
		body = node.AsFunctionExpression().Body
	case ast.IsFunctionDeclaration(node):
		body = node.AsFunctionDeclaration().Body
		name = node.AsFunctionDeclaration().Name()
	case ast.IsMethodDeclaration(node):
		body = node.AsMethodDeclaration().Body
		name = node.AsMethodDeclaration().Name()
	}
	if name != nil {
		if symbol := p.checker.GetSymbolAtLocation(name); symbol != nil {
			return body, p.idFor(p.canonicalSymbol(symbol))
		}
	}
	return body, ""
}

func asyncReturnType(typeChecker *checker.Checker, returnType *checker.Type) bool {
	if returnType == nil {
		return false
	}
	if returnType.IsUnion() || returnType.IsIntersection() {
		for _, constituent := range returnType.Types() {
			if asyncReturnType(typeChecker, constituent) {
				return true
			}
		}
		return false
	}
	if returnType.IsTypeParameter() {
		constraint := checker.Checker_getBaseConstraintOfType(typeChecker, returnType)
		return constraint != nil && constraint != returnType && asyncReturnType(typeChecker, constraint)
	}
	if awaited := checker.Checker_getAwaitedType(typeChecker, returnType); awaited != nil && !checker.Checker_isTypeIdenticalTo(typeChecker, returnType, awaited) {
		return true
	}
	if symbol := checker.Type_symbol(returnType); symbol != nil {
		if symbol.Name == "AsyncIterable" || symbol.Name == "AsyncIterator" {
			return true
		}
	}
	for _, base := range checker.Checker_getBaseTypes(typeChecker, returnType) {
		if base != returnType && asyncReturnType(typeChecker, base) {
			return true
		}
	}
	return false
}

type asyncFlowState struct {
	awaited   bool
	reachable bool
}

func scanAsyncFlow(p *project, path string, sourceFile *ast.SourceFile, root, node *ast.Node, state *asyncFlowState, calls *[]typefacts.Location) {
	if node == nil || !state.reachable {
		return
	}
	if node != root && (ast.IsArrowFunction(node) || ast.IsFunctionExpression(node) || ast.IsFunctionDeclaration(node) || ast.IsMethodDeclaration(node)) {
		return
	}
	if ast.IsBlock(node) {
		for _, statement := range node.AsBlock().Statements.Nodes {
			scanAsyncFlow(p, path, sourceFile, root, statement, state, calls)
			if !state.reachable {
				break
			}
		}
		return
	}
	if ast.IsIfStatement(node) {
		statement := node.AsIfStatement()
		scanAsyncFlow(p, path, sourceFile, root, statement.Expression, state, calls)
		thenState, elseState := *state, *state
		scanAsyncFlow(p, path, sourceFile, root, statement.ThenStatement, &thenState, calls)
		if statement.ElseStatement != nil {
			scanAsyncFlow(p, path, sourceFile, root, statement.ElseStatement, &elseState, calls)
		}
		*state = mergeAsyncBranches(thenState, elseState)
		return
	}
	if ast.IsConditionalExpression(node) {
		expression := node.AsConditionalExpression()
		scanAsyncFlow(p, path, sourceFile, root, expression.Condition, state, calls)
		trueState, falseState := *state, *state
		scanAsyncFlow(p, path, sourceFile, root, expression.WhenTrue, &trueState, calls)
		scanAsyncFlow(p, path, sourceFile, root, expression.WhenFalse, &falseState, calls)
		*state = mergeAsyncBranches(trueState, falseState)
		return
	}
	if ast.IsBinaryExpression(node) {
		expression := node.AsBinaryExpression()
		if expression.OperatorToken.Kind == ast.KindAmpersandAmpersandToken ||
			expression.OperatorToken.Kind == ast.KindBarBarToken ||
			expression.OperatorToken.Kind == ast.KindQuestionQuestionToken {
			scanAsyncFlow(p, path, sourceFile, root, expression.Left, state, calls)
			rightState := *state
			scanAsyncFlow(p, path, sourceFile, root, expression.Right, &rightState, calls)
			*state = mergeAsyncBranches(*state, rightState)
			return
		}
	}
	if ast.IsSwitchStatement(node) {
		scanAsyncSwitchFlow(p, path, sourceFile, root, node, state, calls)
		return
	}
	if ast.IsTryStatement(node) {
		statement := node.AsTryStatement()
		tryState, catchState := *state, *state
		scanAsyncFlow(p, path, sourceFile, root, statement.TryBlock, &tryState, calls)
		if statement.CatchClause != nil {
			scanAsyncFlow(p, path, sourceFile, root, statement.CatchClause.AsCatchClause().Block, &catchState, calls)
		} else {
			catchState = tryState
		}
		*state = mergeAsyncBranches(tryState, catchState)
		if statement.FinallyBlock != nil {
			scanAsyncFlow(p, path, sourceFile, root, statement.FinallyBlock, state, calls)
		}
		return
	}
	if ast.IsIterationStatement(node, true) {
		entry := *state
		loopState := entry
		node.ForEachChild(func(child *ast.Node) bool {
			scanAsyncFlow(p, path, sourceFile, root, child, &loopState, calls)
			return false
		})
		state.awaited, state.reachable = entry.awaited, entry.reachable
		return
	}
	if ast.IsAwaitExpression(node) {
		node.ForEachChild(func(child *ast.Node) bool {
			scanAsyncFlow(p, path, sourceFile, root, child, state, calls)
			return false
		})
		state.awaited = true
		return
	}
	if ast.IsCallExpression(node) && state.awaited {
		if call, ok := p.sourceCallFact(path, sourceFile, node); ok {
			*calls = append(*calls, call.Callee)
		}
	}
	if ast.IsReturnStatement(node) || ast.IsThrowStatement(node) {
		node.ForEachChild(func(child *ast.Node) bool {
			scanAsyncFlow(p, path, sourceFile, root, child, state, calls)
			return false
		})
		state.reachable = false
		return
	}
	node.ForEachChild(func(child *ast.Node) bool {
		scanAsyncFlow(p, path, sourceFile, root, child, state, calls)
		return false
	})
}

func scanAsyncSwitchFlow(p *project, path string, sourceFile *ast.SourceFile, root, node *ast.Node, state *asyncFlowState, calls *[]typefacts.Location) {
	statement := node.AsSwitchStatement()
	scanAsyncFlow(p, path, sourceFile, root, statement.Expression, state, calls)
	entry := *state
	var flowThrough *asyncFlowState
	exits := make([]asyncFlowState, 0, len(statement.CaseBlock.AsCaseBlock().Clauses.Nodes)+1)
	hasDefault := false
	for _, clauseNode := range statement.CaseBlock.AsCaseBlock().Clauses.Nodes {
		clause := clauseNode.AsCaseOrDefaultClause()
		if clauseNode.Kind == ast.KindDefaultClause {
			hasDefault = true
		} else if clause.Expression != nil {
			// A case expression is conditionally evaluated while matching. Preserve calls
			// already dominated before the switch, but do not let an await in one case
			// expression dominate a sibling case or its body.
			caseState := entry
			scanAsyncFlow(p, path, sourceFile, root, clause.Expression, &caseState, calls)
		}
		clauseState := entry
		if flowThrough != nil {
			clauseState = mergeAsyncBranches(clauseState, *flowThrough)
		}
		broke := false
		for _, child := range clause.Statements.Nodes {
			if ast.IsBreakStatement(child) {
				exits = append(exits, clauseState)
				broke = true
				break
			}
			scanAsyncFlow(p, path, sourceFile, root, child, &clauseState, calls)
			if !clauseState.reachable {
				break
			}
		}
		if broke || !clauseState.reachable {
			flowThrough = nil
		} else {
			next := clauseState
			flowThrough = &next
		}
	}
	if flowThrough != nil {
		exits = append(exits, *flowThrough)
	}
	if !hasDefault {
		exits = append(exits, entry)
	}
	if len(exits) == 0 {
		state.reachable = false
		return
	}
	merged := exits[0]
	for _, exit := range exits[1:] {
		merged = mergeAsyncBranches(merged, exit)
	}
	*state = merged
}

func mergeAsyncBranches(left, right asyncFlowState) asyncFlowState {
	if !left.reachable {
		return right
	}
	if !right.reachable {
		return left
	}
	return asyncFlowState{reachable: true, awaited: left.awaited && right.awaited}
}
