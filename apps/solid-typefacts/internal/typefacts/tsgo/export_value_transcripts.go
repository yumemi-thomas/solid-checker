package tsgo

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"strconv"
	"strings"
	"unicode/utf8"

	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts"
	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/wirecbor"
)

var _ typefacts.ExportValueAnalyzer = (*project)(nil)

func (p *project) ExportValueTranscripts(
	ctx context.Context,
	demands []typefacts.ExportValueDemand,
) (typefacts.ExportValueAnswer, error) {
	if err := ctx.Err(); err != nil {
		return typefacts.ExportValueAnswer{}, err
	}
	for _, demand := range demands {
		if demand.CallableDepth < 0 || demand.CallableDepth > typefacts.MaxInvocationCallableDepth {
			return typefacts.ExportValueAnswer{}, fmt.Errorf(
				"export-value callable depth %d exceeds limit %d",
				demand.CallableDepth,
				typefacts.MaxInvocationCallableDepth,
			)
		}
	}
	p.mu.Lock()
	defer p.mu.Unlock()
	if p.closed {
		return typefacts.ExportValueAnswer{}, ErrClosed
	}

	answer := typefacts.ExportValueAnswer{
		Transcripts: make([]typefacts.ExportValueTranscript, len(demands)),
		Envelope: typefacts.InvocationEnvelope{
			Generation:   p.generation,
			DemandSHA256: exportValueDemandDigest(demands),
		},
	}
	for index, demand := range demands {
		if err := ctx.Err(); err != nil {
			return typefacts.ExportValueAnswer{}, err
		}
		answer.Transcripts[index] = p.exportValueTranscriptLocked(ctx, demand)
	}

	inventory, err := p.moduleGraphLocked(ctx, typefacts.ModuleInventoryDemand{Imports: true})
	if err != nil {
		return typefacts.ExportValueAnswer{}, err
	}
	encodedGraph, err := wirecbor.Marshal(inventory)
	if err != nil {
		return typefacts.ExportValueAnswer{}, fmt.Errorf("encode export-value module graph: %w", err)
	}
	answer.Envelope.ModuleGraphSHA256 = sha256String(encodedGraph)
	answer.Envelope.Sources = p.invocationSourceDigestsLocked()
	for _, unresolved := range inventory.Imports {
		if unresolved.ResolvedPath == "" {
			answer.Envelope.OpenReasons = append(answer.Envelope.OpenReasons, "unresolvedModule")
			break
		}
	}
	return answer, nil
}

func (p *project) exportValueTranscriptLocked(
	ctx context.Context,
	demand typefacts.ExportValueDemand,
) typefacts.ExportValueTranscript {
	// Keep every early-refusal transcript wire-valid. Callability is a closed
	// string enum on the ordinary CBOR protocol, so its Go zero value is not a
	// serializable verdict. Selection/identity failures still carry an explicit
	// unknown value domain; the outer open reason says why no value was acquired.
	transcript := typefacts.ExportValueTranscript{
		Location: demand.Location,
		Value: typefacts.InvocationValueFact{
			Callability:      typefacts.CallabilityUnknown,
			Constructability: typefacts.InvocationConstructUnknown,
			Primitive:        typefacts.ValuePrimitiveDomain{Unknown: true},
			OpenReasons:      []string{"valueUnavailable"},
		},
	}
	sourceFile, err := p.sourceFileFor(demand.Location)
	if err != nil {
		transcript.OpenReasons = append(transcript.OpenReasons, "sourceUnavailable")
		return transcript
	}
	cursor := semanticNodeCursor{sourceFile: sourceFile}
	node := cursor.exactExpressionAt(demand.Location.StartByte, demand.Location.EndByte)
	if node == nil || !ast.IsIdentifier(node) {
		transcript.OpenReasons = append(transcript.OpenReasons, "identifierNotExact")
		return transcript
	}
	transcript.QueryName = node.Text()
	alias := p.checker.GetSymbolAtLocation(node)
	if alias == nil {
		transcript.OpenReasons = append(transcript.OpenReasons, "symbolUnresolved")
		return transcript
	}
	target := p.canonicalSymbol(alias)
	if target == nil {
		transcript.OpenReasons = append(transcript.OpenReasons, "aliasUnresolved")
		return transcript
	}
	transcript.Target = p.idFor(target)
	declaration := target.ValueDeclaration
	if declaration == nil && len(target.Declarations) != 0 {
		declaration = target.Declarations[0]
	}
	if declaration == nil {
		transcript.OpenReasons = append(transcript.OpenReasons, "declarationUnavailable")
		return transcript
	}
	transcript.Declaration = p.resolvedDeclaration(nil, declaration, target)
	if transcript.Declaration == nil {
		transcript.OpenReasons = append(transcript.OpenReasons, "declarationUnavailable")
		return transcript
	}
	valueType := p.checker.GetTypeAtLocation(node)
	transcript.Value = p.invocationValueFactLocked(valueType)
	transcript.CallablePaths = p.callablePathsLocked(valueType, demand.CallableDepth)
	if signatures := p.checker.GetSignaturesOfType(valueType, checker.SignatureKindCall); len(signatures) == 1 {
		declaration := p.currentSignatureDeclaration(signatures[0], target)
		if declaration != nil {
			selected := p.selectedSignatureLocked(
				signatures[0], declaration, target, typefacts.CallKindCall, demand.CallableDepth,
			)
			transcript.CallSignature = &selected
		}
	}
	if demand.ImplementationLocation != nil {
		implementation := p.exportImplementationTranscriptLocked(
			ctx,
			*demand.ImplementationLocation,
			demand.CallableDepth,
		)
		transcript.Implementation = &implementation
	}
	transcript.Complete = true
	return transcript
}

