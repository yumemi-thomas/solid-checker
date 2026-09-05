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
	if demand.LocalDeclarationLocation != nil {
		local := p.localDeclarationImplementationTranscriptLocked(
			ctx,
			*demand.LocalDeclarationLocation,
			demand.CallableDepth,
		)
		transcript.LocalDeclaration = &local
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
	transcript.CompletionForm = implementationCompletionForm(implementation)
	selected := p.selectedSignatureLocked(
		signatures[0], selectedDeclaration, target, typefacts.CallKindCall, callableDepth,
	)
	transcript.Signature = &selected
	transcript.ParameterUses = p.parameterUseCensusLocked(ctx, implementation)
	transcript.ControlFlow = p.controlFlowCensusLocked(implementation)
	transcript.CallableReturns = p.callableReturnCensusesLocked(implementation)
	transcript.Calls = p.implementationCallCensusLocked(implementation)
	transcript.UncensusedInvokingForms = p.uncensusedInvokingFormCensusLocked(implementation)
	if len(transcript.ControlFlow.Unsupported) != 0 {
		transcript.OpenReasons = append(transcript.OpenReasons, "controlFlowUnsupported")
		return transcript
	}
	transcript.Complete = true
	return transcript
}

// localDeclarationImplementationTranscriptLocked answers a demand that names a
// function-like declaration by its exact source range, rather than by an
// identifier that resolves to it.
//
// It exists because a census must recurse into module-local helpers, and
// exportImplementationTranscriptLocked cannot reach one: that path starts at an
// identifier, resolves its symbol, and takes the implementation of its single
// call signature — machinery that presupposes a binding some export names. A
// helper nothing exports has no such binding.
//
// Everything it refuses, it refuses by open reason and never by answering a
// transcript about some other declaration:
//
//   - The accepted program resolved no file at that path, or the byte range is
//     outside it or off a UTF-8 boundary: `sourceUnavailable`. This is a
//     statement about the *program*, and by itself it does not separate a file
//     the snapshot carries as runtime source from a `lib.d.ts` or a
//     dependency's declaration file, which the program also holds — hence the
//     next reason.
//   - The file is in the program but carries no runtime bytes, i.e. it is a
//     declaration file: `declarationOutsideSnapshot`. A `.d.ts` has no body to
//     census, and a census that recursed into one would be reading a
//     description of code rather than the code.
//   - No node in that file has exactly this span, or the node that does is not
//     function-like: `declarationNotExact`. Containment is not enough; the
//     span must match in both bytes.
//   - More than one function-like node has exactly this span:
//     `declarationAmbiguous`. Nothing in the grammar is known to produce that,
//     and a demand that hits it is refused rather than resolved by picking.
//   - The declaration has no body: `implementationUnavailable`.
//   - The declaration the checker resolves from the located node's own symbol
//     does not sit inside the demanded span: `declarationIdentityUnbound`. See
//     below.
//
// **What binds the answer to the demand, and what does not.** The transcript's
// Location is the requested location *verbatim*, so on its own it binds
// nothing at all: a producer answering about a different helper would echo the
// demand just the same. The binding is Declaration, whose location the checker
// derives independently — from the located declaration's own name node, via
// resolvedDeclaration — and which must name the demanded file and lie inside
// the demanded span. It is a containment rather than an equality because a
// named function's resolved location is its *identifier*, while an anonymous
// `const helper = () => …` resolves to the arrow itself. The client repeats
// this comparison, and additionally requires QueryName and the resolved
// declaration's name to agree where both are populated.
func (p *project) localDeclarationImplementationTranscriptLocked(
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
	// sourceFileFor accepts any file of the accepted program, which includes
	// every `lib.*.d.ts` and every dependency declaration file. Those are not
	// the snapshot's runtime source — the set the producer publishes as
	// Sources() is exactly the program's non-declaration files — and an
	// implementation census over a declaration file would be a census of a
	// description.
	if sourceFile.IsDeclarationFile || !p.isCurrentSourceFile(sourceFile) {
		transcript.OpenReasons = append(transcript.OpenReasons, "declarationOutsideSnapshot")
		return transcript
	}
	matches := exactFunctionLikeDeclarationsAt(sourceFile, location)
	if len(matches) == 0 {
		transcript.OpenReasons = append(transcript.OpenReasons, "declarationNotExact")
		return transcript
	}
	if len(matches) > 1 {
		transcript.OpenReasons = append(transcript.OpenReasons, "declarationAmbiguous")
		return transcript
	}
	implementation := matches[0]
	if implementation.Body() == nil {
		transcript.OpenReasons = append(transcript.OpenReasons, "implementationUnavailable")
		return transcript
	}
	// The declared name, when there is one, is what names the symbol; a
	// `const helper = () => …` carries its name on the enclosing variable
	// declaration instead, which is the one indirection taken here. An
	// anonymous callable resolves no symbol, and the transcript stays open on
	// `symbolUnresolved` rather than describing a body it cannot identify.
	name := implementation.Name()
	if name == nil {
		if parent := implementation.Parent; parent != nil && ast.IsVariableDeclaration(parent) {
			name = parent.Name()
		}
	}
	if name == nil || !ast.IsIdentifier(name) {
		transcript.OpenReasons = append(transcript.OpenReasons, "symbolUnresolved")
		return transcript
	}
	transcript.QueryName = name.Text()
	target := p.canonicalSymbol(p.checker.GetSymbolAtLocation(name))
	if target == nil {
		transcript.OpenReasons = append(transcript.OpenReasons, "symbolUnresolved")
		return transcript
	}
	transcript.Target = p.idFor(target)
	transcript.Declaration = p.resolvedDeclaration(nil, implementation, target)
	if transcript.Declaration == nil {
		transcript.OpenReasons = append(transcript.OpenReasons, "declarationUnavailable")
		return transcript
	}
	transcript.CompletionForm = implementationCompletionForm(implementation)
	// The identity binding. Location is the demand echoed back, so it proves
	// nothing by itself; this is the comparison that does, because the resolved
	// declaration's own location comes from the checker rather than from the
	// demand.
	if !locationEncloses(location, transcript.Declaration.Location) {
		transcript.OpenReasons = append(
			transcript.OpenReasons, "declarationIdentityUnbound",
		)
		return transcript
	}
	signature := p.checker.GetSignatureFromDeclaration(implementation)
	if signature == nil {
		transcript.OpenReasons = append(transcript.OpenReasons, "callSignatureNotUnique")
		return transcript
	}
	selected := p.selectedSignatureLocked(
		signature, implementation, target, typefacts.CallKindCall, callableDepth,
	)
	transcript.Signature = &selected
	transcript.ParameterUses = p.parameterUseCensusLocked(ctx, implementation)
	transcript.ControlFlow = p.controlFlowCensusLocked(implementation)
	transcript.CallableReturns = p.callableReturnCensusesLocked(implementation)
	transcript.Calls = p.implementationCallCensusLocked(implementation)
	transcript.UncensusedInvokingForms = p.uncensusedInvokingFormCensusLocked(implementation)
	if len(transcript.ControlFlow.Unsupported) != 0 {
		transcript.OpenReasons = append(transcript.OpenReasons, "controlFlowUnsupported")
		return transcript
	}
	transcript.Complete = true
	return transcript
}

