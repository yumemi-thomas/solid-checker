package tsgo

import (
	"context"

	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/microsoft/typescript-go/shim/scanner"
	"github.com/yumemi-thomas/solid-check/internal/typefacts"
)

// SourceFileFacts fuses the SourceCalls, SourceBindings, SourceFunctions,
// and SourceAsyncFunctions traversals into one AST pass. Each table's
// contents and ordering are identical to the standalone walks — the per-node
// fact builders are shared — so consumers may treat the fused and standalone
// capabilities interchangeably.
func (p *project) SourceFileFacts(ctx context.Context, path string) (typefacts.FileFacts, error) {
	if err := ctx.Err(); err != nil {
		return typefacts.FileFacts{}, err
	}
	p.mu.Lock()
	defer p.mu.Unlock()
	if p.closed {
		return typefacts.FileFacts{}, ErrClosed
	}
	sourceFile, err := p.sourceFileFor(typefacts.Location{Path: path})
	if err != nil {
		return typefacts.FileFacts{}, err
	}
	facts := typefacts.FileFacts{
		Calls:          make([]typefacts.SourceCall, 0),
		Bindings:       make([]typefacts.SourceBinding, 0),
		Functions:      make([]typefacts.SourceFunction, 0),
		AsyncFunctions: make([]typefacts.AsyncFunctionFact, 0),
	}
	var visit func(*ast.Node) bool
	visit = func(node *ast.Node) bool {
		if ast.IsCallExpression(node) {
			if call, ok := p.sourceCallFact(path, sourceFile, node); ok {
				facts.Calls = append(facts.Calls, call)
				facts.Resolved = append(facts.Resolved, p.resolvedCallForNode(node))
			}
		}
		if ast.IsVariableDeclaration(node) {
			declaration := node.AsVariableDeclaration()
			if declaration.Initializer != nil && ast.IsCallExpression(declaration.Initializer) {
				if call, ok := p.sourceCallFact(path, sourceFile, declaration.Initializer); ok {
					array, names := bindingNameLocations(path, sourceFile.Text(), declaration.Name())
					facts.Bindings = append(facts.Bindings, typefacts.SourceBinding{Array: array, Names: names, Initializer: call})
				}
			}
			if ast.IsIdentifier(declaration.Name()) && declaration.Initializer != nil && ast.IsArrowFunction(declaration.Initializer) {
				arrow := declaration.Initializer.AsArrowFunction()
				if arrow.Body != nil && ast.IsBlock(arrow.Body) {
					owner := node
					for owner.Parent != nil && !ast.IsVariableStatement(owner) {
						owner = owner.Parent
					}
					function := sourceFunctionFact(path, sourceFile.Text(), declaration.Name(), arrow.Body, arrow.Parameters.Nodes, owner)
					function.Async = ast.HasSyntacticModifier(declaration.Initializer, ast.ModifierFlagsAsync)
					function.Arrow = true
					facts.Functions = append(facts.Functions, function)
				}
			}
			if ast.IsIdentifier(declaration.Name()) && declaration.Initializer != nil && ast.IsIdentifier(declaration.Initializer) {
				alias := p.checker.GetSymbolAtLocation(declaration.Name())
				target := p.checker.GetSymbolAtLocation(declaration.Initializer)
				if alias != nil && target != nil {
					facts.AsyncFunctions = append(facts.AsyncFunctions, typefacts.AsyncFunctionFact{
						Expression: typefacts.Location{Path: path, StartByte: scanner.SkipTrivia(sourceFile.Text(), declaration.Initializer.Pos()), EndByte: declaration.Initializer.End()},
						Symbol:     p.idFor(p.canonicalSymbol(alias)),
						Target:     p.idFor(p.canonicalSymbol(target)),
					})
				}
			}
		}
		if ast.IsFunctionDeclaration(node) {
			declaration := node.AsFunctionDeclaration()
			if declaration.Name() != nil && declaration.Body != nil {
				facts.Functions = append(facts.Functions, sourceFunctionFact(path, sourceFile.Text(), declaration.Name(), declaration.Body, declaration.Parameters.Nodes, node))
			}
		}
		if ast.IsArrowFunction(node) || ast.IsFunctionExpression(node) || ast.IsFunctionDeclaration(node) {
			body, symbol := asyncFunctionBodyAndSymbol(p, node)
			if body != nil {
				fact := typefacts.AsyncFunctionFact{
					Expression: typefacts.Location{Path: path, StartByte: scanner.SkipTrivia(sourceFile.Text(), node.Pos()), EndByte: node.End()},
					Symbol:     symbol, CanReturnAsync: ast.HasSyntacticModifier(node, ast.ModifierFlagsAsync),
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
				facts.AsyncFunctions = append(facts.AsyncFunctions, fact)
			}
		}
		node.ForEachChild(visit)
		return false
	}
	for _, statement := range sourceFile.Statements.Nodes {
		visit(statement)
	}
	return facts, nil
}

// resolvedCallForNode mirrors ResolvedCall with the call node already in
// hand, skipping the position lookup and parent walk. nil corresponds to the
// position-keyed method's ErrNotFound outcomes.
func (p *project) resolvedCallForNode(node *ast.Node) *typefacts.Call {
	target := p.checker.GetSymbolAtLocation(node.AsCallExpression().Expression)
	if target == nil {
		return nil
	}
	target = p.canonicalSymbol(target)
	signature := checker.Checker_getResolvedSignature(p.checker, node, nil, checker.CheckModeNormal)
	if signature == nil {
		return nil
	}
	returnType := checker.Checker_getReturnTypeOfSignature(p.checker, signature)
	if returnType == nil {
		return nil
	}
	return &typefacts.Call{
		Target:         p.idFor(target),
		ReturnType:     p.idForType(returnType),
		ReturnTypeText: p.checker.TypeToString(returnType),
	}
}