func (p *project) exportImplementationTranscriptLocked(
	ctx context.Context,
	location typefacts.Location,
	callableDepth int,
) typefacts.ExportImplementationTranscript {
	transcript := typefacts.ExportImplementationTranscript{Location: location}
	sourceFile, err := p.sourceFileFor(location)
	if err != nil {
		transcript.OpenReasons = append(transcript.OpenReasons, "sourceUnavailable")
		return transcript
	}
	cursor := semanticNodeCursor{sourceFile: sourceFile}
	node := cursor.exactExpressionAt(location.StartByte, location.EndByte)
	if node == nil || !ast.IsIdentifier(node) {
		transcript.OpenReasons = append(transcript.OpenReasons, "identifierNotExact")
		return transcript
	}
	transcript.QueryName = node.Text()
	symbol := p.checker.GetSymbolAtLocation(node)
	if symbol == nil {
		transcript.OpenReasons = append(transcript.OpenReasons, "symbolUnresolved")
		return transcript
	}
	target := p.canonicalSymbol(symbol)
	if target == nil {
		transcript.OpenReasons = append(transcript.OpenReasons, "aliasUnresolved")
		return transcript
	}
	transcript.Target = p.idFor(target)
	valueType := p.checker.GetTypeAtLocation(node)
	signatures := p.checker.GetSignaturesOfType(valueType, checker.SignatureKindCall)
	if len(signatures) != 1 {
		transcript.OpenReasons = append(transcript.OpenReasons, "callSignatureNotUnique")
		return transcript
	}
	selectedDeclaration := p.currentSignatureDeclaration(signatures[0], target)
	implementation := invocationImplementationDeclaration(selectedDeclaration, target)
	if selectedDeclaration == nil || implementation == nil || implementation.Body() == nil {
		transcript.OpenReasons = append(transcript.OpenReasons, "implementationUnavailable")
		return transcript
	}
	transcript.Declaration = p.resolvedDeclaration(nil, implementation, target)
	if transcript.Declaration == nil {
		transcript.OpenReasons = append(transcript.OpenReasons, "declarationUnavailable")
		return transcript
	}
	selected := p.selectedSignatureLocked(
		signatures[0], selectedDeclaration, target, typefacts.CallKindCall, callableDepth,
	)
	transcript.Signature = &selected
	transcript.ParameterUses = p.parameterUseCensusLocked(ctx, implementation)
	transcript.ControlFlow = p.controlFlowCensusLocked(implementation)
	transcript.Calls = p.implementationCallCensusLocked(implementation)
	if len(transcript.ControlFlow.Unsupported) != 0 {
		transcript.OpenReasons = append(transcript.OpenReasons, "controlFlowUnsupported")
		return transcript
	}
	transcript.Complete = true
	return transcript
}