// locationEncloses answers whether inner names the same file as outer and
// falls inside its byte range, endpoints included.
func locationEncloses(outer typefacts.Location, inner typefacts.Location) bool {
	return inner.Path == outer.Path &&
		outer.StartByte <= inner.StartByte && inner.EndByte <= outer.EndByte
}

// exactFunctionLikeDeclarationsAt collects every function-like declaration in
// one file whose source range is exactly the demanded one. It returns all of
// them rather than the first so that an ambiguous demand can be refused as
// ambiguous instead of silently resolved.
func exactFunctionLikeDeclarationsAt(
	sourceFile *ast.SourceFile,
	location typefacts.Location,
) []*ast.Node {
	var matches []*ast.Node
	var visit func(*ast.Node)
	visit = func(node *ast.Node) {
		if node == nil {
			return
		}
		nodeAt := nodeLocation(node)
		if nodeAt.StartByte == location.StartByte && nodeAt.EndByte == location.EndByte &&
			nodeAt.Path == location.Path && ast.IsFunctionLikeDeclaration(node) {
			matches = append(matches, node)
		}
		// A node whose range cannot contain the demanded one holds no
		// descendant that can either, so the walk prunes on containment.
		node.ForEachChild(func(child *ast.Node) bool {
			childAt := nodeLocation(child)
			if childAt.StartByte <= location.StartByte && location.EndByte <= childAt.EndByte {
				visit(child)
			}
			return false
		})
	}
	visit(sourceFile.AsNode())
	return matches
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
	subjectRoots := p.parameterSubjectRootsLocked(implementation)
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
			// A call inside a region a `break` or `continue` makes
			// non-universal is recorded with Reach `unknown`.
			//
			// **`unknown` is the sound value here, and the row's presence is
			// what makes it so.** Reachability is ordered by the strength of
			// the positive claim it licenses: `reachable` says invoking this
			// implementation runs the call on every path, `unknown` says it may
			// run it, `unreachable` says it cannot. A jump falsifies only the
			// first, so `unknown` is exactly what the producer still knows —
			// and it is the weakest non-negative value, so no consumer can read
			// more out of it than the jump left standing. A consumer needing a
			// guarantee refuses it; one at the may-execute floor admits it,
			// which is the only floor a claim about *which* callables a body can
			// reach could ever be built on.
			//
			// The row used to be **dropped** here, and the reason was sound as
			// far as it went: it kept an over-optimistic `reachable` off the
			// wire, and for a claim that some behavior *happens* a missing row
			// is the safe direction, because absence lends authority to nothing.
			// For a claim that some behavior is *absent* it is the exact failure
			// mode. A dropped call is a `CallExpression`, so it leaves no
			// uncensused-form row either: `switch (kind) { case "mount":
			// render(App, el); break; }` published nothing about `render` at all
			// beyond the enclosing construct's `switchReachability` marker, and
			// a consumer that relaxed that marker would have closed a call
			// domain over a call that runs. Dropping is now needed for neither
			// direction — `unknown` is strictly weaker than the row that was
			// withheld, so nothing sound became unsound, and the enumeration a
			// negative census needs is on the wire.
			//
			// An already-`unreachable` row is left alone. It was not the jump
			// that decided it, and downgrading it would discard a proof for
			// nothing; unsafeJumpRegionsLocked covers the whole frame whenever
			// a jump's target cannot be bound, so no `unreachable` row survives
			// a jump this census could not account for.
			if reach != typefacts.Unreachable &&
				locationWithheldByJump(unsafeJumps[flowOwner], nodeLocation(node)) {
				reach = typefacts.ReachUnknown
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
				// The value provenance of the same slot, from the same
				// tracer the return sites use. It answers a different
				// question than the parameter root above — "what created
				// this value" rather than "which parameter is it" — and a
				// displaced slot gets neither, because the runtime value at
				// that position is not the one written there.
				//
				// The empty list is written as an empty list, never as
				// nothing: the wire form of a slot is an array, and one
				// entry per written argument is the invariant a consumer
				// indexes by.
				traced := []typefacts.ImplementationValueSource{}
				if index < exact {
					source = p.parameterValueSourceLocked(argument, bySymbol)
					if found := p.returnValueSourcesLocked(argument); len(found) != 0 {
						traced = found
					}
				}
				call.ArgumentParameters = append(call.ArgumentParameters, source)
				call.ArgumentSources = append(call.ArgumentSources, traced)
			}
			call.Target, call.TargetName, call.TargetModule, call.Declaration =
				p.implementationCallTargetLocked(node.Expression())
			// ADR 0034: a `.call`/`.apply` on a default-library receiver states
			// that receiver and the parameter its `this` argument is rooted at.
			call.CallReceiver, call.ThisParameter =
				p.thisProtocolCallLocked(node, call.Declaration, subjectRoots)
			// The value provenance of the callee, from the same tracer the
			// return sites and the argument slots use. It answers a different
			// question than the resolution just above — "what created the value
			// being called" rather than "which symbol is it" — and the two
			// coexist: `read()` for `const [read] = createSignal(0)` resolves
			// to a BindingElement and traces to createSignal's tuple slot 0.
			//
			// Recorded for a construction as well. This is a value trace, not
			// the callee-parameter resolution the construct branch below
			// withholds, so nothing about a constructor's resolution is being
			// claimed; a consumer whose claim is about a *call* still checks
			// Kind first.
			call.CalleeSources = p.returnValueSourcesLocked(node.Expression())
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
		// Exactly one hop, through a binding whose value cannot have been
		// anything else, and whose slot is the slot it looks like.
		//
		// The arm used to walk every declaration of the symbol and take the
		// first array-binding one, which made `let [a] = f(); [a] = g();` and a
		// redeclared binding trace to `f()` and state it as the value's
		// provenance. Four premises replace that:
		//
		//   - exactly one declaration, so a redeclared `var` proves nothing;
		//   - the symbol is never an assignment target anywhere, answered by
		//     the checker's own assignment-target symbols rather than by
		//     reading source text. This is the whole single-assignment premise:
		//     a `const` gate was tried and reverted, because bundler output
		//     across the measured corpus (solid-js 1.9.14's own dist among
		//     them) destructures with `let`/`var` and never reassigns, and
		//     refusing those buys no soundness that this census does not
		//     already give;
		//   - no rest element. `const [...rest] = createSignal(1)` binds the
		//     *tail array*, not slot 0, and the arm traced it to slot 0;
		//   - no default. `const [a = fallback] = createSignal(2)` is `a`
		//     only when slot 0 is not `undefined`, and nothing here observes
		//     which value won;
		//   - the reference is positioned at or after the end of the binding's
		//     declaration, in the same file. `cb(hoisted); var [hoisted] =
		//     createSignal(1);` reads `undefined`, and `tsc` says nothing about
		//     it for a `var`. The bound is the whole `VariableDeclaration`, so
		//     a self-reference inside the initializer is refused too. This
		//     over-refuses a reference written earlier inside a closure that
		//     runs later, which is recorded rather than special-cased.
		//
		// The slot index counts positions among the pattern's elements, and an
		// omitted element (`const [, set] = …`) still holds its position, so
		// the count is over all elements up to this one. A rest element is
		// refused above rather than counted past.
		//
		// This can only remove sources, never add one, so it tightens
		// ReturnSite.Sources at the same time as the argument slots.
		symbol := p.canonicalSymbol(p.checker.GetSymbolAtLocation(node))
		if symbol == nil || len(symbol.Declarations) != 1 {
			return
		}
		declaration := symbol.Declarations[0]
		if !ast.IsBindingElement(declaration) || declaration.Parent == nil ||
			!ast.IsArrayBindingPattern(declaration.Parent) ||
			p.symbolIsAssignedLocked(symbol, declaration) {
			return
		}
		element := declaration.AsBindingElement()
		if element.DotDotDotToken != nil || element.Initializer != nil {
			return
		}
		pattern := declaration.Parent
		variable := pattern.Parent
		if variable == nil || !ast.IsVariableDeclaration(variable) || variable.AsVariableDeclaration().Initializer == nil ||
			!ast.IsCallExpression(variable.AsVariableDeclaration().Initializer) {
			return
		}
		reference := nodeLocation(node)
		if declared := nodeLocation(variable); reference.Path != declared.Path ||
			reference.StartByte < declared.EndByte {
			return
		}
		for index, candidate := range pattern.AsBindingPattern().Elements.Nodes {
			if candidate != declaration {
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
		if demand.LocalDeclarationLocation == nil {
			hashField(hash, "")
		} else {
			hashField(hash, demand.LocalDeclarationLocation.Path)
			hashField(hash, strconv.Itoa(demand.LocalDeclarationLocation.StartByte))
			hashField(hash, strconv.Itoa(demand.LocalDeclarationLocation.EndByte))
		}
		hashField(hash, strconv.Itoa(demand.CallableDepth))
	}
	return "sha256:" + hex.EncodeToString(hash.Sum(nil))
}

// implementationCompletionForm classifies how a function-like implementation
// completes to its caller, from its own syntax (ADR 0035): the `async`
// modifier and the asterisk token on the declaration itself. Only the four
// function-like kinds a call signature's implementation can be are reviewed;
// anything else is Unclassified, which a consumer refuses.
func implementationCompletionForm(implementation *ast.Node) typefacts.ImplementationCompletionForm {
	if implementation == nil {
		return typefacts.CompletionUnclassified
	}
	generator := false
	switch {
	case ast.IsFunctionDeclaration(implementation):
		generator = implementation.AsFunctionDeclaration().AsteriskToken != nil
	case ast.IsFunctionExpression(implementation):
		generator = implementation.AsFunctionExpression().AsteriskToken != nil
	case ast.IsMethodDeclaration(implementation):
		generator = implementation.AsMethodDeclaration().AsteriskToken != nil
	case ast.IsArrowFunction(implementation):
	default:
		return typefacts.CompletionUnclassified
	}
	async := ast.HasSyntacticModifier(implementation, ast.ModifierFlagsAsync)
	switch {
	case async && generator:
		return typefacts.CompletionAsyncGenerator
	case async:
		return typefacts.CompletionAsync
	case generator:
		return typefacts.CompletionGenerator
	default:
		return typefacts.CompletionPlain
	}
}
