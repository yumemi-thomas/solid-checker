package tsgo

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"path/filepath"
	"sort"
	"strconv"
	"strings"
	"unicode/utf8"

	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/microsoft/typescript-go/shim/scanner"
	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts"
	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/wirecbor"
)

var _ typefacts.InvocationAnalyzer = (*project)(nil)

func (p *project) InvocationTranscripts(
	ctx context.Context,
	demands []typefacts.InvocationDemand,
) (typefacts.InvocationAnswer, error) {
	if err := ctx.Err(); err != nil {
		return typefacts.InvocationAnswer{}, err
	}
	for _, demand := range demands {
		if demand.CallableDepth < 0 || demand.CallableDepth > typefacts.MaxInvocationCallableDepth {
			return typefacts.InvocationAnswer{}, fmt.Errorf(
				"invocation callable depth %d exceeds limit %d",
				demand.CallableDepth,
				typefacts.MaxInvocationCallableDepth,
			)
		}
	}
	p.mu.Lock()
	defer p.mu.Unlock()
	if p.closed {
		return typefacts.InvocationAnswer{}, ErrClosed
	}

	answer := typefacts.InvocationAnswer{
		Transcripts: make([]typefacts.InvocationTranscript, len(demands)),
		Envelope: typefacts.InvocationEnvelope{
			Generation:   p.generation,
			DemandSHA256: invocationDemandDigest(demands),
		},
	}
	for index, demand := range demands {
		if err := ctx.Err(); err != nil {
			return typefacts.InvocationAnswer{}, err
		}
		answer.Transcripts[index] = p.invocationTranscriptLocked(ctx, demand)
		if err := ctx.Err(); err != nil {
			return typefacts.InvocationAnswer{}, err
		}
	}

	inventory, err := p.moduleGraphLocked(ctx, typefacts.ModuleInventoryDemand{Imports: true})
	if err != nil {
		return typefacts.InvocationAnswer{}, err
	}
	encodedGraph, err := wirecbor.Marshal(inventory)
	if err != nil {
		return typefacts.InvocationAnswer{}, fmt.Errorf("encode invocation module graph: %w", err)
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

func (p *project) invocationTranscriptLocked(
	ctx context.Context,
	demand typefacts.InvocationDemand,
) typefacts.InvocationTranscript {
	transcript := typefacts.InvocationTranscript{
		Location: demand.Location,
		Validity: typefacts.ResolvedCallUnresolved,
		Kind:     typefacts.CallKindUnknown,
	}
	sourceFile, err := p.sourceFileFor(demand.Location)
	if err != nil {
		transcript.OpenReasons = append(transcript.OpenReasons, "sourceUnavailable")
		return transcript
	}
	cursor := semanticNodeCursor{sourceFile: sourceFile}
	node := cursor.exactCallLikeAt(demand.Location.StartByte, demand.Location.EndByte)
	if node == nil {
		transcript.OpenReasons = append(transcript.OpenReasons, "callNotExact")
		return transcript
	}
	if ast.IsNewExpression(node) {
		transcript.Kind = typefacts.CallKindConstruct
	} else {
		transcript.Kind = typefacts.CallKindCall
	}
	if invocationUsesFunctionPrototypeIndirection(node) {
		// The selected signature belongs to Function.call/apply/bind, not the
		// receiver. Exposing it would certify the wrong target and bindings.
		transcript.OpenReasons = append(transcript.OpenReasons, "indirectFunctionInvocation")
		return transcript
	}
	target := p.checker.GetSymbolAtLocation(node.Expression())
	if target != nil {
		target = p.canonicalSymbol(target)
		transcript.Target = p.idFor(target)
	}
	signature := checker.Checker_getResolvedSignature(p.checker, node, nil, checker.CheckModeNormal)
	if signature == nil {
		transcript.OpenReasons = append(transcript.OpenReasons, "signatureUnresolved")
		return transcript
	}
	validity, calleeType, err := p.resolvedCallValidityAndCalleeTypeLocked(
		ctx, sourceFile, node, signature, target,
	)
	if err != nil {
		transcript.OpenReasons = append(transcript.OpenReasons, "diagnosticsUnavailable")
		return transcript
	}
	transcript.Validity = validity
	if validity != typefacts.ResolvedCallValid {
		transcript.OpenReasons = append(transcript.OpenReasons, "recoverySignature")
		return transcript
	}
	if calleeType != nil && calleeType.Flags()&checker.TypeFlagsUnion != 0 {
		evidence := newSemanticEvidence(filepath.Clean(sourceFile.FileName()))
		transcript.Targets = p.unionCallTargetsLocked(node, calleeType, &evidence)
		transcript.OpenReasons = append(transcript.OpenReasons, "compositeSignature")
		return transcript
	}

	declaration := p.currentSignatureDeclaration(signature, target)
	resolved := p.resolvedDeclaration(signature, declaration, target)
	if declaration == nil || resolved == nil {
		transcript.OpenReasons = append(transcript.OpenReasons, "declarationUnavailable")
		return transcript
	}
	selected := p.selectedSignatureLocked(
		signature, declaration, target, transcript.Kind, demand.CallableDepth,
	)
	transcript.SelectedSignature = &selected
	transcript.Completeness = append(
		transcript.Completeness,
		typefacts.InvocationDomainSignature,
		typefacts.InvocationDomainParameters,
		typefacts.InvocationDomainResult,
	)
	bindings, omissions, complete := p.invocationBindingsLocked(node, signature, declaration)
	transcript.Bindings = bindings
	transcript.OmittedParameters = omissions
	if complete {
		transcript.Completeness = append(
			transcript.Completeness,
			typefacts.InvocationDomainBindings,
			typefacts.InvocationDomainOmissions,
		)
	} else {
		transcript.OpenReasons = append(transcript.OpenReasons, "bindingOpen")
	}
	if demand.Census {
		implementation := invocationImplementationDeclaration(declaration, target)
		if implementation != nil && implementation.Body() != nil {
			transcript.ParameterUses = p.parameterUseCensusLocked(ctx, implementation)
			transcript.ControlFlow = p.controlFlowCensusLocked(implementation)
			transcript.Completeness = append(transcript.Completeness, typefacts.InvocationDomainUses)
			if len(transcript.ControlFlow.Unsupported) == 0 {
				transcript.Completeness = append(transcript.Completeness, typefacts.InvocationDomainControlFlow)
			} else {
				transcript.OpenReasons = append(transcript.OpenReasons, "controlFlowUnsupported")
			}
		} else {
			transcript.OpenReasons = append(transcript.OpenReasons, "implementationUnavailable")
		}
	}
	return transcript
}

func invocationUsesFunctionPrototypeIndirection(node *ast.Node) bool {
	expression := node.Expression()
	if expression == nil || !ast.IsPropertyAccessExpression(expression) || expression.Name() == nil {
		return false
	}
	switch expression.Name().Text() {
	case "call", "apply", "bind":
		return true
	default:
		return false
	}
}

func (p *project) selectedSignatureLocked(
	signature *checker.Signature,
	declaration *ast.Node,
	target *ast.Symbol,
	kind typefacts.CallKind,
	depth int,
) typefacts.SelectedSignature {
	resolved := p.resolvedDeclaration(signature, declaration, target)
	selected := typefacts.SelectedSignature{
		Declaration:          *resolved,
		OverloadOrdinal:      invocationOverloadOrdinal(declaration),
		OverloadCount:        invocationOverloadCount(declaration),
		MinimumArgumentCount: p.expandedMinimumArgumentCountLocked(signature, declaration),
		HasRest:              signature.HasRestParameter(),
	}
	parameters := signature.Parameters()
	declarationParameters := declaration.Parameters()
	selected.Parameters = make([]typefacts.SelectedParameter, len(parameters))
	for index, symbol := range parameters {
		parameterType := checker.Checker_getTypeAtPosition(p.checker, signature, index)
		parameter := typefacts.SelectedParameter{
			Index:    index,
			Rest:     signature.HasRestParameter() && index == len(parameters)-1,
			Optional: index >= signature.MinArgumentCount(),
			Value:    p.invocationValueFactLocked(parameterType),
		}
		if symbol != nil {
			parameter.Symbol = p.idFor(p.canonicalSymbol(symbol))
			parameter.Optional = parameter.Optional || symbol.Flags&ast.SymbolFlagsOptional != 0
			if declarations := declarationsForSymbol(symbol); len(declarations) != 0 {
				parameter.Declaration = &declarations[0]
			}
		}
		if index < len(declarationParameters) {
			parameter.Defaulted = declarationParameters[index].Initializer() != nil
			parameter.Optional = parameter.Optional || parameter.Defaulted
		}
		parameter.CallablePaths = p.callablePathsLocked(parameterType, depth)
		selected.Parameters[index] = parameter
	}
	returnType := checker.Checker_getReturnTypeOfSignature(p.checker, signature)
	selected.Result = p.invocationValueFactLocked(returnType)
	selected.ResultCallablePaths = p.callablePathsLocked(returnType, depth)
	selected.Identity = selectedSignatureDigest(kind, selected)
	return selected
}

// expandedMinimumArgumentCount reports the minimum runtime argument count.
// TypeScript's Signature.MinArgumentCount counts written formal declarations;
// for a required tuple rest it intentionally leaves the tuple's hidden slots
// behind the single rest formal. Invocation binding expands those slots, so its
// arity identity must expand the minimum as well.
func (p *project) expandedMinimumArgumentCountLocked(
	signature *checker.Signature,
	declaration *ast.Node,
) int {
	minimum := signature.MinArgumentCount()
	if !signature.HasRestParameter() {
		return minimum
	}
	parameters := declaration.Parameters()
	if len(parameters) == 0 {
		return minimum
	}
	restType := p.checker.GetTypeAtLocation(parameters[len(parameters)-1])
	if shape := tupleShapeOfType(p.checker, restType); shape != nil {
		required := shape.FixedLength
		if shape.ExactLengthKnown {
			required = shape.ExactLength
		}
		return minimum + required
	}
	return minimum
}

func invocationOverloadOrdinal(declaration *ast.Node) int {
	if declaration == nil || declaration.Symbol() == nil {
		return 0
	}
	ordinal := 0
	for _, candidate := range declaration.Symbol().Declarations {
		if candidate == declaration {
			return ordinal
		}
		if candidate.Kind == declaration.Kind {
			ordinal++
		}
	}
	return ordinal
}

func invocationOverloadCount(declaration *ast.Node) int {
	if declaration == nil || declaration.Symbol() == nil {
		return 0
	}
	count := 0
	for _, candidate := range declaration.Symbol().Declarations {
		if candidate.Kind == declaration.Kind {
			count++
		}
	}
	return count
}

func selectedSignatureDigest(kind typefacts.CallKind, selected typefacts.SelectedSignature) string {
	hash := sha256.New()
	hashField(hash, "solid-checker:typefacts:selected-signature:v2")
	hashField(hash, string(kind))
	hashField(hash, string(selected.Declaration.Symbol))
	hashField(hash, selected.Declaration.Location.Path)
	hashField(hash, strconv.Itoa(selected.Declaration.Location.StartByte))
	hashField(hash, strconv.Itoa(selected.Declaration.Location.EndByte))
	hashField(hash, strconv.Itoa(selected.OverloadOrdinal))
	hashField(hash, strconv.Itoa(selected.OverloadCount))
	hashField(hash, strconv.Itoa(selected.MinimumArgumentCount))
	hashField(hash, strconv.FormatBool(selected.HasRest))
	for _, parameter := range selected.Parameters {
		text := ""
		if parameter.Value.Type != nil {
			text = parameter.Value.Type.Text
		}
		hashField(hash, text)
	}
	result := ""
	if selected.Result.Type != nil {
		result = selected.Result.Type.Text
	}
	hashField(hash, result)
	return "sha256:" + hex.EncodeToString(hash.Sum(nil))
}

func (p *project) invocationBindingsLocked(
	call *ast.Node,
	signature *checker.Signature,
	declaration *ast.Node,
) ([]typefacts.ArgumentBinding, []int, bool) {
	arguments := call.Arguments()
	bindings := make([]typefacts.ArgumentBinding, len(arguments))
	parameters := signature.Parameters()
	expanded := 0
	complete := true
	positionOpen := false
	bound := make(map[int]struct{}, len(parameters))
	for argumentIndex, argument := range arguments {
		binding := &bindings[argumentIndex]
		binding.ArgumentIndex = argumentIndex
		binding.Location = nodeLocation(argument)
		if positionOpen {
			binding.Disposition = typefacts.ArgumentBindingUnmapped
			binding.Reason = "precededByUnknownSpread"
			complete = false
			continue
		}
		if !ast.IsSpreadElement(argument) {
			formal, rest, ok := formalAtExpandedPosition(signature, expanded)
			if !ok {
				binding.Disposition = typefacts.ArgumentBindingUnmapped
				binding.Reason = "parameterUnavailable"
				complete = false
				continue
			}
			binding.Disposition = typefacts.ArgumentBindingDirect
			binding.Slots = []typefacts.ExpandedArgumentSlot{{
				ExpandedIndex: expanded, ParameterIndex: formal, Rest: rest,
			}}
			bound[formal] = struct{}{}
			expanded++
			continue
		}
		expression := argument.Expression()
		valueType := p.checker.GetTypeAtLocation(expression)
		exactLength, exact := p.exactTupleSpreadLengthLocked(valueType)
		if !exact {
			binding.Disposition = typefacts.ArgumentBindingUnknownLengthSpread
			start := min(expanded, len(parameters))
			possible := typefacts.FormalRange{Start: start}
			if signature.HasRestParameter() {
				possible.Unbounded = true
			} else {
				end := len(parameters)
				possible.EndExclusive = &end
			}
			binding.Possible = &possible
			complete = false
			positionOpen = true
			continue
		}
		binding.Disposition = typefacts.ArgumentBindingExactTupleSpread
		binding.Slots = make([]typefacts.ExpandedArgumentSlot, exactLength)
		for tupleIndex := 0; tupleIndex < exactLength; tupleIndex++ {
			formal, rest, ok := formalAtExpandedPosition(signature, expanded)
			if !ok {
				binding.Disposition = typefacts.ArgumentBindingUnmapped
				binding.Slots = nil
				binding.Reason = "parameterUnavailable"
				complete = false
				break
			}
			tuple := tupleIndex
			binding.Slots[tupleIndex] = typefacts.ExpandedArgumentSlot{
				ExpandedIndex: expanded, TupleIndex: &tuple, ParameterIndex: formal, Rest: rest,
			}
			bound[formal] = struct{}{}
			expanded++
		}
	}
	var omitted []int
	if complete {
		for index := range parameters {
			if _, present := bound[index]; present {
				continue
			}
			if signature.HasRestParameter() && index == len(parameters)-1 {
				continue
			}
			if index >= signature.MinArgumentCount() ||
				(index < len(declaration.Parameters()) && declaration.Parameters()[index].Initializer() != nil) {
				omitted = append(omitted, index)
			}
		}
	}
	return bindings, omitted, complete
}

func (p *project) exactTupleSpreadLengthLocked(value *checker.Type) (int, bool) {
	constituents := invocationConstituents(value)
	if len(constituents) == 0 {
		return 0, false
	}
	length := -1
	for _, constituent := range constituents {
		shape := tupleShapeOfType(p.checker, constituent)
		if shape == nil || !shape.ExactLengthKnown {
			return 0, false
		}
		if length == -1 {
			length = shape.ExactLength
		} else if length != shape.ExactLength {
			return 0, false
		}
	}
	return length, true
}

func formalAtExpandedPosition(signature *checker.Signature, position int) (int, bool, bool) {
	parameters := signature.Parameters()
	if position < len(parameters) {
		rest := signature.HasRestParameter() && position >= len(parameters)-1
		if rest {
			return len(parameters) - 1, true, true
		}
		return position, false, true
	}
	if signature.HasRestParameter() && len(parameters) != 0 {
		return len(parameters) - 1, true, true
	}
	return 0, false, false
}

func (p *project) invocationValueFactLocked(value *checker.Type) typefacts.InvocationValueFact {
	fact := typefacts.InvocationValueFact{
		Callability:      callabilityOfType(p.checker, value),
		Constructability: invocationConstructabilityOfType(p.checker, value),
	}
	if value == nil {
		fact.OpenReasons = append(fact.OpenReasons, "typeUnavailable")
		return fact
	}
	fact.Type = p.typeDescriptorFor(value)
	primitive := primitiveValueDomainOfType(p.checker, value)
	fact.Primitive = typefacts.ValuePrimitiveDomain{
		MayBeString: primitive.MayBeString(), MayBeNumber: primitive.MayBeNumber(),
		MayBeBoolean: primitive.MayBeBoolean(), MayBeBigInt: primitive.MayBeBigInt(),
		MayBeSymbol: primitive.MayBeSymbol(), MayBeNull: primitive.MayBeNull(),
		MayBeUndefined: primitive.MayBeUndefined(), MayBeObject: primitive.MayBeObject(),
		NumbersFinite: primitive.NumbersAreFinite(), Unknown: primitive.Unknown(),
	}
	constituents := invocationConstituents(value)
	fact.Alternatives = make([]typefacts.ValueAlternative, len(constituents))
	for index, constituent := range constituents {
		fact.Alternatives[index] = typefacts.ValueAlternative{
			Index: index, Discriminants: p.discriminantsOfTypeLocked(constituent),
		}
	}
	fact.Partitions = p.finitePartitionsLocked(value, constituents)
	if value.Flags()&(checker.TypeFlagsAny|checker.TypeFlagsUnknown|checker.TypeFlagsIncludesError) != 0 {
		fact.OpenReasons = append(fact.OpenReasons, "openType")
	}
	if value.Flags()&checker.TypeFlagsInstantiable != 0 {
		fact.OpenReasons = append(fact.OpenReasons, "unresolvedGeneric")
	}
	if len(p.checker.GetIndexInfosOfType(value)) != 0 {
		fact.OpenReasons = append(fact.OpenReasons, "openIndex")
	}
	return fact
}

func invocationConstituents(value *checker.Type) []*checker.Type {
	if value == nil {
		return nil
	}
	if value.Flags()&checker.TypeFlagsUnion != 0 {
		return value.Types()
	}
	return []*checker.Type{value}
}

func (p *project) finitePartitionsLocked(
	value *checker.Type,
	constituents []*checker.Type,
) []typefacts.FinitePartition {
	var partitions []typefacts.FinitePartition
	if literal, ok := finiteLiteralPartition(constituents); ok {
		partitions = append(partitions, literal)
	}
	callabilityCases := make([]typefacts.FiniteCase, 0, len(constituents))
	callabilitySeen := make(map[typefacts.Callability]struct{})
	callabilityComplete := len(constituents) != 0
	for _, constituent := range constituents {
		callability := callabilityOfType(p.checker, constituent)
		if callability == typefacts.CallabilityUnknown || callability == typefacts.CallabilityMixed {
			callabilityComplete = false
			break
		}
		if _, seen := callabilitySeen[callability]; !seen {
			callabilitySeen[callability] = struct{}{}
			callabilityCases = append(callabilityCases, typefacts.FiniteCase{Kind: string(callability)})
		}
	}
	if callabilityComplete {
		sort.Slice(callabilityCases, func(i, j int) bool { return callabilityCases[i].Kind < callabilityCases[j].Kind })
		partitions = append(partitions, typefacts.FinitePartition{
			Axis: typefacts.FinitePartitionCallability, Complete: true, Cases: callabilityCases,
		})
	}
	protocolCases := make([]typefacts.FiniteCase, 0, len(constituents))
	protocolSeen := make(map[typefacts.ValueProtocol]struct{})
	protocolComplete := len(constituents) != 0
	for _, constituent := range constituents {
		protocol, ok := p.protocolOfTypeLocked(constituent)
		if !ok {
			protocolComplete = false
			break
		}
		if _, seen := protocolSeen[protocol]; !seen {
			protocolSeen[protocol] = struct{}{}
			protocolCases = append(protocolCases, typefacts.FiniteCase{Kind: string(protocol), Protocol: protocol})
		}
	}
	if protocolComplete {
		sort.Slice(protocolCases, func(i, j int) bool { return protocolCases[i].Kind < protocolCases[j].Kind })
		partitions = append(partitions, typefacts.FinitePartition{
			Axis: typefacts.FinitePartitionProtocol, Complete: true, Cases: protocolCases,
		})
	}
	tupleComplete := len(constituents) != 0
	var tupleCases []typefacts.FiniteCase
	seenLengths := make(map[int]struct{})
	for _, constituent := range constituents {
		shape := tupleShapeOfType(p.checker, constituent)
		if shape == nil || !shape.ExactLengthKnown {
			tupleComplete = false
			break
		}
		if _, seen := seenLengths[shape.ExactLength]; !seen {
			seenLengths[shape.ExactLength] = struct{}{}
			length := shape.ExactLength
			tupleCases = append(tupleCases, typefacts.FiniteCase{Kind: "tuple", TupleLength: &length})
		}
	}
	if tupleComplete {
		partitions = append(partitions, typefacts.FinitePartition{
			Axis: typefacts.FinitePartitionTuple, Complete: true, Cases: tupleCases,
		})
	}
	if discriminantCases, ok := p.discriminantPartitionLocked(constituents); ok {
		partitions = append(partitions, typefacts.FinitePartition{
			Axis: typefacts.FinitePartitionDiscriminant, Complete: true, Cases: discriminantCases,
		})
	}
	return partitions
}

func (p *project) discriminantPartitionLocked(
	constituents []*checker.Type,
) ([]typefacts.FiniteCase, bool) {
	if len(constituents) == 0 {
		return nil, false
	}
	alternatives := make([]map[string]typefacts.PrimitiveLiteralCandidate, len(constituents))
	for index, constituent := range constituents {
		alternatives[index] = make(map[string]typefacts.PrimitiveLiteralCandidate)
		for _, discriminant := range p.discriminantsOfTypeLocked(constituent) {
			alternatives[index][discriminant.Property] = discriminant.Value
		}
	}
	properties := make([]string, 0, len(alternatives[0]))
	for property := range alternatives[0] {
		properties = append(properties, property)
	}
	sort.Strings(properties)
	for _, property := range properties {
		seen := make(map[string]struct{}, len(alternatives))
		cases := make([]typefacts.FiniteCase, len(alternatives))
		valid := true
		for index, alternative := range alternatives {
			value, present := alternative[property]
			key := primitiveLiteralKey(value)
			if !present {
				valid = false
				break
			}
			if _, duplicate := seen[key]; duplicate {
				valid = false
				break
			}
			seen[key] = struct{}{}
			cases[index] = typefacts.FiniteCase{
				Kind:          "alternative",
				Discriminants: []typefacts.Discriminant{{Property: property, Value: value}},
			}
		}
		if valid {
			return cases, true
		}
	}
	return nil, false
}

func primitiveLiteralKey(value typefacts.PrimitiveLiteralCandidate) string {
	switch value.Kind {
	case typefacts.PrimitiveLiteralString:
		return "string:" + value.String
	case typefacts.PrimitiveLiteralNumber:
		return "number:" + strconv.FormatFloat(value.Number, 'g', -1, 64)
	case typefacts.PrimitiveLiteralBoolean:
		return "boolean:" + strconv.FormatBool(value.Boolean)
	default:
		return "unknown"
	}
}

func finiteLiteralPartition(constituents []*checker.Type) (typefacts.FinitePartition, bool) {
	partition := typefacts.FinitePartition{Axis: typefacts.FinitePartitionLiteral, Complete: true}
	if len(constituents) == 0 {
		return partition, false
	}
	for _, constituent := range constituents {
		candidate, kind, ok := exactPrimitiveCase(constituent)
		if !ok {
			return partition, false
		}
		partition.Cases = append(partition.Cases, typefacts.FiniteCase{Kind: kind, Literal: candidate})
	}
	return partition, true
}

func exactPrimitiveCase(value *checker.Type) (*typefacts.PrimitiveLiteralCandidate, string, bool) {
	if text, ok := checker.PrimitiveStringLiteral(value); ok {
		candidate := typefacts.PrimitiveLiteralCandidate{Kind: typefacts.PrimitiveLiteralString, String: text}
		return &candidate, "string", true
	}
	if number, ok := checker.PrimitiveNumberLiteral(value); ok {
		candidate := typefacts.PrimitiveLiteralCandidate{Kind: typefacts.PrimitiveLiteralNumber, Number: number}
		return &candidate, "number", true
	}
	if boolean, ok := checker.PrimitiveBooleanLiteral(value); ok {
		candidate := typefacts.PrimitiveLiteralCandidate{Kind: typefacts.PrimitiveLiteralBoolean, Boolean: boolean}
		return &candidate, "boolean", true
	}
	if value != nil && value.Flags()&checker.TypeFlagsNull != 0 {
		return nil, "null", true
	}
	if value != nil && value.Flags()&(checker.TypeFlagsUndefined|checker.TypeFlagsVoid) != 0 {
		return nil, "undefined", true
	}
	return nil, "", false
}

func (p *project) protocolOfTypeLocked(value *checker.Type) (typefacts.ValueProtocol, bool) {
	if value == nil || value.Flags()&(checker.TypeFlagsAny|checker.TypeFlagsUnknown|checker.TypeFlagsIncludesError|checker.TypeFlagsInstantiable) != 0 {
		return "", false
	}
	if checker.Checker_getIterationTypeOfIterable(
		p.checker,
		checker.IterationUseAsyncGeneratorReturnType,
		checker.IterationTypeKindYield,
		value,
		nil,
	) != nil {
		return typefacts.ValueProtocolAsyncIterable, true
	}
	if awaited := checker.Checker_getAwaitedType(p.checker, value); awaited != nil &&
		!checker.Checker_isTypeIdenticalTo(p.checker, value, awaited) {
		return typefacts.ValueProtocolPromise, true
	}
	return typefacts.ValueProtocolPlain, true
}

func (p *project) discriminantsOfTypeLocked(value *checker.Type) []typefacts.Discriminant {
	if value == nil {
		return nil
	}
	var discriminants []typefacts.Discriminant
	for _, property := range p.checker.GetPropertiesOfType(value) {
		name, ok := invocationPropertyName(property.Name)
		if !ok {
			continue
		}
		propertyType := p.checker.GetTypeOfPropertyOfType(value, name)
		constituents := invocationConstituents(propertyType)
		if len(constituents) != 1 {
			continue
		}
		candidate, _, ok := exactPrimitiveCase(constituents[0])
		if !ok || candidate == nil {
			continue
		}
		discriminants = append(discriminants, typefacts.Discriminant{Property: name, Value: *candidate})
	}
	sort.Slice(discriminants, func(i, j int) bool { return discriminants[i].Property < discriminants[j].Property })
	return discriminants
}

func (p *project) callablePathsLocked(value *checker.Type, depth int) []typefacts.CallablePathFact {
	constituents := invocationConstituents(value)
	paths := make([]typefacts.CallablePathFact, 0)
	closedAlternatives := make([]bool, len(constituents))
	for alternative, constituent := range constituents {
		closedAlternatives[alternative] = constituent != nil &&
			constituent.Flags()&(checker.TypeFlagsAny|checker.TypeFlagsUnknown|checker.TypeFlagsIncludesError|checker.TypeFlagsInstantiable) == 0 &&
			len(p.checker.GetIndexInfosOfType(constituent)) == 0
		p.walkCallablePathsLocked(
			constituent,
			alternative,
			nil,
			depth,
			make(map[*checker.Type]struct{}),
			&paths,
		)
	}
	// A fixed path discovered in one closed alternative must be represented in
	// every alternative. Closed alternatives prove absence; open alternatives
	// retain an explicit unknown instead. This is what prevents a union sibling
	// from inheriting another sibling's callback.
	type pathTemplate struct {
		path []typefacts.PathSegment
	}
	templates := make(map[string]pathTemplate)
	present := make(map[string]map[int]struct{})
	for _, fact := range paths {
		if len(fact.Path) == 0 {
			continue
		}
		key := callablePathKey(fact)
		templates[key] = pathTemplate{path: fact.Path}
		if present[key] == nil {
			present[key] = make(map[int]struct{})
		}
		present[key][fact.Alternative] = struct{}{}
	}
	for key, template := range templates {
		for alternative := range constituents {
			if _, exists := present[key][alternative]; exists {
				continue
			}
			presence := typefacts.PathUnknown
			complete := false
			var reasons []string
			if closedAlternatives[alternative] {
				presence = typefacts.PathAbsent
				complete = true
			} else {
				reasons = []string{"openAlternative"}
			}
			paths = append(paths, typefacts.CallablePathFact{
				Alternative:      alternative,
				Path:             append([]typefacts.PathSegment(nil), template.path...),
				Presence:         presence,
				Callability:      typefacts.CallabilityUnknown,
				Constructability: typefacts.InvocationConstructUnknown,
				Complete:         complete,
				OpenReasons:      reasons,
			})
		}
	}
	sort.Slice(paths, func(i, j int) bool {
		left, right := callablePathKey(paths[i]), callablePathKey(paths[j])
		if left != right {
			return left < right
		}
		return paths[i].Alternative < paths[j].Alternative
	})
	return paths
}

func (p *project) walkCallablePathsLocked(
	value *checker.Type,
	alternative int,
	path []typefacts.PathSegment,
	remaining int,
	seen map[*checker.Type]struct{},
	paths *[]typefacts.CallablePathFact,
) {
	fact := typefacts.CallablePathFact{
		Alternative:      alternative,
		Path:             append([]typefacts.PathSegment(nil), path...),
		Presence:         typefacts.PathRequired,
		Callability:      callabilityOfType(p.checker, value),
		Constructability: invocationConstructabilityOfType(p.checker, value),
		Complete:         value != nil && value.Flags()&(checker.TypeFlagsAny|checker.TypeFlagsUnknown|checker.TypeFlagsIncludesError) == 0,
	}
	if !fact.Complete {
		fact.OpenReasons = append(fact.OpenReasons, "openType")
	}
	*paths = append(*paths, fact)
	if value == nil || remaining == 0 {
		if value != nil && (len(p.checker.GetPropertiesOfType(value)) != 0 || checker.IsTupleType(value)) {
			last := &(*paths)[len(*paths)-1]
			last.Complete = false
			last.OpenReasons = append(last.OpenReasons, "depthLimit")
		}
		return
	}
	if _, cycling := seen[value]; cycling {
		last := &(*paths)[len(*paths)-1]
		last.Complete = false
		last.OpenReasons = append(last.OpenReasons, "cycle")
		return
	}
	seen[value] = struct{}{}
	defer delete(seen, value)
	if checker.IsTupleType(value) {
		target := value.TargetTupleType()
		elements := checker.Checker_getTypeArguments(p.checker, value)
		for index := 0; target != nil && index < target.FixedLength() && index < len(elements); index++ {
			tupleIndex := index
			segment := typefacts.PathSegment{Kind: typefacts.PathSegmentTuple, Index: &tupleIndex}
			p.walkCallablePathsLocked(elements[index], alternative, append(path, segment), remaining-1, seen, paths)
		}
		return
	}
	properties := append([]*ast.Symbol(nil), p.checker.GetPropertiesOfType(value)...)
	sort.Slice(properties, func(i, j int) bool { return properties[i].Name < properties[j].Name })
	for _, property := range properties {
		name, ok := invocationPropertyName(property.Name)
		if !ok {
			continue
		}
		propertyType := p.checker.GetTypeOfPropertyOfType(value, name)
		segment := typefacts.PathSegment{Kind: typefacts.PathSegmentProperty, Property: name}
		before := len(*paths)
		p.walkCallablePathsLocked(propertyType, alternative, append(path, segment), remaining-1, seen, paths)
		if before < len(*paths) {
			child := &(*paths)[before]
			if property.Flags&ast.SymbolFlagsOptional != 0 {
				child.Presence = typefacts.PathOptional
			}
			if declarations := declarationsForSymbol(property); len(declarations) != 0 {
				child.Declaration = &declarations[0]
			}
		}
	}
	if len(p.checker.GetIndexInfosOfType(value)) != 0 {
		last := &(*paths)[len(*paths)-1]
		last.Complete = false
		last.OpenReasons = append(last.OpenReasons, "openIndex")
	}
}

func invocationPropertyName(name string) (string, bool) {
	if name == "" || !utf8.ValidString(name) || strings.HasPrefix(name, ast.InternalSymbolNamePrefix) {
		return "", false
	}
	return name, true
}

func invocationConstructabilityOfType(
	typeChecker *checker.Checker,
	value *checker.Type,
) typefacts.InvocationConstructability {
	switch constructabilityOfType(typeChecker, value) {
	case typefacts.ConstructabilityConstructable:
		return typefacts.InvocationConstructable
	case typefacts.ConstructabilityNonConstructable:
		return typefacts.InvocationNonConstructable
	case typefacts.ConstructabilityMixed:
		return typefacts.InvocationConstructMixed
	default:
		return typefacts.InvocationConstructUnknown
	}
}

func callablePathKey(path typefacts.CallablePathFact) string {
	var builder strings.Builder
	for _, segment := range path.Path {
		if segment.Kind == typefacts.PathSegmentTuple && segment.Index != nil {
			builder.WriteByte('[')
			builder.WriteString(strconv.Itoa(*segment.Index))
			builder.WriteByte(']')
		} else {
			builder.WriteByte('.')
			builder.WriteString(segment.Property)
		}
	}
	return builder.String()
}

func invocationImplementationDeclaration(selected *ast.Node, target *ast.Symbol) *ast.Node {
	if selected != nil && selected.Body() != nil {
		return selected
	}
	if target == nil {
		return nil
	}
	for _, declaration := range target.Declarations {
		if declaration.Body() != nil && isExactCallableImplementationKind(strings.TrimPrefix(declaration.KindString(), "Kind")) {
			return declaration
		}
	}
	return nil
}

type parameterCensusRoot struct {
	index  int
	symbol *ast.Symbol
	path   []typefacts.PathSegment
}

func (p *project) parameterUseCensusLocked(
	ctx context.Context,
	implementation *ast.Node,
) []typefacts.ParameterUse {
	roots := p.parameterCensusRootsLocked(implementation)
	bySymbol := make(map[*ast.Symbol]parameterCensusRoot, len(roots))
	aliases := make(map[*ast.Symbol]struct{})
	for _, root := range roots {
		bySymbol[p.canonicalSymbol(root.symbol)] = root
	}
	// Close the deliberately small proven-alias domain to a fixed point before
	// classifying uses. Only immutable identifier-to-identifier bindings enter;
	// mutation, destructuring and property storage remain ordinary escapes.
	for changed := true; changed; {
		changed = false
		implementation.Body().ForEachChild(func(node *ast.Node) bool {
			var scan func(*ast.Node)
			scan = func(current *ast.Node) {
				if current == nil {
					return
				}
				if ast.IsVariableDeclaration(current) && ast.IsVarConst(current) &&
					ast.IsIdentifier(current.Name()) && ast.IsIdentifier(current.Initializer()) {
					initializer := p.canonicalSymbol(p.checker.GetSymbolAtLocation(current.Initializer()))
					if root, ok := bySymbol[initializer]; ok {
						alias := p.canonicalSymbol(p.checker.GetSymbolAtLocation(current.Name()))
						if alias != nil {
							if _, exists := bySymbol[alias]; !exists {
								bySymbol[alias] = root
								aliases[alias] = struct{}{}
								changed = true
							}
						}
					}
				}
				current.ForEachChild(func(child *ast.Node) bool {
					scan(child)
					return false
				})
			}
			scan(node)
			return false
		})
	}
	uses := make([]typefacts.ParameterUse, 0)
	var visit func(*ast.Node, bool)
	visit = func(node *ast.Node, captured bool) {
		if node == nil || ctx.Err() != nil {
			return
		}
		nested := captured || node != implementation.Body() && isCallableDeclaration(node)
		if ast.IsIdentifier(node) && !ast.IsDeclarationNameOrImportPropertyName(node) && !ast.IsPartOfTypeNode(node) {
			symbol := p.canonicalSymbol(p.checker.GetSymbolAtLocation(node))
			if root, ok := bySymbol[symbol]; ok {
				_, alias := aliases[symbol]
				kind := p.parameterUseKindLocked(node)
				if alias && kind == typefacts.ParameterUseDirectCall {
					kind = typefacts.ParameterUseAliasCall
				}
				if nested {
					kind = typefacts.ParameterUseCapture
				}
				uses = append(uses, typefacts.ParameterUse{
					ParameterIndex: root.index,
					BindingPath:    append([]typefacts.PathSegment(nil), root.path...),
					Location:       nodeLocation(node),
					Kind:           kind,
					Alias:          alias,
					Captured:       nested,
				})
			}
		}
		node.ForEachChild(func(child *ast.Node) bool {
			visit(child, nested)
			return false
		})
	}
	visit(implementation.Body(), false)
	sort.Slice(uses, func(i, j int) bool {
		if uses[i].Location.Path != uses[j].Location.Path {
			return uses[i].Location.Path < uses[j].Location.Path
		}
		return uses[i].Location.StartByte < uses[j].Location.StartByte
	})
	return uses
}

func (p *project) parameterCensusRootsLocked(implementation *ast.Node) []parameterCensusRoot {
	var roots []parameterCensusRoot
	for index, parameter := range implementation.Parameters() {
		var walk func(*ast.Node, []typefacts.PathSegment)
		walk = func(node *ast.Node, path []typefacts.PathSegment) {
			if node == nil {
				return
			}
			if ast.IsIdentifier(node) {
				if symbol := p.checker.GetSymbolAtLocation(node); symbol != nil {
					roots = append(roots, parameterCensusRoot{index: index, symbol: symbol, path: append([]typefacts.PathSegment(nil), path...)})
				}
				return
			}
			if ast.IsObjectBindingPattern(node) || ast.IsArrayBindingPattern(node) {
				for elementIndex, element := range node.AsBindingPattern().Elements.Nodes {
					if element == nil || !ast.IsBindingElement(element) {
						continue
					}
					var segment typefacts.PathSegment
					if ast.IsArrayBindingPattern(node) {
						index := elementIndex
						segment = typefacts.PathSegment{Kind: typefacts.PathSegmentTuple, Index: &index}
					} else {
						name := element.AsBindingElement().PropertyName
						if name == nil {
							name = element.Name()
						}
						if name == nil {
							continue
						}
						segment = typefacts.PathSegment{Kind: typefacts.PathSegmentProperty, Property: name.Text()}
					}
					walk(element.Name(), append(path, segment))
				}
				return
			}
			node.ForEachChild(func(child *ast.Node) bool {
				walk(child, path)
				return false
			})
		}
		walk(parameter.Name(), nil)
	}
	return roots
}

func (p *project) parameterUseKindLocked(node *ast.Node) typefacts.ParameterUseKind {
	parent := node.Parent
	if parent == nil {
		return typefacts.ParameterUseUnknownEscape
	}
	if isCallLikeExpression(parent) && parent.Expression() == node {
		return typefacts.ParameterUseDirectCall
	}
	if ast.IsPropertyAccessExpression(parent) && parent.Expression() == node {
		return typefacts.ParameterUsePropertyAccess
	}
	if ast.IsReturnStatement(parent) {
		return typefacts.ParameterUseReturn
	}
	if ast.IsVariableDeclaration(parent) && parent.Initializer() == node {
		return typefacts.ParameterUseStorage
	}
	if isArgumentOfCall(node, parent) {
		signature := checker.Checker_getResolvedSignature(p.checker, parent, nil, checker.CheckModeNormal)
		if signature != nil && signature.Flags()&checker.SignatureFlagsIsSignatureCandidateForOverloadFailure == 0 {
			return typefacts.ParameterUseArgumentKnown
		}
		return typefacts.ParameterUseArgumentUnknown
	}
	return typefacts.ParameterUseUnknownEscape
}

func isArgumentOfCall(node, parent *ast.Node) bool {
	if !isCallLikeExpression(parent) {
		return false
	}
	for _, argument := range parent.Arguments() {
		if argument == node {
			return true
		}
	}
	return false
}

func isCallableDeclaration(node *ast.Node) bool {
	return ast.IsArrowFunction(node) || ast.IsFunctionExpression(node) ||
		ast.IsFunctionDeclaration(node) || ast.IsMethodDeclaration(node)
}

func (p *project) controlFlowCensusLocked(implementation *ast.Node) *typefacts.ControlFlowCensus {
	census := &typefacts.ControlFlowCensus{}
	body := implementation.Body()
	var scan func(*ast.Node, typefacts.Reachability) typefacts.Reachability
	scan = func(node *ast.Node, reach typefacts.Reachability) typefacts.Reachability {
		if node == nil {
			return reach
		}
		if node != body && isCallableDeclaration(node) {
			return reach
		}
		if ast.IsBlock(node) {
			current := reach
			for _, statement := range node.AsBlock().Statements.Nodes {
				current = scan(statement, current)
			}
			return current
		}
		if ast.IsReturnStatement(node) {
			var value *typefacts.InvocationValueFact
			var captures []int
			if expression := node.Expression(); expression != nil {
				fact := p.invocationValueFactLocked(p.checker.GetTypeAtLocation(expression))
				value = &fact
				captures = p.returnedClosureCapturesLocked(implementation, expression)
			}
			census.Returns = append(census.Returns, typefacts.ReturnSite{
				Location: nodeLocation(node), Reach: reach, Value: value, Captures: captures,
			})
			return typefacts.Unreachable
		}
		if ast.IsThrowStatement(node) {
			census.Throws = append(census.Throws, typefacts.ThrowSite{Location: nodeLocation(node), Reach: reach})
			return typefacts.Unreachable
		}
		if ast.IsIfStatement(node) {
			statement := node.AsIfStatement()
			expression := statement.Expression
			var partitions []typefacts.FinitePartition
			if expression != nil {
				partitions = p.invocationValueFactLocked(p.checker.GetTypeAtLocation(expression)).Partitions
			}
			census.Branches = append(census.Branches, typefacts.BranchSite{
				Location: nodeLocation(node), Reach: reach, Partitions: partitions,
			})
			thenReach := scan(statement.ThenStatement, reach)
			elseReach := reach
			if statement.ElseStatement != nil {
				elseReach = scan(statement.ElseStatement, reach)
			}
			return mergeReachability(thenReach, elseReach)
		}
		if ast.IsConditionalExpression(node) {
			expression := node.AsConditionalExpression()
			partitions := p.invocationValueFactLocked(p.checker.GetTypeAtLocation(expression.Condition)).Partitions
			census.Branches = append(census.Branches, typefacts.BranchSite{
				Location: nodeLocation(node), Reach: reach, Partitions: partitions,
			})
			return mergeReachability(scan(expression.WhenTrue, reach), scan(expression.WhenFalse, reach))
		}
		if ast.IsTryStatement(node) || ast.IsIterationStatement(node, true) || ast.IsSwitchStatement(node) {
			marker := "switchReachability"
			if ast.IsTryStatement(node) {
				marker = "tryReachability"
			} else if ast.IsIterationStatement(node, true) {
				marker = "iterationReachability"
			}
			census.Unsupported = append(census.Unsupported, marker)
			if ast.IsSwitchStatement(node) {
				expression := node.Expression()
				partitions := p.invocationValueFactLocked(p.checker.GetTypeAtLocation(expression)).Partitions
				census.Branches = append(census.Branches, typefacts.BranchSite{
					Location: nodeLocation(node), Reach: reach, Partitions: partitions,
				})
			}
			node.ForEachChild(func(child *ast.Node) bool {
				scan(child, typefacts.ReachUnknown)
				return false
			})
			return typefacts.ReachUnknown
		}
		current := reach
		node.ForEachChild(func(child *ast.Node) bool {
			current = scan(child, current)
			return false
		})
		return current
	}
	scan(body, typefacts.Reachable)
	sort.Strings(census.Unsupported)
	census.Unsupported = compactStrings(census.Unsupported)
	return census
}

func mergeReachability(left, right typefacts.Reachability) typefacts.Reachability {
	if left == right {
		return left
	}
	if left == typefacts.Reachable || right == typefacts.Reachable {
		return typefacts.Reachable
	}
	return typefacts.ReachUnknown
}

func (p *project) returnedClosureCapturesLocked(
	implementation *ast.Node,
	expression *ast.Node,
) []int {
	if !isCallableDeclaration(expression) {
		return nil
	}
	roots := p.parameterCensusRootsLocked(implementation)
	bySymbol := make(map[*ast.Symbol]int, len(roots))
	for _, root := range roots {
		bySymbol[p.canonicalSymbol(root.symbol)] = root.index
	}
	seen := make(map[int]struct{})
	expression.ForEachChild(func(node *ast.Node) bool {
		var visit func(*ast.Node)
		visit = func(current *ast.Node) {
			if ast.IsIdentifier(current) && !ast.IsDeclarationNameOrImportPropertyName(current) {
				if index, ok := bySymbol[p.canonicalSymbol(p.checker.GetSymbolAtLocation(current))]; ok {
					seen[index] = struct{}{}
				}
			}
			current.ForEachChild(func(child *ast.Node) bool {
				visit(child)
				return false
			})
		}
		visit(node)
		return false
	})
	indices := make([]int, 0, len(seen))
	for index := range seen {
		indices = append(indices, index)
	}
	sort.Ints(indices)
	return indices
}

func nodeLocation(node *ast.Node) typefacts.Location {
	if node == nil {
		return typefacts.Location{}
	}
	sourceFile := ast.GetSourceFileOfNode(node)
	if sourceFile == nil {
		return typefacts.Location{}
	}
	return typefacts.Location{
		Path:      filepath.Clean(sourceFile.FileName()),
		StartByte: scanner.SkipTrivia(sourceFile.Text(), node.Pos()),
		EndByte:   node.End(),
	}
}

func invocationDemandDigest(demands []typefacts.InvocationDemand) string {
	hash := sha256.New()
	hashField(hash, "solid-checker:typefacts:invocations:v1")
	for _, demand := range demands {
		hashField(hash, demand.Location.Path)
		hashField(hash, strconv.Itoa(demand.Location.StartByte))
		hashField(hash, strconv.Itoa(demand.Location.EndByte))
		hashField(hash, strconv.Itoa(demand.CallableDepth))
		hashField(hash, strconv.FormatBool(demand.Census))
	}
	return "sha256:" + hex.EncodeToString(hash.Sum(nil))
}

type hashStringWriter interface {
	Write([]byte) (int, error)
}

func hashField(hash hashStringWriter, value string) {
	length := strconv.Itoa(len(value))
	_, _ = hash.Write([]byte(length))
	_, _ = hash.Write([]byte{':'})
	_, _ = hash.Write([]byte(value))
}

func sha256String(value []byte) string {
	digest := sha256.Sum256(value)
	return "sha256:" + hex.EncodeToString(digest[:])
}

func (p *project) invocationSourceDigestsLocked() []typefacts.TranscriptSourceDigest {
	files := p.program.SourceFiles()
	digests := make([]typefacts.TranscriptSourceDigest, 0, len(files))
	for _, sourceFile := range files {
		digest := sha256.Sum256([]byte(sourceFile.Text()))
		digests = append(digests, typefacts.TranscriptSourceDigest{
			Path:   filepath.Clean(sourceFile.FileName()),
			SHA256: "sha256:" + hex.EncodeToString(digest[:]),
		})
	}
	sort.Slice(digests, func(i, j int) bool { return digests[i].Path < digests[j].Path })
	return digests
}

func compactStrings(values []string) []string {
	if len(values) < 2 {
		return values
	}
	write := 1
	for read := 1; read < len(values); read++ {
		if values[read] == values[write-1] {
			continue
		}
		values[write] = values[read]
		write++
	}
	return values[:write]
}