func (p *project) implementationCallCensusLocked(
	implementation *ast.Node,
) []typefacts.ImplementationCall {
	roots := p.parameterCensusRootsLocked(implementation)
	bySymbol := make(map[*ast.Symbol]parameterCensusRoot, len(roots))
	for _, root := range roots {
		bySymbol[p.canonicalSymbol(root.symbol)] = root
	}
	var calls []typefacts.ImplementationCall
	var visit func(*ast.Node, bool, typefacts.Reachability) typefacts.Reachability
	visit = func(node *ast.Node, captured bool, reach typefacts.Reachability) typefacts.Reachability {
		if node == nil {
			return reach
		}
		nested := captured || node != implementation.Body() && isCallableDeclaration(node)
		if ast.IsBlock(node) {
			current := reach
			for _, statement := range node.AsBlock().Statements.Nodes {
				current = visit(statement, nested, current)
			}
			return current
		}
		if ast.IsIfStatement(node) {
			statement := node.AsIfStatement()
			visit(statement.Expression, nested, reach)
			thenReach := visit(statement.ThenStatement, nested, reach)
			elseReach := reach
			if statement.ElseStatement != nil {
				elseReach = visit(statement.ElseStatement, nested, reach)
			}
			return mergeReachability(thenReach, elseReach)
		}
		if ast.IsTryStatement(node) {
			statement := node.AsTryStatement()
			tryReach := visit(statement.TryBlock, nested, reach)
			catchReach := typefacts.Unreachable
			if statement.CatchClause != nil {
				catchReach = visit(statement.CatchClause.AsCatchClause().Block, nested, reach)
			}
			merged := mergeReachability(tryReach, catchReach)
			if statement.FinallyBlock != nil {
				visit(statement.FinallyBlock, nested, reach)
			}
			return merged
		}
		if ast.IsIterationStatement(node, true) || ast.IsSwitchStatement(node) {
			node.ForEachChild(func(child *ast.Node) bool {
				visit(child, nested, typefacts.ReachUnknown)
				return false
			})
			return typefacts.ReachUnknown
		}
		if ast.IsCallExpression(node) {
			call := typefacts.ImplementationCall{
				Location: nodeLocation(node),
				Reach:    reach,
				Captured: nested,
			}
			call.CalleeParameter = p.parameterValueSourceLocked(node.Expression(), bySymbol)
			for _, argument := range node.Arguments() {
				call.ArgumentParameters = append(
					call.ArgumentParameters,
					p.parameterValueSourceLocked(argument, bySymbol),
				)
			}
			call.Target, call.TargetName, call.TargetModule, call.Declaration =
				p.implementationCallTargetLocked(node.Expression())
			calls = append(calls, call)
		}
		terminates := ast.IsReturnStatement(node) || ast.IsThrowStatement(node)
		node.ForEachChild(func(child *ast.Node) bool {
			visit(child, nested, reach)
			return false
		})
		if terminates {
			return typefacts.Unreachable
		}
		return reach
	}
	visit(implementation.Body(), false, typefacts.Reachable)
	return calls
}

func (p *project) implementationCallTargetLocked(
	expression *ast.Node,
) (typefacts.SymbolID, string, string, *typefacts.ResolvedDeclaration) {
	symbol := p.checker.GetSymbolAtLocation(expression)
	if symbol == nil {
		return "", "", "", nil
	}
	targetName := symbol.Name
	targetModule, importedName := importedAliasIdentity(symbol)
	if importedName != "" {
		targetName = importedName
	}
	if !utf8.ValidString(targetName) || strings.HasPrefix(targetName, ast.InternalSymbolNamePrefix) {
		targetName = ""
	}
	target := p.canonicalSymbol(symbol)
	if target == nil {
		return "", targetName, targetModule, nil
	}
	var resolved *typefacts.ResolvedDeclaration
	declaration := target.ValueDeclaration
	if declaration == nil && len(target.Declarations) != 0 {
		declaration = target.Declarations[0]
	}
	if declaration != nil {
		resolved = p.resolvedDeclaration(nil, declaration, target)
	}
	return p.idFor(target), targetName, targetModule, resolved
}

func importedAliasIdentity(symbol *ast.Symbol) (string, string) {
	if symbol == nil || symbol.Flags&ast.SymbolFlagsAlias == 0 {
		return "", ""
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
		default:
			continue
		}
		owner := declaration.Parent
		for owner != nil && !ast.IsImportDeclaration(owner) {
			owner = owner.Parent
		}
		if owner == nil || owner.AsImportDeclaration().ModuleSpecifier == nil {
			continue
		}
		return owner.AsImportDeclaration().ModuleSpecifier.Text(), importedName
	}
	return "", ""
}

