package tsgo

import (
	"context"
	"path/filepath"
	"sort"

	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/microsoft/typescript-go/shim/scanner"
	"github.com/yumemi-thomas/solid-checker/internal/typefacts"
)

type asyncLocationKey struct {
	startByte int
	endByte   int
}

func asyncFactsDurable(facts []typefacts.AsyncFunctionFact) bool {
	for _, fact := range facts {
		if !durableSymbolID(fact.Symbol) || !durableSymbolID(fact.Target) {
			return false
		}
	}
	return true
}

// AsyncFunctionsAt performs the production semantic lookup: each location
// selects an inline/containing function, or follows an identifier through
// local variable aliases to its function declaration. One checker lock covers
// the complete batch, and per-location results survive unchanged generations.
func (p *project) AsyncFunctionsAt(ctx context.Context, locations []typefacts.Location) ([]typefacts.AsyncFunctionFact, error) {
	if err := ctx.Err(); err != nil {
		return nil, err
	}
	p.mu.Lock()
	defer p.mu.Unlock()
	if p.closed {
		return nil, ErrClosed
	}
	facts := make([]typefacts.AsyncFunctionFact, 0, len(locations))
	seen := make(map[asyncFactKey]struct{}, len(locations))
	for _, location := range locations {
		if err := ctx.Err(); err != nil {
			return nil, err
		}
		location.Path = filepath.Clean(location.Path)
		memo := p.memoFor(location.Path)
		key := asyncLocationKey{startByte: location.StartByte, endByte: location.EndByte}
		var selected []typefacts.AsyncFunctionFact
		if memo != nil && memo.asyncAt != nil {
			selected = memo.asyncAt[key]
		}
		if selected == nil {
			sourceFile, err := p.sourceFileFor(location)
			if err != nil {
				return nil, err
			}
			selected = p.asyncFunctionsAtLocation(location, sourceFile)
			if memo != nil && asyncFactsDurable(selected) {
				if memo.asyncAt == nil {
					memo.asyncAt = make(map[asyncLocationKey][]typefacts.AsyncFunctionFact)
				}
				memo.asyncAt[key] = append([]typefacts.AsyncFunctionFact{}, selected...)
			}
		}
		for _, fact := range selected {
			key := asyncFactKey{
				path:      filepath.Clean(fact.Expression.Path),
				startByte: fact.Expression.StartByte,
				endByte:   fact.Expression.EndByte,
				symbol:    fact.Symbol,
				target:    fact.Target,
			}
			if _, exists := seen[key]; exists {
				continue
			}
			seen[key] = struct{}{}
			fact.Expression.Path = key.path
			for index := range fact.CallsAfterAwait {
				fact.CallsAfterAwait[index].Path = filepath.Clean(fact.CallsAfterAwait[index].Path)
			}
			facts = append(facts, fact)
		}
	}
	sort.Slice(facts, func(i, j int) bool {
		left, right := facts[i], facts[j]
		if left.Expression.Path != right.Expression.Path {
			return left.Expression.Path < right.Expression.Path
		}
		if left.Expression.StartByte != right.Expression.StartByte {
			return left.Expression.StartByte < right.Expression.StartByte
		}
		if left.Expression.EndByte != right.Expression.EndByte {
			return left.Expression.EndByte < right.Expression.EndByte
		}
		if left.Symbol != right.Symbol {
			return left.Symbol < right.Symbol
		}
		return left.Target < right.Target
	})
	return facts, nil
}

type asyncFactKey struct {
	path      string
	startByte int
	endByte   int
	symbol    typefacts.SymbolID
	target    typefacts.SymbolID
}

func (p *project) asyncFunctionsAtLocation(location typefacts.Location, sourceFile *ast.SourceFile) []typefacts.AsyncFunctionFact {
	node := deepestNodeAt(ast.GetNodeAtPosition(sourceFile, location.StartByte, false), location.StartByte)
	if node == nil {
		return nil
	}
	var facts []typefacts.AsyncFunctionFact
	if ast.IsIdentifier(node) {
		p.appendAsyncSymbolFacts(&facts, p.checker.GetSymbolAtLocation(node), 0)
		return facts
	}
	for owner := node; owner != nil; owner = owner.Parent {
		if isAsyncFunctionNode(owner) {
			if fact, ok := p.asyncFunctionFact(location.Path, sourceFile, owner); ok {
				facts = append(facts, fact)
			}
			break
		}
	}
	return facts
}

func (p *project) appendAsyncSymbolFacts(facts *[]typefacts.AsyncFunctionFact, symbol *ast.Symbol, depth int) {
	if symbol == nil || depth > 32 {
		return
	}
	declaration := symbol.ValueDeclaration
	if declaration == nil {
		return
	}
	sourceFile := ast.GetSourceFileOfNode(declaration)
	if sourceFile == nil {
		return
	}
	path := filepath.Clean(sourceFile.FileName())
	if isAsyncFunctionNode(declaration) {
		if fact, ok := p.asyncFunctionFact(path, sourceFile, declaration); ok {
			*facts = append(*facts, fact)
		}
		return
	}
	if !ast.IsVariableDeclaration(declaration) {
		return
	}
	variable := declaration.AsVariableDeclaration()
	if fact, ok := p.asyncAliasFact(path, sourceFile, declaration); ok {
		*facts = append(*facts, fact)
	}
	initializer := variable.Initializer
	if initializer == nil {
		return
	}
	if isAsyncFunctionNode(initializer) {
		if fact, ok := p.asyncFunctionFact(path, sourceFile, initializer); ok {
			*facts = append(*facts, fact)
		}
		return
	}
	if ast.IsIdentifier(initializer) {
		p.appendAsyncSymbolFacts(facts, p.checker.GetSymbolAtLocation(initializer), depth+1)
	}
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
