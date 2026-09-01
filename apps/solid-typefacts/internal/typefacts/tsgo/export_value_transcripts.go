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
	signatures := p.checker.GetSignaturesOfType(valueType, checker.SignatureKindCall)
	if len(signatures) == 1 {
		declaration := p.currentSignatureDeclaration(signatures[0], target)
		if declaration != nil {
			selected := p.selectedSignatureLocked(
				signatures[0], declaration, target, typefacts.CallKindCall, demand.CallableDepth,
			)
			transcript.CallSignature = &selected
		}
	} else if len(signatures) > 1 {
		// An overload set has no single signature, and inventing one would
		// answer a different question than the one asked. Report the complete
		// set instead so a consumer can require its premise of *every*
		// overload: a claim that holds for all of them holds for the export.
		// The set is all-or-nothing. Dropping a signature whose current
		// declaration cannot be selected would silently narrow "every
		// overload" to "every overload we could describe", so the whole field
		// stays empty and the consumer's demand stays open.
		selected := make([]typefacts.SelectedSignature, 0, len(signatures))
		for _, signature := range signatures {
			declaration := p.currentSignatureDeclaration(signature, target)
			if declaration == nil {
				continue
			}
			selected = append(selected, p.selectedSignatureLocked(
				signature, declaration, target, typefacts.CallKindCall, demand.CallableDepth,
			))
		}
		transcript.CallSignatures = completeOverloadSet(selected, len(signatures))
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

// completeOverloadSet is the all-or-nothing gate on a reported overload set: it
// answers `selected` only when it describes every one of the `count` call
// signatures the type has, and nothing otherwise.
//
// The gate is a single decision rather than loop control flow on purpose. "Every
// overload" narrowing to "every overload we could describe" is a silent
// soundness loss, and a `break` that becomes a `continue` is exactly how that
// happens; here the count is what decides, and one test pins it.
func completeOverloadSet(
	selected []typefacts.SelectedSignature,
	count int,
) []typefacts.SelectedSignature {
	if len(selected) != count {
		return nil
	}
	return selected
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
	transcript.CallableReturns = p.callableReturnCensusesLocked(implementation)
	transcript.Calls = p.implementationCallCensusLocked(implementation)
	if len(transcript.ControlFlow.Unsupported) != 0 {
		transcript.OpenReasons = append(transcript.OpenReasons, "controlFlowUnsupported")
		return transcript
	}
	transcript.Complete = true
	return transcript
}

// callableReturnCensusesLocked records the return-carry edges owned by every
// nested callable in an implementation. The implementation's own returns are
// already ControlFlow; repeating them here would create two authorities for
// the first link. Each nested callable is visited exactly once even though the
// body walk descends through all of them.
func (p *project) callableReturnCensusesLocked(
	implementation *ast.Node,
) []typefacts.CallableReturnCensus {
	seen := make(map[*ast.Node]struct{})
	var callables []*ast.Node
	p.walkImplementationBodyLocked(
		implementation,
		func(node *ast.Node, _ *ast.Node, _ typefacts.Reachability) {
			if node == implementation || !isCallableDeclaration(node) || node.Body() == nil {
				return
			}
			if _, exists := seen[node]; exists {
				return
			}
			seen[node] = struct{}{}
			callables = append(callables, node)
		},
	)
	censuses := make([]typefacts.CallableReturnCensus, 0, len(callables))
	for _, callable := range callables {
		flow := p.controlFlowCensusLocked(callable)
		// A partial nested control-flow answer cannot authorize a return edge.
		// Omitting the whole callable census is the fail-closed direction because
		// absence is never read as proof that it returns nothing.
		if len(flow.Unsupported) != 0 {
			continue
		}
		returns := make([]typefacts.CallableReturnCarrySite, 0, len(flow.Returns))
		for _, site := range flow.Returns {
			var carried []typefacts.CallableCarryBinding
			if expression := returnSiteExpression(callable, site.Location); expression != nil {
				carried = p.callableReturnBindingsLocked(expression)
			}
			returns = append(returns, typefacts.CallableReturnCarrySite{
				Location:         site.Location,
				Reach:            site.Reach,
				CarryReach:       site.CarryReach,
				CarriedCallables: carried,
			})
		}
		censuses = append(censuses, typefacts.CallableReturnCensus{
			Callable: nodeLocation(callable),
			Returns:  returns,
		})
	}
	return censuses
}

// returnSiteExpression reconnects one control-flow return row to the exact
// expression that produced it. The walk stops at nested callable boundaries:
// their returns belong to their own census. A concise body is itself the
// returned expression and carries its own location in ControlFlow.
func returnSiteExpression(
	callable *ast.Node,
	location typefacts.Location,
) *ast.Node {
	body := callable.Body()
	if body == nil {
		return nil
	}
	if !ast.IsBlock(body) {
		if nodeLocation(body) == location {
			return body
		}
		return nil
	}
	var found *ast.Node
	var visit func(*ast.Node)
	visit = func(node *ast.Node) {
		if node == nil || found != nil {
			return
		}
		if node != body && isCallableDeclaration(node) {
			return
		}
		if ast.IsReturnStatement(node) && nodeLocation(node) == location {
			found = node.Expression()
			return
		}
		node.ForEachChild(func(child *ast.Node) bool {
			visit(child)
			return found != nil
		})
	}
	visit(body)
	return found
}

func (p *project) implementationCallCensusLocked(
	implementation *ast.Node,
) []typefacts.ImplementationCall {
	unsafeJumps := p.unsafeJumpRegionsLocked(implementation)
	roots := p.parameterCensusRootsLocked(implementation)
	bySymbol := make(map[*ast.Symbol]parameterCensusRoot, len(roots))
	for _, root := range roots {
		bySymbol[p.canonicalSymbol(root.symbol)] = root
	}
	var calls []typefacts.ImplementationCall
	// The walk is shared with the parameter-use census on purpose: a call and a
	// property access on the same statement must not disagree about whether
	// invoking the export runs that statement. It hands each node the innermost
	// callable containing it rather than a bare capture flag, so a consumer can
	// require each link of an execution chain to be proven instead of assuming
	// that everything inside a carried range runs.
	p.walkImplementationBodyLocked(
		implementation,
		func(node *ast.Node, enclosing *ast.Node, reach typefacts.Reachability) {
			// A construction runs what it is handed exactly as a call does —
			// `new Promise(executor)` runs its executor before it returns — so a
			// census that recorded call expressions only left every callable a
			// construction carries unreachable to the execution premise. Both
			// kinds are recorded; the kind travels with the fact so that a
			// consumer whose claim is specifically about a *call* can refuse a
			// construction rather than silently accept one.
			construct := ast.IsNewExpression(node)
			if !ast.IsCallExpression(node) && !construct {
				return
			}
			flowOwner := enclosing
			if flowOwner == nil {
				flowOwner = implementation
			}
			if reach != typefacts.Unreachable &&
				locationWithheldByJump(unsafeJumps[flowOwner], nodeLocation(node)) {
				return
			}
			kind := typefacts.CallKindCall
			if construct {
				kind = typefacts.CallKindConstruct
			}
			call := typefacts.ImplementationCall{
				Location: nodeLocation(node),
				Reach:    reach,
				Kind:     kind,
				Captured: enclosing != nil,
			}
			if enclosing != nil {
				enclosingLocation := nodeLocation(enclosing)
				call.EnclosingCallable = &enclosingLocation
			}
			// The list stays one entry per written argument, so its length
			// remains the same syntactic count every consumer already reads
			// — but a slot a spread has displaced names no parameter,
			// because the runtime value at that position is not the one
			// written there. See exactArgumentSlots.
			exact := exactArgumentSlots(node)
			for index, argument := range node.Arguments() {
				var source *typefacts.ParameterValueSource
				if index < exact {
					source = p.parameterValueSourceLocked(argument, bySymbol)
				}
				call.ArgumentParameters = append(call.ArgumentParameters, source)
			}
			call.Target, call.TargetName, call.TargetModule, call.Declaration =
				p.implementationCallTargetLocked(node.Expression())
			call.ArgumentCallables = p.argumentCallableLocationsLocked(node)
			call.DefaultLibraryInvoker, call.InvokedArguments = p.defaultLibraryInvokerLocked(node)
			if !construct {
				// Both remaining facts are claims about the body of a resolved
				// *function*: which parameter this call calls, and what the
				// callee's own body does with the parameters it is given. A
				// constructor resolves through a class's construct signatures,
				// which is a different resolution and was not reviewed here, so
				// a construct site states neither and the demand stays open.
				call.CalleeParameter = p.parameterValueSourceLocked(node.Expression(), bySymbol)
				call.CalleeDirectlyCalledParameters,
					call.CalleeInvokedParameters,
					call.CalleeStronglyInvokedParameters,
					call.CalleePendingInvocations =
					p.calleeParameterInvocationFactsLocked(node.Expression())
			}
			calls = append(calls, call)
		},
	)
	return calls
}

// callTargetIdentityLocked names the callee a call or construct expression
// resolves to: its canonical symbol, the name it is exported under, and the
// module it was imported from when it was imported at all. It is the identity
// half of implementationCallTargetLocked, factored out because the callee-body
// walk needs the identity without paying for the resolved declaration.
func (p *project) callTargetIdentityLocked(
	expression *ast.Node,
) (*ast.Symbol, string, string) {
	symbol := p.checker.GetSymbolAtLocation(expression)
	if symbol == nil {
		return nil, "", ""
	}
	targetName := symbol.Name
	targetModule, importedName := importedAliasIdentity(symbol)
	if importedName != "" {
		targetName = importedName
	}
	if !utf8.ValidString(targetName) || strings.HasPrefix(targetName, ast.InternalSymbolNamePrefix) {
		targetName = ""
	}
	return p.canonicalSymbol(symbol), targetName, targetModule
}

func (p *project) implementationCallTargetLocked(
	expression *ast.Node,
) (typefacts.SymbolID, string, string, *typefacts.ResolvedDeclaration) {
	target, targetName, targetModule := p.callTargetIdentityLocked(expression)
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