func (p *project) returnValueSourcesLocked(expression *ast.Node) []typefacts.ImplementationValueSource {
	var sources []typefacts.ImplementationValueSource
	var walk func(*ast.Node, []typefacts.PathSegment)
	walk = func(node *ast.Node, path []typefacts.PathSegment) {
		for node != nil && ast.IsParenthesizedExpression(node) {
			node = node.AsParenthesizedExpression().Expression
		}
		if node == nil {
			return
		}
		if ast.IsArrayLiteralExpression(node) {
			for index, element := range node.AsArrayLiteralExpression().Elements.Nodes {
				item := index
				walk(element, append(path, typefacts.PathSegment{Kind: typefacts.PathSegmentTuple, Index: &item}))
			}
			return
		}
		if ast.IsArrowFunction(node) || ast.IsFunctionExpression(node) {
			sources = append(sources, typefacts.ImplementationValueSource{
				Path: append([]typefacts.PathSegment(nil), path...), Kind: typefacts.ImplementationValueDirectCallable,
			})
			return
		}
		if ast.IsCallExpression(node) {
			target, name, module, _ := p.implementationCallTargetLocked(node.Expression())
			if target != "" {
				sources = append(sources, typefacts.ImplementationValueSource{
					Path: append([]typefacts.PathSegment(nil), path...), Kind: typefacts.ImplementationValueCallResult,
					Target: target, TargetName: name, TargetModule: module,
				})
			}
			return
		}
		if !ast.IsIdentifier(node) {
			return
		}
		symbol := p.checker.GetSymbolAtLocation(node)
		if symbol == nil {
			return
		}
		for _, declaration := range symbol.Declarations {
			if ast.IsBindingElement(declaration) && declaration.Parent != nil && ast.IsArrayBindingPattern(declaration.Parent) {
				pattern := declaration.Parent
				variable := pattern.Parent
				if variable == nil || !ast.IsVariableDeclaration(variable) || variable.AsVariableDeclaration().Initializer == nil ||
					!ast.IsCallExpression(variable.AsVariableDeclaration().Initializer) {
					continue
				}
				for index, element := range pattern.AsBindingPattern().Elements.Nodes {
					if element != declaration {
						continue
					}
					target, name, module, _ := p.implementationCallTargetLocked(variable.AsVariableDeclaration().Initializer.Expression())
					if target == "" {
						return
					}
					item := index
					sources = append(sources, typefacts.ImplementationValueSource{
						Path: append([]typefacts.PathSegment(nil), path...), Kind: typefacts.ImplementationValueCallResult,
						Target: target, TargetName: name, TargetModule: module,
						TargetPath: []typefacts.PathSegment{{Kind: typefacts.PathSegmentTuple, Index: &item}},
					})
					return
				}
			}
		}
	}
	walk(expression, nil)
	return sources
}

func (p *project) parameterValueSourceLocked(
	node *ast.Node,
	bySymbol map[*ast.Symbol]parameterCensusRoot,
) *typefacts.ParameterValueSource {
	for node != nil && ast.IsParenthesizedExpression(node) {
		node = node.AsParenthesizedExpression().Expression
	}
	if node == nil {
		return nil
	}
	if ast.IsIdentifier(node) {
		symbol := p.canonicalSymbol(p.checker.GetSymbolAtLocation(node))
		root, ok := bySymbol[symbol]
		if !ok {
			return nil
		}
		return &typefacts.ParameterValueSource{
			ParameterIndex: root.index,
			Path:           append([]typefacts.PathSegment(nil), root.path...),
		}
	}
	if ast.IsPropertyAccessExpression(node) && node.Name() != nil {
		source := p.parameterValueSourceLocked(node.Expression(), bySymbol)
		if source == nil {
			return nil
		}
		source.Path = append(source.Path, typefacts.PathSegment{
			Kind:     typefacts.PathSegmentProperty,
			Property: node.Name().Text(),
		})
		return source
	}
	return nil
}

func exportValueDemandDigest(demands []typefacts.ExportValueDemand) string {
	hash := sha256.New()
	hashField(hash, "solid-checker:typefacts:export-values:v1")
	for _, demand := range demands {
		hashField(hash, demand.Location.Path)
		hashField(hash, strconv.Itoa(demand.Location.StartByte))
		hashField(hash, strconv.Itoa(demand.Location.EndByte))
		if demand.ImplementationLocation == nil {
			hashField(hash, "")
		} else {
			hashField(hash, demand.ImplementationLocation.Path)
			hashField(hash, strconv.Itoa(demand.ImplementationLocation.StartByte))
			hashField(hash, strconv.Itoa(demand.ImplementationLocation.EndByte))
		}
		hashField(hash, strconv.Itoa(demand.CallableDepth))
	}
	return "sha256:" + hex.EncodeToString(hash.Sum(nil))
}
