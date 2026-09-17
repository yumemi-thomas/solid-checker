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
			declarationParameter := declarationParameters[index]
			parameter.Defaulted = declarationParameter.Initializer() != nil
			parameter.Optional = parameter.Optional || parameter.Defaulted
			parameter.DeclaredType = p.declaredTypeReferenceLocked(declarationParameter)
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

func (p *project) declaredTypeReferenceLocked(
	parameter *ast.Node,
) *typefacts.DeclaredTypeReference {
	if parameter == nil || !ast.IsParameterDeclaration(parameter) {
		return nil
	}
	typeNode := parameter.AsParameterDeclaration().Type
	if typeNode == nil || !ast.IsTypeReferenceNode(typeNode) {
		return nil
	}
	typeName := typeNode.AsTypeReferenceNode().TypeName
	if typeName == nil || !ast.IsIdentifier(typeName) {
		return nil
	}
	symbol := p.checker.GetSymbolAtLocation(typeName)
	module, name := importedAliasIdentity(symbol)
	if module == "" || name == "" || !utf8.ValidString(module) || !utf8.ValidString(name) {
		return nil
	}
	return &typefacts.DeclaredTypeReference{Name: name, Module: module}
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

// overloadDeclarations is the declaration set an overload ordinal and count
// range over: the symbol's declarations of the signature's own kind that state
// a call signature. TypeScript's overload set is the bodiless declarations; the
// implementation that follows them has a body and is not a signature of the
// type. A function declared once, with its body, is its own one-member set.
// Counting the implementation reported `overloadCount == len(signatures) + 1`
// for every overloaded function whose *source* was analyzed rather than its
// `.d.ts`, and the consumer, which requires the count to agree with the type's
// signatures, refused the set as incomplete (docs/precision-backlog.md).
func overloadDeclarations(declaration *ast.Node) []*ast.Node {
	if declaration == nil || declaration.Symbol() == nil {
		return nil
	}
	var sameKind, bodiless []*ast.Node
	for _, candidate := range declaration.Symbol().Declarations {
		if candidate.Kind != declaration.Kind {
			continue
		}
		sameKind = append(sameKind, candidate)
		if candidate.Body() == nil {
			bodiless = append(bodiless, candidate)
		}
	}
	if len(bodiless) != 0 {
		return bodiless
	}
	return sameKind
}

func invocationOverloadOrdinal(declaration *ast.Node) int {
	for ordinal, candidate := range overloadDeclarations(declaration) {
		if candidate == declaration {
			return ordinal
		}
	}
	return 0
}

func invocationOverloadCount(declaration *ast.Node) int {
	return len(overloadDeclarations(declaration))
}

func selectedSignatureDigest(kind typefacts.CallKind, selected typefacts.SelectedSignature) string {
	hash := sha256.New()
	hashField(hash, "solid-checker:typefacts:selected-signature:v3")
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
		if parameter.DeclaredType == nil {
			hashField(hash, "")
			hashField(hash, "")
		} else {
			hashField(hash, parameter.DeclaredType.Module)
			hashField(hash, parameter.DeclaredType.Name)
		}
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
	if value != nil && len(p.checker.GetIndexInfosOfType(value)) != 0 &&
		!invocationValueHasClosedExactTupleIndex(p.checker, value) &&
		!invocationValueHasClosedPrimitiveApparentIndex(p.checker, value, primitive) &&
		!invocationValueHasClosedArrayIndex(p.checker, value) {
		fact.OpenReasons = append(fact.OpenReasons, "openIndex")
	}
	return fact
}

// invocationValueHasClosedExactTupleIndex distinguishes a tuple's synthesized
// numeric index from an author-declared open key space. A single fixed tuple's
// index value is exactly the union of the slots the callable-path walk already
// enumerates. Optional and rest elements keep the index open, as does every
// non-tuple (including a union whose constituents happen to be tuples).
func invocationValueHasClosedExactTupleIndex(typeChecker *checker.Checker, value *checker.Type) bool {
	if value == nil || value.Flags()&checker.TypeFlagsUnion != 0 {
		return false
	}
	indexInfos := typeChecker.GetIndexInfosOfType(value)
	if len(indexInfos) != 1 || !checker.Checker_isTypeIdenticalTo(
		typeChecker,
		indexInfos[0].KeyType(),
		typeChecker.GetNumberType(),
	) {
		return false
	}
	shape := tupleShapeOfType(typeChecker, value)
	return shape != nil && shape.ExactLengthKnown && !shape.HasRest
}

// invocationValueHasClosedPrimitiveApparentIndex recognizes only the
// intrinsic String wrapper's synthesized numeric index. The primitive value's
// own shape is still string; the wrapper index answers member enumeration, not
// whether the value itself is callable, constructable, or primitive. Any
// augmentation, boxed String object, mixed primitive domain, or changed index
// value keeps the root open.
func invocationValueHasClosedPrimitiveApparentIndex(
	typeChecker *checker.Checker,
	value *checker.Type,
	primitive typefacts.PrimitiveValueDomain,
) bool {
	pureString := typefacts.NewPrimitiveValueDomain(true, false, false, false, false, false, false, false, false)
	if primitive != pureString {
		return false
	}
	indexInfos := typeChecker.GetIndexInfosOfType(value)
	return len(indexInfos) == 1 &&
		checker.Checker_isTypeIdenticalTo(typeChecker, indexInfos[0].KeyType(), typeChecker.GetNumberType()) &&
		checker.Checker_isTypeIdenticalTo(typeChecker, indexInfos[0].ValueType(), typeChecker.GetStringType())
}

// invocationValueHasClosedArrayIndex recognizes only the global Array and
// ReadonlyArray references whose sole numeric index has exactly their element
// type. This closes the value's own shape and says nothing about its unbounded
// member census, on both fact kinds: the root value fact drops openIndex, and a
// callable-path fact records the unenumerated elements as SubtreeEnumerated=false
// while keeping its own Complete and no openIndex. The whole-census consumer
// gates read SubtreeEnumerated and still refuse; a demand naming that exact path
// asserts nothing about the elements and no longer reads the census's premise as
// its own. Tuples are handled by the stricter fixed-length arm above, and
// author-defined array-like interfaces do not satisfy the checker's
// array-or-tuple predicate.
//
// Ask this of the value's own type, never of its apparent type: a type parameter
// constrained to an array has array index infos but is not an array, and closing
// it on the strength of a constraint is unsound (an instantiation may intersect
// in a call signature).
func invocationValueHasClosedArrayIndex(typeChecker *checker.Checker, value *checker.Type) bool {
	if value == nil || value.Flags()&checker.TypeFlagsUnion != 0 ||
		!checker.Checker_isArrayOrTupleType(typeChecker, value) ||
		checker.IsTupleType(value) {
		return false
	}
	indexInfos := typeChecker.GetIndexInfosOfType(value)
	elements := checker.Checker_getTypeArguments(typeChecker, value)
	return len(indexInfos) == 1 && len(elements) == 1 &&
		checker.Checker_isTypeIdenticalTo(typeChecker, indexInfos[0].KeyType(), typeChecker.GetNumberType()) &&
		checker.Checker_isTypeIdenticalTo(typeChecker, indexInfos[0].ValueType(), elements[0])
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
	for alternative, constituent := range constituents {
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
	// every alternative. The nearest observed prefix proves absence only when
	// its own shape is closed and its children were enumerated. A depth, cycle,
	// index, or generic cut retains an explicit unknown instead. This prevents a
	// union sibling from inheriting another sibling's callback without claiming
	// a path absent below an unenumerated parent.
	//
	// A closed declared census is not evidence of absence for every name. The
	// compiler augments every object type with the global `Object` interface's
	// members, and GetPropertiesOfType never enumerates them, so `toString`,
	// `hasOwnProperty` and `valueOf` are answered by the compiler — `tsc`
	// accepts `({ dispose(): void }).toString()` — on a sibling whose declared
	// census names none of them. The tuple member census makes exactly those
	// names reachable as templates, and a template some alternative declares
	// outright reaches them too. Such a name therefore retains the explicit
	// unknown regardless of what the sibling's prefix proves, and if the global
	// `Object` interface does not resolve at all, no absence is synthesized for
	// any template.
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
		if present[key] == nil {
			present[key] = make(map[int]struct{})
		}
		present[key][fact.Alternative] = struct{}{}
		// An apparent member does not oblige a sibling alternative to answer
		// for it, and reconciling it would cost closure rather than buy
		// precision. Every alternative to which the compiler's Function
		// fallback applies emits the same nine leaves at the same path length,
		// because the depth decrement is uniform — so the only alternatives a
		// reconciled apparent template can reach are the ones that genuinely do
		// not have the member. There the augmented-name rule below refuses to
		// prove absence, and the synthesized fact is an explicit unknown that
		// carries Apparent=false and therefore *counts* against declared-member
		// census closure. `(() => void) | undefined` measured 20 facts with 9
		// open declared ones that way, against 11 facts and none open with this
		// exclusion. The templates that close F2/F3 come from declared
		// siblings, so they are unaffected.
		if fact.Apparent {
			continue
		}
		templates[key] = pathTemplate{path: fact.Path}
	}
	for key, template := range templates {
		for alternative := range constituents {
			if _, exists := present[key][alternative]; exists {
				continue
			}
			presence := typefacts.PathUnknown
			complete := false
			var reasons []string
			subtreeEnumerated := false
			if !p.templateNameMayBeCompilerAugmentedLocked(template.path) &&
				callablePathPrefixProvesAbsence(paths, alternative, template.path) {
				presence = typefacts.PathAbsent
				complete = true
				subtreeEnumerated = true
			} else {
				reasons = []string{"openAlternative"}
			}
			paths = append(paths, typefacts.CallablePathFact{
				Alternative:       alternative,
				Path:              append([]typefacts.PathSegment(nil), template.path...),
				Presence:          presence,
				Callability:       typefacts.CallabilityUnknown,
				Constructability:  typefacts.InvocationConstructUnknown,
				Complete:          complete,
				SubtreeEnumerated: subtreeEnumerated,
				OpenReasons:       reasons,
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

func callablePathPrefixProvesAbsence(
	paths []typefacts.CallablePathFact,
	alternative int,
	path []typefacts.PathSegment,
) bool {
	nearest := -1
	proven := false
	for index := range paths {
		fact := &paths[index]
		if fact.Alternative != alternative || len(fact.Path) >= len(path) ||
			!callablePathSegmentsPrefix(fact.Path, path) || len(fact.Path) <= nearest {
			continue
		}
		nearest = len(fact.Path)
		proven = fact.Presence == typefacts.PathRequired && fact.Complete &&
			fact.SubtreeEnumerated && len(fact.OpenReasons) == 0
	}
	return nearest >= 0 && proven
}

func callablePathSegmentsPrefix(prefix, path []typefacts.PathSegment) bool {
	if len(prefix) > len(path) {
		return false
	}
	for index := range prefix {
		left, right := prefix[index], path[index]
		if left.Kind != right.Kind || left.Property != right.Property ||
			(left.Index == nil) != (right.Index == nil) ||
			(left.Index != nil && *left.Index != *right.Index) {
			return false
		}
	}
	return true
}

func (p *project) walkCallablePathsLocked(
	value *checker.Type,
	alternative int,
	path []typefacts.PathSegment,
	remaining int,
	seen map[*checker.Type]struct{},
	paths *[]typefacts.CallablePathFact,
) {
	factIndex := len(*paths)
	callability := callabilityOfType(p.checker, value)
	constructability := invocationConstructabilityOfType(p.checker, value)
	fact := typefacts.CallablePathFact{
		Alternative:      alternative,
		Path:             append([]typefacts.PathSegment(nil), path...),
		Presence:         typefacts.PathRequired,
		Callability:      callability,
		Constructability: constructability,
		Complete: value != nil &&
			value.Flags()&(checker.TypeFlagsAny|checker.TypeFlagsUnknown|checker.TypeFlagsIncludesError|checker.TypeFlagsInstantiable) == 0 &&
			callability != typefacts.CallabilityUnknown &&
			constructability != typefacts.InvocationConstructUnknown,
		SubtreeEnumerated: true,
	}
	if value != nil && value.Flags()&checker.TypeFlagsInstantiable != 0 {
		fact.OpenReasons = append(fact.OpenReasons, "unresolvedGeneric")
	}
	if !fact.Complete && len(fact.OpenReasons) == 0 {
		fact.OpenReasons = append(fact.OpenReasons, "openType")
	}
	*paths = append(*paths, fact)
	if value != nil && len(p.checker.GetIndexInfosOfType(value)) != 0 &&
		!invocationValueHasClosedExactTupleIndex(p.checker, value) {
		fact := &(*paths)[factIndex]
		// An exact Array's sole numeric index is the compiler's own synthesized
		// element accessor, and this walk emits the array's declared members but
		// never its elements. That is a statement about member *enumeration*,
		// not about this node's shape: callability, constructability and the
		// type flags are all answered for `T[]` exactly as they are for a fixed
		// tuple. So record it the way the depth and cycle cuts are recorded —
		// SubtreeEnumerated only. The whole-census gates
		// (`callable_path_census_is_closed`) still refuse it; a demand naming
		// this exact path, which asserts nothing about the elements, no longer
		// reads the census's premise as its own.
		//
		// Everything else keeps `openIndex` and its incompleteness: a string- or
		// symbol-keyed index signature, a union, and any tuple with an optional
		// or rest element are all author-declared open key spaces whose value
		// type is not the element type this walk skipped.
		fact.SubtreeEnumerated = false
		if !invocationValueHasClosedArrayIndex(p.checker, value) {
			fact.Complete = false
			fact.OpenReasons = append(fact.OpenReasons, "openIndex")
		}
	}
	if value == nil || remaining == 0 {
		if value != nil && (len(p.checker.GetPropertiesOfType(value)) != 0 || checker.IsTupleType(value)) {
			(*paths)[factIndex].SubtreeEnumerated = false
		}
		return
	}
	if _, cycling := seen[value]; cycling {
		(*paths)[factIndex].SubtreeEnumerated = false
		return
	}
	seen[value] = struct{}{}
	defer delete(seen, value)
	emitted := make(map[string]struct{})
	if checker.IsTupleType(value) {
		target := value.TargetTupleType()
		elements := checker.Checker_getTypeArguments(p.checker, value)
		for index := 0; target != nil && index < target.FixedLength() && index < len(elements); index++ {
			tupleIndex := index
			segment := typefacts.PathSegment{Kind: typefacts.PathSegmentTuple, Index: &tupleIndex}
			p.walkCallablePathsLocked(elements[index], alternative, append(path, segment), remaining-1, seen, paths)
		}
		// A tuple's element slots are Tuple segments, above. Its *members* are
		// the ones its Array or ReadonlyArray base declares — slice, length,
		// map, and the rest — which GetPropertiesOfType returns and which this
		// branch used to drop by returning after the elements. A plain array
		// already censuses them, so dropping them for a tuple made the same
		// member answerable for `T[]` and absent for `[T, T]`. The base's
		// numeric element properties are skipped: those slots are already the
		// Tuple segments, and emitting both would name one value twice.
		p.walkDeclaredMembersLocked(value, alternative, path, remaining, seen, paths, emitted, true)
		p.appendApparentFunctionMembersLocked(value, alternative, path, paths, emitted)
		return
	}
	p.walkDeclaredMembersLocked(value, alternative, path, remaining, seen, paths, emitted, false)
	p.appendApparentFunctionMembersLocked(value, alternative, path, paths, emitted)
}

// walkDeclaredMembersLocked emits and recurses into the members
// GetPropertiesOfType returns for this node — the ones some declaration in the
// program actually writes down. skipArrayIndexNames drops the canonical numeric
// element properties of a tuple, whose slots the caller already emitted as
// Tuple segments.
func (p *project) walkDeclaredMembersLocked(
	value *checker.Type,
	alternative int,
	path []typefacts.PathSegment,
	remaining int,
	seen map[*checker.Type]struct{},
	paths *[]typefacts.CallablePathFact,
	emitted map[string]struct{},
	skipArrayIndexNames bool,
) {
	properties := append([]*ast.Symbol(nil), p.checker.GetPropertiesOfType(value)...)
	sort.Slice(properties, func(i, j int) bool { return properties[i].Name < properties[j].Name })
	for _, property := range properties {
		name, ok := invocationPropertyName(property.Name)
		if !ok {
			continue
		}
		if skipArrayIndexNames && isCanonicalArrayIndexName(name) {
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
		emitted[name] = struct{}{}
	}
}

// isCanonicalArrayIndexName is the compiler's own canonical numeric index
// spelling: the decimal digits that round-trip through a number. "01", "1.0",
// "-1" and "1e3" are ordinary string keys, not element slots.
func isCanonicalArrayIndexName(name string) bool {
	if name == "0" {
		return true
	}
	if name == "" || name[0] < '1' || name[0] > '9' {
		return false
	}
	for index := 1; index < len(name); index++ {
		if name[index] < '0' || name[index] > '9' {
			return false
		}
	}
	return true
}

// appendApparentFunctionMembersLocked emits the members a callable node carries
// through the compiler's apparent-type augmentation rather than through any
// declaration of its own.
//
// getPropertyOfType falls back to the global `Function` interface (via
// CallableFunction or NewableFunction) for every object type with call or
// construct signatures, so `fn.bind`, `fn.call`, `fn.toString` and the rest are
// real members the compiler answers and `tsc` accepts. GetPropertiesOfType does
// not enumerate them, so before this the census said such a value had *no*
// members, and a consumer asking for one read absence where the compiler
// answers a type.
//
// The facts are leaves, and deliberately so. Recursing would be unbounded in
// the useless direction (bind.bind.bind…) while the values reached are
// library-owned and never caller-supplied, so no proof about the package under
// analysis can rest on them. They carry Apparent, which keeps them out of
// declared-member census closure and out of the template set that
// cross-alternative reconciliation answers for — though they still serve as
// prefixes there, so nothing below one can be proved absent through it. An
// exact path lookup finds them like any other fact.
//
// A member the node declares itself wins: the compiler's own lookup order
// consults resolved members before the Function fallback, so the declared fact
// already emitted is the answer. Each member's type comes from
// GetTypeOfPropertyOfType on the node — TypeScript's answer for that node,
// which is how an augmented `interface Function` or a strictBindCallApply
// signature reaches the fact rather than a shape reconstructed here.
func (p *project) appendApparentFunctionMembersLocked(
	value *checker.Type,
	alternative int,
	path []typefacts.PathSegment,
	paths *[]typefacts.CallablePathFact,
	emitted map[string]struct{},
) {
	if value == nil ||
		(len(p.checker.GetSignaturesOfType(value, checker.SignatureKindCall)) == 0 &&
			len(p.checker.GetSignaturesOfType(value, checker.SignatureKindConstruct)) == 0) {
		return
	}
	for _, name := range p.apparentFunctionMemberNamesLocked() {
		if _, done := emitted[name]; done {
			continue
		}
		memberType := p.checker.GetTypeOfPropertyOfType(value, name)
		if memberType == nil {
			continue
		}
		emitted[name] = struct{}{}
		// Presence is the compiler's, not a constant. `interface Function`
		// declares nine required members today, but an augmentation may add
		// `maybe?(): void`, and reporting that as required would tell a
		// consumer a member is always there when the type says it may not be.
		// The declared walk reads the same flag off the same kind of symbol.
		presence := typefacts.PathRequired
		if symbol := p.checker.GetPropertyOfType(value, name); symbol != nil &&
			symbol.Flags&ast.SymbolFlagsOptional != 0 {
			presence = typefacts.PathOptional
		}
		callability := callabilityOfType(p.checker, memberType)
		constructability := invocationConstructabilityOfType(p.checker, memberType)
		fact := typefacts.CallablePathFact{
			Alternative: alternative,
			Path: append(
				append([]typefacts.PathSegment(nil), path...),
				typefacts.PathSegment{Kind: typefacts.PathSegmentProperty, Property: name},
			),
			Presence:         presence,
			Callability:      callability,
			Constructability: constructability,
			Complete: memberType.Flags()&(checker.TypeFlagsAny|
				checker.TypeFlagsUnknown|
				checker.TypeFlagsIncludesError|
				checker.TypeFlagsInstantiable) == 0 &&
				callability != typefacts.CallabilityUnknown &&
				constructability != typefacts.InvocationConstructUnknown,
			Apparent: true,
		}
		if memberType.Flags()&checker.TypeFlagsInstantiable != 0 {
			fact.OpenReasons = append(fact.OpenReasons, "unresolvedGeneric")
		}
		if !fact.Complete && len(fact.OpenReasons) == 0 {
			fact.OpenReasons = append(fact.OpenReasons, "openType")
		}
		*paths = append(*paths, fact)
	}
}

// templateNameMayBeCompilerAugmentedLocked reports whether a reconciliation
// template ends in a property name the compiler can answer on *any* object type
// through its apparent-type augmentation — that is, a member of the global
// `Object` interface.
//
// It exists because a closed declared census is not evidence that such a name
// is absent. GetPropertiesOfType never enumerates the `Object` fallback, so a
// sibling alternative declaring `{ dispose(): void }` has a closed, fully
// enumerated census that names no `toString` — while `tsc` accepts
// `obj.toString()`.
//
// The global `Function` interface is deliberately *not* consulted. Its
// fallback applies only to a type with call or construct signatures, and every
// such alternative emits all nine apparent leaves at the same path length
// (the depth decrement is uniform), so a Function-only name is reconciled into
// an alternative only when that alternative genuinely lacks it — `tsc` agrees
// that `({ q: 1 }).bind` does not exist. Suppressing there would give up a
// correct absence for all nine names and buy nothing.
func (p *project) templateNameMayBeCompilerAugmentedLocked(path []typefacts.PathSegment) bool {
	names, resolved := p.objectMemberNamesLocked()
	return templateNameMayBeAugmented(names, resolved, path)
}

// templateNameMayBeAugmented is the decision itself, separated from the checker
// so the fail-closed branch is testable without a program.
//
// An unresolved name set suppresses *every* absence, including for a tuple
// segment. Without the set there is no way to tell a genuinely absent member
// from one the `Object` fallback supplies, and the census must not guess in the
// permissive direction. Only the last segment is consulted: every earlier
// segment had to be observed for the template to exist, and the prefix proof
// already requires the nearest one to be closed and enumerated.
func templateNameMayBeAugmented(
	objectMembers map[string]struct{},
	resolved bool,
	path []typefacts.PathSegment,
) bool {
	if !resolved {
		return true
	}
	if len(path) == 0 {
		return false
	}
	last := path[len(path)-1]
	if last.Kind != typefacts.PathSegmentProperty {
		return false
	}
	_, augmented := objectMembers[last.Property]
	return augmented
}

// objectMemberNamesLocked is the global `Object` interface's member names for
// this program, and whether the interface resolved at all. getPropertyOfTypeEx
// falls back to it for every object type, so a name in this set may exist on a
// value whose declared census does not mention it.
func (p *project) objectMemberNamesLocked() (map[string]struct{}, bool) {
	if p.objectMemberNamesResolved {
		return p.objectMemberNames, p.objectMemberNames != nil
	}
	p.objectMemberNamesResolved = true
	declared := p.globalInterfaceMemberNamesLocked("Object")
	if declared == nil {
		return nil, false
	}
	names := make(map[string]struct{}, len(declared))
	for _, name := range declared {
		names[name] = struct{}{}
	}
	p.objectMemberNames = names
	return p.objectMemberNames, true
}

// apparentFunctionMemberNamesLocked is the global `Function` interface's member
// names for this program, in sorted order. A project whose lib declares no
// `Function` (or declares it with type parameters) gets an empty set, and the
// census then simply carries no apparent members — the compiler's own fallback
// is equally absent there. A user augmentation of `interface Function` is part
// of the answer, because it is part of what the compiler resolves.
func (p *project) apparentFunctionMemberNamesLocked() []string {
	if p.apparentFunctionMembersResolved {
		return p.apparentFunctionMembers
	}
	p.apparentFunctionMembersResolved = true
	p.apparentFunctionMembers = p.globalInterfaceMemberNamesLocked("Function")
	return p.apparentFunctionMembers
}

// globalInterfaceMemberNamesLocked enumerates one zero-arity global
// interface's member names, sorted, filtering symbol-named members. A project
// whose lib omits the interface (or declares it with type parameters) yields
// nothing, which is the honest answer: the compiler's own fallback is equally
// absent there. A user augmentation is included, because it is part of what the
// compiler resolves.
func (p *project) globalInterfaceMemberNamesLocked(global string) []string {
	declared := checker.Checker_getGlobalType(p.checker, global, 0, false)
	if declared == nil {
		return nil
	}
	names := make([]string, 0)
	for _, property := range p.checker.GetPropertiesOfType(declared) {
		name, ok := invocationPropertyName(property.Name)
		if !ok {
			continue
		}
		names = append(names, name)
	}
	sort.Strings(names)
	return names
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
	unsafeJumps := p.unsafeJumpRegionsLocked(implementation)
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
					current.Name() != nil && current.Initializer() != nil &&
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
	body := implementation.Body()
	p.walkImplementationBodyLocked(
		implementation,
		func(node *ast.Node, enclosing *ast.Node, reach typefacts.Reachability) {
			if ctx.Err() != nil {
				return
			}
			// A shorthand property assignment's name is *both* a declaration
			// name and a value read: `{ cb }` stores `cb` exactly as
			// `{ cb: cb }` does. The declaration-name guard skipped it, so
			// `const holder = { cb }` recorded no use of `cb` at all while
			// every other escape of the same value — `{ cb: cb }`, `[cb]`,
			// `(0, cb)`, `new Map([["k", cb]])` — recorded `unknownEscape`.
			// A consumer that reads this census as exhaustive was therefore
			// blind to the shorthand, and the value could leave the body
			// unrecorded.
			//
			// The use is recorded as `unknownEscape`, which is what this
			// census says about a value it cannot classify further: the
			// producer knows the value was stored into an object literal and
			// states no more than that. This also covers `{ ...{ cb } }`,
			// whose inner literal is the same node kind.
			shorthand := node.Parent != nil &&
				ast.IsShorthandPropertyAssignment(node.Parent) &&
				node.Parent.Name() == node
			if !ast.IsIdentifier(node) || ast.IsPartOfTypeNode(node) ||
				(ast.IsDeclarationNameOrImportPropertyName(node) && !shorthand) {
				return
			}
			// The use census exempts an implementation whose own body *is* a
			// callable, where the call census does not. The shared walk answers
			// the honest question — the innermost callable containing the node,
			// the body included — and each census reads the answer its own
			// consumers need. Moving this exemption would turn `DirectCall` into
			// `Capture` for a `const`-declared concise-arrow export, which
			// operation reachability reads as an open escape; that is a separate
			// measured decision, recorded in docs/precision-backlog.md.
			captured := enclosing != nil && enclosing != body
			flowOwner := enclosing
			if flowOwner == nil {
				flowOwner = implementation
			}
			if reach != typefacts.Unreachable &&
				locationWithheldByJump(unsafeJumps[flowOwner], nodeLocation(node)) {
				return
			}
			// The symbol at a shorthand name is the *property*, not the value
			// it reads; the value symbol has its own resolution.
			resolved := p.checker.GetSymbolAtLocation(node)
			if shorthand {
				resolved = p.checker.GetShorthandAssignmentValueSymbol(node.Parent)
			}
			symbol := p.canonicalSymbol(resolved)
			root, ok := bySymbol[symbol]
			if !ok {
				return
			}
			_, alias := aliases[symbol]
			kind := p.parameterUseKindLocked(node)
			// parameterUseKindLocked already answers unknownEscape for a shorthand
			// property name; this is belt-and-braces, not the guard, so a change
			// to the classifier cannot silently turn an object-literal escape
			// into a weaker kind.
			if shorthand {
				kind = typefacts.ParameterUseUnknownEscape
			}
			if alias && kind == typefacts.ParameterUseDirectCall {
				kind = typefacts.ParameterUseAliasCall
			}
			if captured {
				kind = typefacts.ParameterUseCapture
			}
			uses = append(uses, typefacts.ParameterUse{
				ParameterIndex: root.index,
				BindingPath:    append([]typefacts.PathSegment(nil), root.path...),
				Location:       nodeLocation(node),
				Reach:          reach,
				Kind:           kind,
				Alias:          alias,
				Captured:       captured,
			})
		},
	)
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

// isCallableDeclaration reports whether a node opens a function-like frame of
// its own: it owns a body, and the statements in that body run when *it* is
// invoked rather than when the code around it runs.
//
// Every walk that asks "is this a nested callable" asks this, so the
// enumeration has to be exhaustive rather than a list of the shapes that came
// up. It is taken from the compiler's own `IsFunctionLikeDeclaration` — the
// closed set of body-bearing function-like declaration kinds: arrows, function
// expressions and declarations, methods, **constructors**, and **get/set
// accessors** — plus a class's static block, which the compiler classifies
// separately and which is likewise a body of its own.
//
// Listing only arrows, function expressions, function declarations and methods
// was a soundness hole rather than an omission: `registry.push({ get value() {
// cb(); return 1; } })` stores an object, and the getter's body runs only when
// somebody reads the property. A walk that did not stop at the accessor
// reported `cb()` as a call the *enclosing* body makes, which is the strongest
// form of every claim built on the census. `class Holder { constructor() {
// cb(); } }` and `class Holder { static { cb(); } }` are the same shape.
//
// A signature or type kind (`FunctionType`, `MethodSignature`, …) is
// deliberately *not* here: it has no body, so nothing inside it is a call site
// in the first place, and treating a type node as a callable value would be a
// different mistake.
//
// The descent that asks the neighbouring question — "is this node a callable
// *value*", in returnedCallablesLocked and calleeImplementationLocked — reads
// the same predicate. The two readings coincide in reach because those sites
// only ever see expressions and value declarations, and no accessor,
// constructor or static block can appear in either position: an object
// literal's accessors are filtered out by objectLiteralPropertyValue, which
// accepts a property assignment and nothing else.
func isCallableDeclaration(node *ast.Node) bool {
	return node != nil &&
		(ast.IsFunctionLikeDeclaration(node) || ast.IsClassStaticBlockDeclaration(node))
}

// containsReturnStatement reports whether `node` holds a `return` of the
// enclosing function -- one not inside a nested callable, whose returns are
// that callable's own. It decides whether an arm that never completes normally
// left by throwing alone or may have left by returning a value.
func containsReturnStatement(node *ast.Node) bool {
	if node == nil {
		return false
	}
	if ast.IsReturnStatement(node) {
		return true
	}
	found := false
	node.ForEachChild(func(child *ast.Node) bool {
		if found || isCallableDeclaration(child) {
			return false
		}
		if containsReturnStatement(child) {
			found = true
		}
		return false
	})
	return found
}

// returnedParameterIdentityLocked proves binding identity, not value shape.
// Every excluded form remains open; symbol spelling is never positive evidence.
func (p *project) returnedParameterIdentityLocked(implementation, expression *ast.Node) *typefacts.ParameterValueSource {
	if implementation == nil || ast.HasSyntacticModifier(implementation, ast.ModifierFlagsAsync) {
		return nil
	}
	return p.unwrittenParameterIdentityLocked(implementation, expression)
}

// Lexical binding identity survives an await when no code can replace the
// binding. This is deliberately separate from return identity: an async
// function still wraps its result even when it returns an unwritten parameter.
func (p *project) unwrittenParameterIdentityLocked(implementation, expression *ast.Node) *typefacts.ParameterValueSource {
	if implementation == nil {
		return nil
	}
	switch {
	case ast.IsFunctionDeclaration(implementation):
		if implementation.AsFunctionDeclaration().AsteriskToken != nil {
			return nil
		}
	case ast.IsFunctionExpression(implementation):
		if implementation.AsFunctionExpression().AsteriskToken != nil {
			return nil
		}
	case ast.IsArrowFunction(implementation):
	default:
		return nil
	}
	expression = identityPreservingUnwrap(expression)
	if expression == nil || !ast.IsIdentifier(expression) {
		return nil
	}
	symbol := p.canonicalSymbol(p.checker.GetSymbolAtLocation(expression))
	// A redeclaration initializer replaces the parameter without appearing in
	// the assignment-target census. Require the witnessed binding itself to
	// have one declaration before treating it as the caller's original value.
	if symbol == nil || len(symbol.Declarations) != 1 || p.symbolIsAssignedLocked(symbol, implementation) {
		return nil
	}
	// Direct eval and mapped arguments can mutate a binding without a visible
	// assignment to its symbol. Refuse mentions rather than guessing strictness.
	if mentionsArgumentsOrEval(implementation) {
		return nil
	}
	var result *typefacts.ParameterValueSource
	names := make(map[string]bool)
	for index, parameter := range implementation.Parameters() {
		if name := parameter.Name(); name != nil && ast.IsIdentifier(name) {
			if names[name.Text()] {
				return nil
			}
			names[name.Text()] = true
		}
		if parameter.Name() == nil || !ast.IsIdentifier(parameter.Name()) ||
			parameter.Initializer() != nil || parameter.AsParameterDeclaration().DotDotDotToken != nil {
			continue
		}
		if p.canonicalSymbol(p.checker.GetSymbolAtLocation(parameter.Name())) == symbol {
			if result != nil {
				return nil
			}
			result = &typefacts.ParameterValueSource{ParameterIndex: index}
		}
	}
	return result
}

func (p *project) controlFlowCensusLocked(implementation *ast.Node) *typefacts.ControlFlowCensus {
	census := &typefacts.ControlFlowCensus{}
	body := implementation.Body()
	if body != nil && !ast.IsBlock(body) {
		value := p.invocationValueFactLocked(p.checker.GetTypeAtLocation(body))
		carried := p.carriedCallableLocationsLocked(body)
		reachable := typefacts.Reachable
		census.Returns = append(census.Returns, typefacts.ReturnSite{
			Location:         nodeLocation(body),
			Reach:            typefacts.Reachable,
			Value:            &value,
			Parameter:        p.returnedParameterIdentityLocked(implementation, body),
			CarriedCallables: carried,
			CarryReach:       &reachable,
			Sources:          p.returnValueSourcesLocked(body),
		})
		return census
	}
	// markIncomplete is the single writer of both incompleteness records. The
	// marker set a consumer already reads and the classified rows can never
	// disagree, because there is nowhere to append to one of them alone: the
	// client refuses a transcript whose two lists name different markers, and a
	// second append site is exactly how that drift starts.
	markIncomplete := func(
		marker string,
		class typefacts.ControlFlowIncompletenessClass,
		node *ast.Node,
	) {
		census.Unsupported = append(census.Unsupported, marker)
		census.Incompleteness = append(census.Incompleteness, typefacts.ControlFlowIncompleteness{
			Marker:   marker,
			Class:    class,
			Location: nodeLocation(node),
		})
	}
	// ReturnSite.Reach deliberately retains the producer's historical,
	// optimistic reachability. carryReach is a separate lower-bound premise for
	// a value-carry edge: entering either arm of an undecidable branch makes the
	// edge conditional, while both arms falling through restores the incoming
	// lower bound for statements after the branch.
	type flowState struct {
		reach      typefacts.Reachability
		carryReach typefacts.Reachability
	}
	unknownArm := func(state flowState) flowState {
		if state.carryReach != typefacts.Unreachable {
			state.carryReach = typefacts.ReachUnknown
		}
		return state
	}
	var scan func(*ast.Node, flowState) flowState
	scan = func(node *ast.Node, state flowState) flowState {
		if node == nil {
			return state
		}
		if node != body && isCallableDeclaration(node) {
			return state
		}
		if ast.IsBlock(node) {
			current := state
			for _, statement := range node.AsBlock().Statements.Nodes {
				current = scan(statement, current)
			}
			return current
		}
		if ast.IsReturnStatement(node) {
			var value *typefacts.InvocationValueFact
			var carried []typefacts.Location
			if expression := node.Expression(); expression != nil {
				fact := p.invocationValueFactLocked(p.checker.GetTypeAtLocation(expression))
				value = &fact
				carried = p.carriedCallableLocationsLocked(expression)
			}
			var carryReach *typefacts.Reachability
			if node.Expression() != nil {
				edgeReach := state.carryReach
				carryReach = &edgeReach
			}
			census.Returns = append(census.Returns, typefacts.ReturnSite{
				Location: nodeLocation(node), Reach: state.reach, Value: value,
				Parameter:        p.returnedParameterIdentityLocked(implementation, node.Expression()),
				CarriedCallables: carried, CarryReach: carryReach,
				Sources: p.returnValueSourcesLocked(node.Expression()),
			})
			return flowState{reach: typefacts.Unreachable, carryReach: typefacts.Unreachable}
		}
		if ast.IsThrowStatement(node) {
			census.Throws = append(census.Throws, typefacts.ThrowSite{
				Location: nodeLocation(node), Reach: state.reach,
			})
			return flowState{reach: typefacts.Unreachable, carryReach: typefacts.Unreachable}
		}
		if ast.IsIfStatement(node) {
			statement := node.AsIfStatement()
			expression := statement.Expression
			var partitions []typefacts.FinitePartition
			if expression != nil {
				partitions = p.invocationValueFactLocked(p.checker.GetTypeAtLocation(expression)).Partitions
			}
			census.Branches = append(census.Branches, typefacts.BranchSite{
				Location: nodeLocation(node), Reach: state.reach, Partitions: partitions,
			})
			truthy, known := p.literalTruthinessLocked(expression, 0)
			thenInput := state
			elseInput := state
			if known {
				if truthy {
					elseInput = flowState{reach: typefacts.Unreachable, carryReach: typefacts.Unreachable}
				} else {
					thenInput = flowState{reach: typefacts.Unreachable, carryReach: typefacts.Unreachable}
				}
			} else {
				thenInput = unknownArm(thenInput)
				elseInput = unknownArm(elseInput)
			}
			thenState := scan(statement.ThenStatement, thenInput)
			elseState := elseInput
			if statement.ElseStatement != nil {
				elseState = scan(statement.ElseStatement, elseInput)
			}
			if known {
				if truthy {
					return thenState
				}
				return elseState
			}
			merged := flowState{reach: mergeReachability(thenState.reach, elseState.reach)}
			// A throw guard -- an arm whose exit is unreachable and whose only
			// way out is a `throw`, never a `return` -- takes nothing from the
			// state after the `if`: an execution that reaches the next
			// statement took the other arm and completed it, and no
			// value-return edge competes from inside the guard, because an
			// execution that throws returns no value at all. When the other arm
			// is absent or always completes normally, the successor therefore
			// keeps the entry carry strength, and `if (!ok) throw …; return
			// input;` still has an unconditional value-return edge
			// (`@solidjs/web`'s `withMeta`). An arm that may `return` is not a
			// guard: its return is another value-return edge, and the merge
			// below keeps the successor's carry unknown exactly as before.
			thenGuards := thenState.reach == typefacts.Unreachable &&
				!containsReturnStatement(statement.ThenStatement)
			elseGuards := statement.ElseStatement != nil &&
				elseState.reach == typefacts.Unreachable &&
				!containsReturnStatement(statement.ElseStatement)
			switch {
			case merged.reach == typefacts.Unreachable || state.carryReach == typefacts.Unreachable:
				merged.carryReach = typefacts.Unreachable
			case p.constructCompletesNormallyLocked(statement.ThenStatement) &&
				(statement.ElseStatement == nil || p.constructCompletesNormallyLocked(statement.ElseStatement)):
				merged.carryReach = state.carryReach
			case thenGuards &&
				(statement.ElseStatement == nil || p.constructCompletesNormallyLocked(statement.ElseStatement)):
				merged.carryReach = state.carryReach
			case elseGuards && p.constructCompletesNormallyLocked(statement.ThenStatement):
				merged.carryReach = state.carryReach
			default:
				merged.carryReach = typefacts.ReachUnknown
			}
			return merged
		}
		if ast.IsConditionalExpression(node) {
			expression := node.AsConditionalExpression()
			partitions := p.invocationValueFactLocked(p.checker.GetTypeAtLocation(expression.Condition)).Partitions
			census.Branches = append(census.Branches, typefacts.BranchSite{
				Location: nodeLocation(node), Reach: state.reach, Partitions: partitions,
			})
			thenState := scan(expression.WhenTrue, unknownArm(state))
			elseState := scan(expression.WhenFalse, unknownArm(state))
			return flowState{
				reach:      mergeReachability(thenState.reach, elseState.reach),
				carryReach: state.carryReach,
			}
		}
		if ast.IsTryStatement(node) || ast.IsIterationStatement(node, true) || ast.IsSwitchStatement(node) {
			marker := "switchReachability"
			if ast.IsTryStatement(node) {
				marker = "tryReachability"
			} else if ast.IsIterationStatement(node, true) {
				marker = "iterationReachability"
			}
			// The construct is walked in full below — every child is scanned,
			// and the shared body walk records every call and invoking form
			// inside it — so what is missing is only the lower bound: control
			// may not enter a loop body, a catch clause, or a selected clause.
			// That is ControlFlowReachabilityLowerBound, and it is what lets a
			// consumer asking a may-execute question read this transcript.
			markIncomplete(marker, typefacts.ControlFlowReachabilityLowerBound, node)
			if ast.IsSwitchStatement(node) {
				expression := node.Expression()
				partitions := p.invocationValueFactLocked(p.checker.GetTypeAtLocation(expression)).Partitions
				census.Branches = append(census.Branches, typefacts.BranchSite{
					Location: nodeLocation(node), Reach: state.reach, Partitions: partitions,
				})
			}
			node.ForEachChild(func(child *ast.Node) bool {
				scan(child, flowState{
					reach: typefacts.ReachUnknown, carryReach: typefacts.ReachUnknown,
				})
				return false
			})
			// Reachability *inside* the construct stays unknown — a loop body may
			// never run, a catch clause may never be entered — which is why the
			// children above are scanned with ReachUnknown and the marker stands.
			// Reachability *after* it is a separate question, and for a construct
			// that can only complete normally it has the same answer as before it:
			// control arrives at the following statement either way. Answering it
			// keeps a single `for (const key of …)` from poisoning every remaining
			// statement of the implementation.
			//
			// An unreachable construct is never promoted: nothing that follows dead
			// code becomes live by having a loop in between.
			switch {
			case state.reach == typefacts.Unreachable:
				return flowState{reach: typefacts.Unreachable, carryReach: typefacts.Unreachable}
			case state.reach == typefacts.Reachable && p.constructCompletesNormallyLocked(node):
				return state
			default:
				return flowState{reach: typefacts.ReachUnknown, carryReach: typefacts.ReachUnknown}
			}
		}
		if ast.IsBreakStatement(node) || isContinueStatement(node) {
			// Exact jump-target flow is outside this census. In particular, a
			// labelled break can bypass a later return without changing the legacy
			// optimistic Reach row. Mark the whole census unsupported so no
			// callable-return edge can acquire lower-bound authority from it.
			//
			// This is ControlFlowUnaccounted rather than the lower-bound class,
			// and the reason is what the *target* buys: every repair either
			// census applies to a jump is bounded by the construct that owns
			// the target — the region implementationCallCensusLocked reduces to
			// `unknown`, and the fallthrough question
			// constructCompletesNormallyLocked answers. A jump this predicate
			// declines is one whose target no enclosing construct of this frame
			// owns, so there is no region to bound and the census is not
			// claiming to know where control goes. Over-refusal is the safe
			// direction; the two shapes real code is made of — a `break` inside
			// the `switch` or loop that owns it — are handled and leave the
			// construct's own lower-bound marker alone.
			if !jumpHandledByFallthroughConstruct(node, implementation) {
				markIncomplete("jumpReachability", typefacts.ControlFlowUnaccounted, node)
			}
			return flowState{reach: typefacts.Unreachable, carryReach: typefacts.Unreachable}
		}
		current := state
		node.ForEachChild(func(child *ast.Node) bool {
			current = scan(child, current)
			return false
		})
		return current
	}
	scan(body, flowState{reach: typefacts.Reachable, carryReach: typefacts.Reachable})
	sort.Strings(census.Unsupported)
	census.Unsupported = compactStrings(census.Unsupported)
	census.Incompleteness = sortedControlFlowIncompleteness(census.Incompleteness)
	if stringListContains(census.Unsupported, "jumpReachability") {
		// The optimistic ReturnSite reach rows remain useful to their historical
		// consumers, but partial control flow is never enough to authorize a
		// callable carried by a return. Nested-census construction omits this
		// callable entirely; the implementation-level equivalent is absence of
		// carry authority on every return.
		for index := range census.Returns {
			census.Returns[index].CarryReach = nil
		}
	}
	return census
}

// sortedControlFlowIncompleteness puts the classified rows in a deterministic
// order and removes exact duplicates. One row per construct is the invariant, so
// a duplicate can only come from two records of the same construct.
func sortedControlFlowIncompleteness(
	rows []typefacts.ControlFlowIncompleteness,
) []typefacts.ControlFlowIncompleteness {
	if len(rows) == 0 {
		return nil
	}
	sort.SliceStable(rows, func(left, right int) bool {
		first, second := rows[left], rows[right]
		if first.Location.Path != second.Location.Path {
			return first.Location.Path < second.Location.Path
		}
		if first.Location.StartByte != second.Location.StartByte {
			return first.Location.StartByte < second.Location.StartByte
		}
		if first.Location.EndByte != second.Location.EndByte {
			return first.Location.EndByte < second.Location.EndByte
		}
		if first.Marker != second.Marker {
			return first.Marker < second.Marker
		}
		return first.Class < second.Class
	})
	unique := rows[:1]
	for _, row := range rows[1:] {
		if row != unique[len(unique)-1] {
			unique = append(unique, row)
		}
	}
	return unique
}

// unsafeJumpRegionsLocked records the exact target-owned region in which a jump
// makes the shared body walk's positive execution rows non-universal. Source
// byte order is insufficient: a `for` update is written before its body break,
// and sibling branch order says nothing about execution. A break therefore
// covers its whole target subtree. A continue covers only the loop body; the
// update/condition remain valid may-execute evidence. The target boundary
// restores authority and no region crosses a callable identity.
//
// A jump whose target is not inside the frame at all has no such boundary, so
// its region is the **whole flow owner**. Regions are keyed by flow owner, which
// is what makes this walk cover a jump inside a *nested* callable: the
// control-flow census never enters one and so leaves no marker there, while
// this walk does and reduces that callable's own rows.
//
// What a caller does with a region is reduce the row's Reach to `unknown`, not
// drop the row — see implementationCallCensusLocked.
func (p *project) unsafeJumpRegionsLocked(
	implementation *ast.Node,
) map[*ast.Node][]typefacts.Location {
	regions := make(map[*ast.Node][]typefacts.Location)
	p.walkImplementationBodyLocked(
		implementation,
		func(node *ast.Node, enclosing *ast.Node, _ typefacts.Reachability) {
			if !ast.IsBreakStatement(node) && !isContinueStatement(node) {
				return
			}
			owner := enclosing
			if owner == nil {
				owner = implementation
			}
			region := owner
			if target := jumpTargetWithin(node, owner); target != nil {
				region = target
				if isContinueStatement(node) && !tryBetweenJumpAndTarget(node, target) {
					for region != nil && region.KindString() == "KindLabeledStatement" {
						region = region.Statement()
					}
					if region != nil && ast.IsIterationStatement(region, false) {
						region = region.Statement()
					}
				}
			}
			// A narrowing step that lands on nothing leaves the jump with no
			// region at all, which would be a jump whose rows nothing repairs.
			// Nothing in the grammar is known to produce it; the fallback is
			// the frame, so the failure mode is over-refusal rather than an
			// unrepaired row.
			if region == nil {
				region = owner
			}
			regions[owner] = append(regions[owner], nodeLocation(region))
		},
	)
	return regions
}

func tryBetweenJumpAndTarget(jump, target *ast.Node) bool {
	for ancestor := jump.Parent; ancestor != nil && ancestor != target; ancestor = ancestor.Parent {
		if ast.IsTryStatement(ancestor) {
			return true
		}
	}
	return false
}

func locationWithheldByJump(regions []typefacts.Location, location typefacts.Location) bool {
	for _, region := range regions {
		if location.Path == region.Path && location.StartByte >= region.StartByte &&
			location.EndByte <= region.EndByte {
			return true
		}
	}
	return false
}

func stringListContains(values []string, wanted string) bool {
	for _, value := range values {
		if value == wanted {
			return true
		}
	}
	return false
}

// jumpHandledByFallthroughConstruct reports whether the shared walkers already
// have an exact enclosing loop/switch owner for this break or continue. A jump
// to a labelled block outside those constructs is deliberately excluded: its
// target is the unsupported shape that triggered the per-frame refusal.
func jumpHandledByFallthroughConstruct(jump, callable *ast.Node) bool {
	for ancestor := jump.Parent; ancestor != nil; ancestor = ancestor.Parent {
		if ancestor != callable && isCallableDeclaration(ancestor) {
			return false
		}
		if ast.IsTryStatement(ancestor) {
			return false
		}
		if ast.IsIterationStatement(ancestor, true) || ast.IsSwitchStatement(ancestor) {
			// The nearest fallthrough construct must own the target. Looking
			// through it for an outer label would let the nearer construct merge
			// an abrupt path back into positive may-execute evidence.
			return jumpTargetWithin(jump, ancestor) != nil
		}
		if ancestor == callable {
			return false
		}
	}
	return false
}

// walkImplementationBodyLocked visits every node of an implementation body once,
// threading the two facts every implementation census classifies a node by:
// which callable frame the node sits in, and whether invoking the
// implementation reaches it by falling through.
//
// One walk on purpose. The call census and the parameter-use census answer the
// same question about the same statement — does invoking the export run this? —
// and a use census that carried no answer at all is how a property access in
// code after a `return` came to witness a read the export never performs. There
// is one reachability notion here, and both censuses read it from this walk.
//
// `observe` sees each node before its children, with the *innermost callable
// containing it* and the reachability in effect at that node. Nested callables
// are descended into rather than skipped, exactly as both censuses always have.
//
// The walk carries the containing callable rather than a bare "is captured"
// flag. A consumer that only knows a call is captured somewhere inside a
// carried range must assume the callables in between run; one that knows
// *which* callable immediately contains it can require each link of the chain
// to be proven on its own.
//
// *Every* callable is a boundary here, the implementation's own body included
// when that body is itself a callable. `export const wrap = cb => () => cb();`
// has a concise body that is an arrow, and exempting it stamped the `cb()` site
// as a call the implementation makes, for an implementation that only hands a
// closure back. The walk states the honest answer; a census that wants the
// exemption applies it to this answer itself (parameterUseCensusLocked does,
// deliberately, and says why).
func (p *project) walkImplementationBodyLocked(
	implementation *ast.Node,
	observe func(node *ast.Node, enclosing *ast.Node, reach typefacts.Reachability),
) {
	body := implementation.Body()
	if body == nil {
		return
	}
	var visit func(*ast.Node, *ast.Node, typefacts.Reachability) typefacts.Reachability
	visit = func(
		node *ast.Node, enclosing *ast.Node, reach typefacts.Reachability,
	) typefacts.Reachability {
		if node == nil {
			return reach
		}
		nested := enclosing
		if isCallableDeclaration(node) {
			nested = node
		}
		observe(node, nested, reach)
		switch {
		case ast.IsBlock(node):
			current := reach
			for _, statement := range node.AsBlock().Statements.Nodes {
				current = visit(statement, nested, current)
			}
			return current
		case ast.IsIfStatement(node):
			statement := node.AsIfStatement()
			visit(statement.Expression, nested, reach)
			thenReach := visit(
				statement.ThenStatement,
				nested,
				p.literalBranchReachLocked(reach, statement.Expression, true),
			)
			// The arm that is not written is still a path out of the `if`, and it
			// is excluded by exactly the same literal condition that excludes a
			// written one: after `if (true) { return }` nothing falls through.
			elseReach := p.literalBranchReachLocked(reach, statement.Expression, false)
			if statement.ElseStatement != nil {
				elseReach = visit(statement.ElseStatement, nested, elseReach)
			}
			return mergeReachability(thenReach, elseReach)
		case ast.IsTryStatement(node):
			statement := node.AsTryStatement()
			tryReach := visit(statement.TryBlock, nested, reach)
			catchReach := typefacts.Unreachable
			if statement.CatchClause != nil {
				clause := statement.CatchClause.AsCatchClause()
				// The catch *parameter*, before the block. A destructuring
				// catch binding may carry a default — `catch ({ message =
				// describe() })` — and that call is reached exactly when the
				// clause is. Visiting the block alone left it in no census at
				// all, which is the one thing this walk may not do: the
				// enumeration a negative call domain rests on is over every
				// node of the frame. It went unnoticed while the only census
				// that asks for that enumeration refused every `try` outright.
				if clause.VariableDeclaration != nil {
					visit(clause.VariableDeclaration, nested, reach)
				}
				catchReach = visit(clause.Block, nested, reach)
			}
			completes := mergeReachability(tryReach, catchReach)
			if statement.FinallyBlock != nil {
				// A `finally` runs on every path out of the `try`, so its own
				// contents are reached exactly when the `try` statement was —
				// `reach`, not the merge, because `try { return x } finally
				// { cleanup() }` does run `cleanup`. What follows the `try` is a
				// different question: it is reached only if the finally block
				// completes too, since a `return` there overrides the jump it
				// interrupted. Discarding that answer is what let
				// `try {…} finally { return }` claim its successor runs.
				completes = conjoinReachability(
					completes, visit(statement.FinallyBlock, nested, reach),
				)
			}
			return completes
		case ast.IsIterationStatement(node, true), ast.IsSwitchStatement(node):
			if ast.IsSwitchStatement(node) {
				statement := node.AsSwitchStatement()
				visit(statement.Expression, nested, reach)
				clauseEntry := typefacts.ReachUnknown
				if reach == typefacts.Unreachable {
					clauseEntry = typefacts.Unreachable
				}
				for _, clauseNode := range statement.CaseBlock.AsCaseBlock().Clauses.Nodes {
					clause := clauseNode.AsCaseOrDefaultClause()
					if clause.Expression != nil {
						visit(clause.Expression, nested, clauseEntry)
					}
					current := clauseEntry
					for _, child := range clause.Statements.Nodes {
						current = visit(child, nested, current)
					}
				}
			} else {
				node.ForEachChild(func(child *ast.Node) bool {
					visit(child, nested, typefacts.ReachUnknown)
					return false
				})
			}
			// The same question controlFlowCensusLocked answers, answered the same
			// way: reachability *inside* the construct stays unknown, because a
			// loop body may never run; reachability *after* a construct that can
			// only complete normally is whatever it was before it, because control
			// arrives at the following statement either way. Without this a single
			// `for (const [k, v] of …)` poisons every remaining statement of the
			// implementation.
			//
			// An unreachable construct is never promoted: nothing that follows dead
			// code becomes live by having a loop in between.
			switch {
			case reach == typefacts.Unreachable:
				return typefacts.Unreachable
			case reach == typefacts.Reachable && p.constructCompletesNormallyLocked(node):
				return typefacts.Reachable
			default:
				return typefacts.ReachUnknown
			}
		}
		terminates := ast.IsReturnStatement(node) || ast.IsThrowStatement(node) ||
			ast.IsBreakStatement(node) || isContinueStatement(node)
		node.ForEachChild(func(child *ast.Node) bool {
			visit(child, nested, reach)
			return false
		})
		if terminates {
			return typefacts.Unreachable
		}
		return reach
	}
	visit(body, nil, typefacts.Reachable)
}

// conjoinReachability answers "reached only if both of these are", which is what
// a `finally` imposes on the statement after its `try`: the try or catch has to
// complete normally *and* so does the finally block.
func conjoinReachability(left, right typefacts.Reachability) typefacts.Reachability {
	switch {
	case left == typefacts.Unreachable || right == typefacts.Unreachable:
		return typefacts.Unreachable
	case left == typefacts.ReachUnknown || right == typefacts.ReachUnknown:
		return typefacts.ReachUnknown
	default:
		return typefacts.Reachable
	}
}

// literalBranchReachLocked is the reachability of one arm of an `if` whose
// condition may be a decidable literal. `if (false) { … }` is dead code exactly
// as code after a `return` is, and a census calling it reachable is what would
// let a property access there witness a read the export never performs.
//
// Only a decidable literal condition (including the `const` indirection and the
// literal comparison literalTruthiness reads) rules an arm out. Every other
// condition leaves both arms at the reachability the `if` itself had — the
// optimistic reading this walk has always taken, retained deliberately and
// recorded as an approximation in docs/precision-backlog.md.
func (p *project) literalBranchReachLocked(
	reach typefacts.Reachability,
	condition *ast.Node,
	taken bool,
) typefacts.Reachability {
	if truthy, known := p.literalTruthinessLocked(condition, 0); known && truthy != taken {
		return typefacts.Unreachable
	}
	return reach
}

// maxNestedConstructDepth bounds the recursion through nested loop / `try` /
// `switch` constructs. Real implementations nest a handful deep; an input that
// exceeds this answers "does not provably complete", which is the conservative
// direction.
const maxNestedConstructDepth = 32

// constructCompletesNormally reports whether a loop, `try`, or `switch` can only
// finish by falling out of its bottom — in which case the statement after it is
// reached exactly when the construct itself was.
//
// Three things can falsify that. The first is a jump out: a `return` or `throw`
// anywhere inside, or a `break`/`continue` whose target is outside. The second is
// a loop with no exit edge — `while (true)`, `while (1)`, `do … while (!0)`,
// `for (;;)` — for which the following statement is reached only through a
// `break` aimed at the loop, so the loop must contain one. The third is a *nested*
// construct that itself cannot complete: a `while (true)` inside a `try`, a
// `for (;;)` inside a `for … of`, an endless loop in a `switch` clause. Control
// that enters one never reaches the bottom of the construct wrapping it, so the
// nested answer is required before the outer one can be given.
//
// The nested descent asks a different question than the top-level one. It asks
// whether control can leave the nested construct *at all* — not whether it leaves
// by falling out the bottom. A `break outer` inside a nested `while (true)` never
// reaches that loop's bottom, yet it lands exactly where the enclosing construct
// falls through to, and the scan below classifies that jump itself. Only a
// construct with no way out at all stops the statement after the enclosing one
// from being reached.
//
// Deliberately conservative shapes, each answering false and leaving today's
// ReachUnknown in place:
//
//   - Any `throw`, including one a sibling `catch` would swallow (`try { throw x }
//     catch {}` does reach the next statement, and is still refused here).
//   - Any `return`, including one in a `finally` that would override the jump.
//   - A `break L` or `continue L` naming a label declared *outside* the construct,
//     which is a jump past it. A label that wraps the construct is not that case:
//     the census admits the labeled statement as the construct itself, so
//     `outer: for (…) { for (…) { break outer } }` falls through.
//   - A loop condition that is truthy without being a literal (`while (fn())`,
//     `while (flag)`), which is treated as an ordinary exit edge. Only conditions
//     literalTruthiness can decide are read here; the type checker's truthiness
//     of an arbitrary expression is not consulted. Note the direction: an
//     undecided condition is treated as *having* an exit edge, so the construct
//     looks completable and the code after it keeps the reachability it already
//     had. That is the optimistic arm, retained deliberately — see the note on
//     alwaysTruthyLiteralConditionLocked.
//
// Jumps inside a nested callable are not this function's control flow and are
// skipped, exactly as the surrounding census skips them.
func (p *project) constructCompletesNormallyLocked(construct *ast.Node) bool {
	flow := p.scanConstructControlFlowLocked(construct, 0, map[*ast.Node]bool{})
	if flow.escapes || flow.nestedTraps {
		return false
	}
	return !p.loopWithoutExitEdgeLocked(construct) || flow.breaksConstruct
}

// constructTraps reports that control entering `construct` may never leave it:
// no `return`, no `throw`, no jump aimed anywhere outside it, and either a header
// that cannot end the loop or a nested construct that traps in the same way. The
// statement after whatever contains it is then not reached by falling through.
//
// It is deliberately one-sided. Over-reporting a trap costs precision only —
// today's ReachUnknown — while under-reporting it is what let `try { while (true)
// {…} } finally {…}` claim its successor was reached.
func (p *project) constructTrapsLocked(
	construct *ast.Node, depth int, memo map[*ast.Node]bool,
) bool {
	if construct == nil {
		return false
	}
	if depth > maxNestedConstructDepth {
		return true
	}
	if answer, computed := memo[construct]; computed {
		return answer
	}
	// Bound the recursion before descending: a malformed parent chain must not
	// make this re-enter the same node forever.
	memo[construct] = true
	flow := p.scanConstructControlFlowLocked(construct, depth, memo)
	answer := !flow.escapes && !flow.breaksConstruct &&
		(p.loopWithoutExitEdgeLocked(construct) || flow.nestedTraps)
	memo[construct] = answer
	return answer
}

// constructControlFlow is one construct's own classification: whether control
// leaves it upward, whether a `break` aimed at it lets control out of its bottom,
// and whether it contains a construct control can never leave.
type constructControlFlow struct {
	escapes         bool
	breaksConstruct bool
	nestedTraps     bool
}

func (p *project) scanConstructControlFlowLocked(
	construct *ast.Node,
	depth int,
	memo map[*ast.Node]bool,
) constructControlFlow {
	var flow constructControlFlow
	var scan func(*ast.Node)
	scan = func(node *ast.Node) {
		if node == nil || flow.escapes {
			return
		}
		if node != construct && isCallableDeclaration(node) {
			return
		}
		if node != construct && isNestedFallThroughConstruct(node) &&
			p.constructTrapsLocked(node, depth+1, memo) {
			flow.nestedTraps = true
		}
		switch {
		case ast.IsReturnStatement(node), ast.IsThrowStatement(node):
			flow.escapes = true
			return
		case ast.IsBreakStatement(node), isContinueStatement(node):
			switch target := jumpTargetWithin(node, construct); {
			case target == nil:
				flow.escapes = true
				return
			case target == construct && ast.IsBreakStatement(node):
				flow.breaksConstruct = true
			}
		}
		node.ForEachChild(func(child *ast.Node) bool {
			scan(child)
			return false
		})
	}
	scan(construct)
	return flow
}

// isNestedFallThroughConstruct names the nodes whose own completion has to be
// established before the construct containing them can claim to complete.
//
// A labeled loop is the construct, not the loop under the label: the census
// admits the labeled statement (ast.IsIterationStatement looks through labels)
// and loopWithoutExitEdge unwraps them, so counting the inner loop as a second
// nested construct would evaluate `break L` against the wrong construct and
// refuse a shape the label makes legitimate. A label wrapping a `try` or a
// `switch` is not admitted that way, so those are still reached through the
// label.
func isNestedFallThroughConstruct(node *ast.Node) bool {
	if node == nil {
		return false
	}
	if parent := node.Parent; parent != nil &&
		parent.KindString() == "KindLabeledStatement" &&
		parent.Statement() == node &&
		ast.IsIterationStatement(parent, true) {
		return false
	}
	return ast.IsTryStatement(node) || ast.IsIterationStatement(node, true) ||
		ast.IsSwitchStatement(node)
}

// jumpTargetWithin resolves a `break`/`continue` to the statement it transfers
// control to, and returns nil when that target is not the construct or something
// nested inside it — the definition of leaving.
//
// The search climbs parents from the jump and stops at the construct, so a target
// found on the way is necessarily the construct or a descendant. A labeled jump
// matches the nearest enclosing label of that name; an unlabeled `continue`
// matches the nearest iteration statement; an unlabeled `break` matches the
// nearest iteration or `switch` statement.
//
// No callable boundary is tested on the way up because none can be crossed: the
// only caller hands this a jump found by a scan that stops at every nested
// callable, so the jump is a descendant of the construct with no callable in
// between, and the walk terminates at the construct itself.
func jumpTargetWithin(jump *ast.Node, construct *ast.Node) *ast.Node {
	label := jumpLabelText(jump)
	continues := isContinueStatement(jump)
	for current := jump.Parent; current != nil; current = current.Parent {
		switch {
		case label != "":
			if current.KindString() == "KindLabeledStatement" &&
				current.AsLabeledStatement().Label != nil &&
				current.AsLabeledStatement().Label.Text() == label {
				return current
			}
		case continues:
			if ast.IsIterationStatement(current, false) {
				return current
			}
		default:
			if ast.IsIterationStatement(current, false) || ast.IsSwitchStatement(current) {
				return current
			}
		}
		if current == construct {
			return nil
		}
	}
	return nil
}

func jumpLabelText(jump *ast.Node) string {
	switch jump.KindString() {
	case "KindBreakStatement":
		if label := jump.AsBreakStatement().Label; label != nil {
			return label.Text()
		}
	case "KindContinueStatement":
		if label := jump.AsContinueStatement().Label; label != nil {
			return label.Text()
		}
	}
	return ""
}

func isContinueStatement(node *ast.Node) bool {
	return node != nil && node.KindString() == "KindContinueStatement"
}

// loopWithoutExitEdge names the loops whose header cannot end the loop, so the
// statement after them is reached only through a `break`. A labeled loop is
// unwrapped first, because ast.IsIterationStatement admits the label as the
// construct.
func (p *project) loopWithoutExitEdgeLocked(construct *ast.Node) bool {
	node := construct
	for range maxReturnedCallableDepth {
		if node == nil || node.KindString() != "KindLabeledStatement" {
			break
		}
		node = node.Statement()
	}
	if node == nil {
		return false
	}
	switch node.KindString() {
	case "KindWhileStatement":
		return p.alwaysTruthyLiteralConditionLocked(node.AsWhileStatement().Expression)
	case "KindDoStatement":
		return p.alwaysTruthyLiteralConditionLocked(node.AsDoStatement().Expression)
	case "KindForStatement":
		return node.AsForStatement().Condition == nil ||
			p.alwaysTruthyLiteralConditionLocked(node.AsForStatement().Condition)
	default:
		// `for … in` and `for … of` always have an exit edge — an exhausted
		// iterator — and `switch`/`try` are not loops at all.
		return false
	}
}

// alwaysTruthyLiteralConditionLocked reports whether a loop header can never end
// the loop because its condition always evaluates to true: `true`, a non-zero
// numeric literal, a non-empty string literal, a `!` applied to a literal that is
// always falsy, a `const` that uniquely names one of those, or a comparison of
// two such literals.
//
// The type checker's opinion of an arbitrary expression's truthiness is
// deliberately not consulted, and the direction of that refusal matters. An
// undecided condition is treated as *having* an exit edge, so the loop looks
// completable and the statements after it keep the reachability they already
// had. That is the optimistic arm: `while (flag) {}` where `flag` is provably
// true leaves the following code "reachable" when it is not. It is retained
// because the alternative — asking the checker whether an arbitrary expression
// is truthy — would make reachability depend on inference rather than on the
// program's own text. The two indirections below exist because they are decided
// by the text: they were the shapes that reached this in practice.
func (p *project) alwaysTruthyLiteralConditionLocked(expression *ast.Node) bool {
	truthy, known := p.literalTruthinessLocked(expression, 0)
	return known && truthy
}

// literalTruthinessLocked evaluates a condition the program spells out, reporting
// the value and whether it was decidable at all. Anything that is not decided by
// the text — a call, a template with substitutions, a reassignable binding, a
// numeric literal whose text this cannot read exactly — reports undecided.
func (p *project) literalTruthinessLocked(
	expression *ast.Node, depth int,
) (truthy bool, known bool) {
	if depth > maxNestedConstructDepth {
		return false, false
	}
	expression = identityPreservingUnwrap(expression)
	if expression == nil {
		return false, false
	}
	switch {
	case expression.Kind == ast.KindTrueKeyword:
		return true, true
	case expression.Kind == ast.KindFalseKeyword, expression.Kind == ast.KindNullKeyword:
		return false, true
	case ast.IsNumericLiteral(expression):
		// Decimal literals are read exactly; a radix prefix or a numeric
		// separator that ParseFloat refuses stays undecided rather than
		// guessing.
		value, err := strconv.ParseFloat(expression.Text(), 64)
		if err != nil {
			return false, false
		}
		return value != 0, true
	case ast.IsStringLiteral(expression), ast.IsNoSubstitutionTemplateLiteral(expression):
		return expression.Text() != "", true
	case ast.IsPrefixUnaryExpression(expression) &&
		expression.AsPrefixUnaryExpression().Operator == ast.KindExclamationToken:
		operand, decided := p.literalTruthinessLocked(
			expression.AsPrefixUnaryExpression().Operand, depth+1,
		)
		return !operand, decided
	case ast.IsIdentifier(expression):
		return p.constBindingTruthinessLocked(expression, depth)
	case ast.IsBinaryExpression(expression):
		return p.literalComparisonTruthinessLocked(expression, depth)
	}
	return false, false
}

// constBindingTruthinessLocked reads a condition that names a binding instead of
// spelling the literal. `const ALWAYS = true; while (ALWAYS) {}` is `while
// (true)`, and a census that does not see that calls the loop exitable and
// promotes the dead code after it to reachable.
//
// The gate is the one collectReturnedCallablesThroughBindingLocked already uses
// for the same reason: exactly one declaration, `const`, and never written to.
// A reassignable binding proves nothing about the value the header reads, and
// merged or repeated declarations leave no single initializer.
func (p *project) constBindingTruthinessLocked(
	identifier *ast.Node, depth int,
) (truthy bool, known bool) {
	target := p.canonicalSymbol(p.checker.GetSymbolAtLocation(identifier))
	if target == nil || len(target.Declarations) != 1 {
		return false, false
	}
	declaration := target.Declarations[0]
	if !ast.IsVariableDeclaration(declaration) || !ast.IsVarConst(declaration) ||
		p.symbolIsAssignedLocked(target, declaration) {
		return false, false
	}
	return p.literalTruthinessLocked(declaration.Initializer(), depth+1)
}

// literalComparisonTruthinessLocked decides `while (1 === 1)` and its relatives:
// a comparison whose *both* operands reduce to a primitive the text spells out.
//
// Only the four equality operators, and only between operands of the same
// primitive kind. A loose comparison of a number with a string has coercion
// rules this does not model and stays undecided, as does every relational and
// arithmetic operator.
//
// The operator kinds are matched by the compiler's own generated names because
// they are outside this repository's pinned ast shim surface — the same reason
// objectLiteralPropertyValue matches "KindPropertyAssignment" by name. An
// unrecognized spelling falls through to undecided, which is the safe arm.
func (p *project) literalComparisonTruthinessLocked(
	expression *ast.Node, depth int,
) (truthy bool, known bool) {
	binary := expression.AsBinaryExpression()
	if binary == nil || binary.OperatorToken == nil {
		return false, false
	}
	var negated bool
	switch binary.OperatorToken.KindString() {
	case "KindEqualsEqualsToken", "KindEqualsEqualsEqualsToken":
	case "KindExclamationEqualsToken", "KindExclamationEqualsEqualsToken":
		negated = true
	default:
		return false, false
	}
	left, leftKnown := p.literalComparandLocked(binary.Left, depth+1)
	right, rightKnown := p.literalComparandLocked(binary.Right, depth+1)
	if !leftKnown || !rightKnown || left.kind != right.kind {
		return false, false
	}
	equal := left == right
	return equal != negated, true
}

// literalComparand is the exact primitive value an expression spells, reduced to
// something comparable by value. `kind` separates the primitive types so that a
// cross-kind comparison is refused rather than answered by Go's own equality.
type literalComparand struct {
	kind    string
	boolean bool
	number  float64
	text    string
}

// literalComparandLocked reduces one side of a comparison to the primitive it
// spells, following the same `const` indirection the truthiness read follows.
// Anything else — a call, an object literal, a template with substitutions, a
// numeric literal ParseFloat refuses — reports undecided.
func (p *project) literalComparandLocked(
	expression *ast.Node, depth int,
) (literalComparand, bool) {
	if depth > maxNestedConstructDepth {
		return literalComparand{}, false
	}
	expression = identityPreservingUnwrap(expression)
	if expression == nil {
		return literalComparand{}, false
	}
	switch {
	case expression.Kind == ast.KindTrueKeyword:
		return literalComparand{kind: "boolean", boolean: true}, true
	case expression.Kind == ast.KindFalseKeyword:
		return literalComparand{kind: "boolean"}, true
	case expression.Kind == ast.KindNullKeyword:
		return literalComparand{kind: "null"}, true
	case ast.IsNumericLiteral(expression):
		value, err := strconv.ParseFloat(expression.Text(), 64)
		if err != nil {
			return literalComparand{}, false
		}
		return literalComparand{kind: "number", number: value}, true
	case ast.IsStringLiteral(expression), ast.IsNoSubstitutionTemplateLiteral(expression):
		return literalComparand{kind: "string", text: expression.Text()}, true
	case ast.IsIdentifier(expression):
		target := p.canonicalSymbol(p.checker.GetSymbolAtLocation(expression))
		if target == nil || len(target.Declarations) != 1 {
			return literalComparand{}, false
		}
		declaration := target.Declarations[0]
		if !ast.IsVariableDeclaration(declaration) || !ast.IsVarConst(declaration) ||
			p.symbolIsAssignedLocked(target, declaration) {
			return literalComparand{}, false
		}
		return p.literalComparandLocked(declaration.Initializer(), depth+1)
	}
	return literalComparand{}, false
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

// carriedCallableLocationsLocked reports the exact source ranges of the
// callables an expression provably carries.
//
// The descent is argument-agnostic in its core: the question "which callable
// does this expression carry" is the same one whether the expression is
// returned from an implementation or handed to one of its calls at an argument
// slot. Return sites were simply the first consumer. The one place the two
// questions part company is a value that bundles several callables, which
// [carriedCallableDescent] names; use singleCallableLocationsLocked for the
// argument-slot question.
//
// The carried value is rarely the callable itself. Real packages hand back
// `Object.assign(fn, { clear })`, `[fn, clear]`, or a `const` naming an arrow, and
// every one of those constructions preserves the callable's identity: whatever the
// caller invokes is the very function object whose body sits in one of these
// ranges. Reporting the ranges rather than a set of captured parameter indices is
// what makes the fact *bind*: a consumer asking whether a call inside some nested
// callable can be reached through the returned value answers it by naming that
// callable — the call's EnclosingCallable is one of these ranges, or it is not. A
// union of parameter indices could not answer that, because it says nothing about
// which callable mentioned the parameter, and a call in a never-returned closure
// would discharge on a returned closure that merely names the same parameter.
//
// These are ranges, not a reachability claim about everything inside them. A call
// two callables deep inside a carried closure — `setTimeout(() => callback(…), wait)`
// inside a returned debounced function — is reached by *composing* two facts, this
// range and the invoking position of `setTimeout`, and never by observing that its
// bytes nest. The intervening callable might just as well have been stored in a
// registry and never run.
//
// Absence of a range is never proof that nothing is carried: the descent is a
// whitelist and stops at the first construction it cannot vouch for.
func (p *project) carriedCallableLocationsLocked(expression *ast.Node) []typefacts.Location {
	return p.callableLocationsLocked(expression, carriedCallableDescentWholeValue)
}

// callableReturnBindingsLocked is the return-carry answer for a nested
// callable. It preserves the existing identity whitelist and adds one
// distinction the implementation-level ReturnSite could not express: a
// conditional value may carry different callable identities on its two arms.
// Those alternatives are Unknown, never Reachable, unless the condition is a
// compile-time literal and selects one arm exactly.
func (p *project) callableReturnBindingsLocked(
	expression *ast.Node,
) []typefacts.CallableCarryBinding {
	bindings := make(map[typefacts.Location]typefacts.Reachability)
	budget := maxReturnedCallableBudget
	var collect func(*ast.Node, typefacts.Reachability, int, map[*ast.Node]struct{})
	collect = func(
		candidate *ast.Node,
		reach typefacts.Reachability,
		depth int,
		visiting map[*ast.Node]struct{},
	) {
		if candidate == nil || depth > maxReturnedCallableDepth || budget <= 0 {
			return
		}
		budget--
		node := identityPreservingUnwrap(candidate)
		if node == nil {
			return
		}
		if _, cycling := visiting[node]; cycling {
			return
		}
		visiting[node] = struct{}{}
		defer delete(visiting, node)

		if locations := p.carriedCallableLocationsLocked(node); len(locations) != 0 {
			for _, location := range locations {
				previous, exists := bindings[location]
				if !exists || previous == typefacts.Unreachable || reach == typefacts.Reachable {
					bindings[location] = reach
				}
			}
			return
		}
		if ast.IsIdentifier(node) {
			target := p.canonicalSymbol(p.checker.GetSymbolAtLocation(node))
			if target == nil || len(target.Declarations) != 1 {
				return
			}
			declaration := target.Declarations[0]
			if !ast.IsVariableDeclaration(declaration) || !ast.IsVarConst(declaration) {
				return
			}
			collect(declaration.Initializer(), reach, depth+1, visiting)
			return
		}
		if !ast.IsConditionalExpression(node) {
			return
		}
		conditional := node.AsConditionalExpression()
		if truthy, known := p.literalTruthinessLocked(conditional.Condition, 0); known {
			if truthy {
				collect(conditional.WhenTrue, reach, depth+1, visiting)
			} else {
				collect(conditional.WhenFalse, reach, depth+1, visiting)
			}
			return
		}
		alternativeReach := typefacts.ReachUnknown
		if reach == typefacts.Unreachable {
			alternativeReach = typefacts.Unreachable
		}
		collect(conditional.WhenTrue, alternativeReach, depth+1, visiting)
		collect(conditional.WhenFalse, alternativeReach, depth+1, visiting)
	}
	collect(expression, typefacts.Reachable, 0, make(map[*ast.Node]struct{}))

	answer := make([]typefacts.CallableCarryBinding, 0, len(bindings))
	for location, reach := range bindings {
		answer = append(answer, typefacts.CallableCarryBinding{Location: location, Reach: reach})
	}
	sort.Slice(answer, func(i, j int) bool {
		if answer[i].Location.Path != answer[j].Location.Path {
			return answer[i].Location.Path < answer[j].Location.Path
		}
		if answer[i].Location.StartByte != answer[j].Location.StartByte {
			return answer[i].Location.StartByte < answer[j].Location.StartByte
		}
		return answer[i].Location.EndByte < answer[j].Location.EndByte
	})
	return answer
}

// singleCallableLocationsLocked is the same descent restricted to the
// constructions that carry *one* callable and carry it by identity.
//
// The two descents answer different questions and must not be shared. A
// returned value hands the caller everything it contains, so an array or object
// literal is a faithful carrier there: the caller receives every element. An
// argument slot whose runtime invokes it does not. `addEventListener` accepts an
// `EventListenerObject` and calls exactly its `handleEvent` member; crediting
// every property of that literal would assert execution of code the runtime
// never reaches, which is why the literal arms and `Object.assign` are absent
// here rather than merely discouraged.
func (p *project) singleCallableLocationsLocked(expression *ast.Node) []typefacts.Location {
	return p.callableLocationsLocked(expression, carriedCallableDescentSingleCallable)
}

func (p *project) callableLocationsLocked(
	expression *ast.Node,
	descent carriedCallableDescent,
) []typefacts.Location {
	closures := p.returnedCallablesLocked(expression, descent)
	if len(closures) == 0 {
		return nil
	}
	locations := make([]typefacts.Location, 0, len(closures))
	for _, closure := range closures {
		locations = append(locations, nodeLocation(closure))
	}
	sort.Slice(locations, func(i, j int) bool {
		if locations[i].Path != locations[j].Path {
			return locations[i].Path < locations[j].Path
		}
		if locations[i].StartByte != locations[j].StartByte {
			return locations[i].StartByte < locations[j].StartByte
		}
		return locations[i].EndByte < locations[j].EndByte
	})
	write := 0
	for read := range locations {
		if write != 0 && locations[read] == locations[write-1] {
			continue
		}
		locations[write] = locations[read]
		write++
	}
	return locations[:write]
}

// Bounds on the identity-preserving descent. Depth stops a chain of `const`
// indirections and nested literals; the node budget stops a single pathological
// literal — a thousand-element array of arrays — from making one return site cost
// the whole census. Both are deliberately generous relative to real returned
// shapes, which nest two or three levels.
const (
	maxReturnedCallableDepth  = 8
	maxReturnedCallableBudget = 256
)

// carriedCallableDescent names which of the two questions the descent is
// answering, because they differ in exactly one place: whether a construction
// that bundles several callables into one value carries them all.
type carriedCallableDescent int

const (
	// carriedCallableDescentWholeValue is the return-site question. Whatever the
	// caller receives, it receives entirely, so an array literal, an object
	// literal and `Object.assign`'s target all hand their callables on.
	carriedCallableDescentWholeValue carriedCallableDescent = iota
	// carriedCallableDescentSingleCallable is the invoking-argument question.
	// The claim being built is that a proven invoking slot *runs* the callable
	// it is given, and a bundle is not one callable: the runtime picks a member
	// and the rest never run.
	carriedCallableDescentSingleCallable
)

// returnedCallablesLocked collects the callable declarations a returned value
// provably carries, descending only through constructions that keep the
// callable's runtime identity intact:
//
//   - the expression itself, when it is a callable declaration;
//   - parentheses and the three type-only wrappers (`as`, `satisfies`, `!`), which
//     do not exist at runtime at all;
//   - an identifier whose single declaration is a `const` variable, through that
//     variable's initializer;
//   - array-literal elements and object-literal property values, which the
//     literal stores by reference;
//   - `Object.assign`'s argument 0, which the ES specification returns by
//     identity — but only when the callee resolves to the exact default-library
//     `ObjectConstructor.assign` symbol.
//
// Every other construction — a call whose result identity is unknown, a
// conditional, a spread, a shorthand property, an element access — contributes
// nothing and leaves the demand a fail-closed open premise. Nothing here reports
// absence, so refusing a construction is always the safe answer.
func (p *project) returnedCallablesLocked(
	expression *ast.Node,
	descent carriedCallableDescent,
) []*ast.Node {
	var closures []*ast.Node
	budget := maxReturnedCallableBudget
	p.collectReturnedCallablesLocked(
		expression, descent, 0, &budget, make(map[*ast.Node]struct{}), &closures,
	)
	return closures
}

func (p *project) collectReturnedCallablesLocked(
	expression *ast.Node,
	descent carriedCallableDescent,
	depth int,
	budget *int,
	visiting map[*ast.Node]struct{},
	closures *[]*ast.Node,
) {
	if expression == nil || depth > maxReturnedCallableDepth || *budget <= 0 {
		return
	}
	*budget--
	node := identityPreservingUnwrap(expression)
	if node == nil {
		return
	}
	// A `const` cycle is not expressible in running code, but it is expressible in
	// an AST the checker has already reported on, and this descent must terminate
	// on any input rather than trust that it was well-formed.
	if _, cycling := visiting[node]; cycling {
		return
	}
	visiting[node] = struct{}{}
	defer delete(visiting, node)

	if isCallableDeclaration(node) {
		*closures = append(*closures, node)
		return
	}
	if ast.IsIdentifier(node) {
		p.collectReturnedCallablesThroughBindingLocked(
			node, descent, depth, budget, visiting, closures,
		)
		return
	}
	// A bundle of callables is one value with several functions in it. The
	// caller of a returning implementation receives them all; a proven invoking
	// argument slot runs at most the one member its runtime names, so the
	// bundling arms stop here for that question.
	if descent == carriedCallableDescentSingleCallable {
		return
	}
	if ast.IsArrayLiteralExpression(node) {
		for _, element := range node.AsArrayLiteralExpression().Elements.Nodes {
			// A spread's contribution to the element positions is not fixed, so the
			// slot it feeds carries no proven callable. Sibling elements are still
			// stored by reference and remain provable on their own.
			if element == nil || ast.IsSpreadElement(element) {
				continue
			}
			p.collectReturnedCallablesLocked(element, descent, depth+1, budget, visiting, closures)
		}
		return
	}
	if ast.IsObjectLiteralExpression(node) {
		for _, property := range node.AsObjectLiteralExpression().Properties.Nodes {
			if property == nil {
				continue
			}
			if ast.IsMethodDeclaration(property) {
				*closures = append(*closures, property)
				continue
			}
			p.collectReturnedCallablesLocked(
				objectLiteralPropertyValue(property), descent, depth+1, budget, visiting, closures,
			)
		}
		return
	}
	if ast.IsCallExpression(node) && p.isDefaultLibraryObjectAssignLocked(node) {
		arguments := node.Arguments()
		if len(arguments) != 0 && !ast.IsSpreadElement(arguments[0]) {
			p.collectReturnedCallablesLocked(
				arguments[0], descent, depth+1, budget, visiting, closures,
			)
		}
	}
}

// collectReturnedCallablesThroughBindingLocked resolves one identifier to the
// callable it names.
//
// The declaration-is-callable case keeps using the symbol's value declaration, so
// an overloaded function still resolves to its implementation — but only when the
// binding is never written to. `function fn() {}; fn = () => {}; return fn` is a
// function declaration whose name no longer denotes it, and a hoisted declaration
// says nothing about which function object the binding holds at the return. The
// variable case is narrower on purpose: it demands exactly one declaration and
// `const`. A reassignable binding does not prove the returned value is the
// callable this initializer spells — the very next statement may rebind it — and
// merged or repeated declarations leave no single initializer to read. Every
// refusal emits nothing.
func (p *project) collectReturnedCallablesThroughBindingLocked(
	identifier *ast.Node,
	descent carriedCallableDescent,
	depth int,
	budget *int,
	visiting map[*ast.Node]struct{},
	closures *[]*ast.Node,
) {
	target := p.canonicalSymbol(p.checker.GetSymbolAtLocation(identifier))
	if target == nil {
		return
	}
	declaration := target.ValueDeclaration
	if declaration == nil && len(target.Declarations) == 1 {
		declaration = target.Declarations[0]
	}
	if declaration == nil {
		return
	}
	if isCallableDeclaration(declaration) {
		if p.symbolIsAssignedLocked(target, declaration) {
			return
		}
		*closures = append(*closures, declaration)
		return
	}
	if len(target.Declarations) != 1 ||
		!ast.IsVariableDeclaration(declaration) ||
		!ast.IsVarConst(declaration) {
		return
	}
	p.collectReturnedCallablesLocked(
		declaration.Initializer(), descent, depth+1, budget, visiting, closures,
	)
}

// symbolIsAssignedLocked reports whether the file declaring `declaration` writes
// to `target` anywhere — `fn = …`, a compound assignment, `fn++`, or a
// destructuring pattern that names it.
//
// A hoisted `function fn() {}` binds a mutable variable, so its declaration is
// not by itself proof that `fn` denotes that body at a later return. The question
// is answered from the compiler's own ast.GetAssignmentTarget rather than from a
// name scan, so a shadowing inner `fn` in the same file is not mistaken for a
// write to this one.
//
// Scanning the declaring file alone is exact for the shapes that matter: an
// imported binding is read-only, and TypeScript refuses an assignment to one.
// The remaining hole is a `declare global` value augmented and written from
// another file, which no descent here vouches for anyway.
func (p *project) symbolIsAssignedLocked(target *ast.Symbol, declaration *ast.Node) bool {
	sourceFile := ast.GetSourceFileOfNode(declaration)
	if sourceFile == nil {
		return true
	}
	assigned, computed := p.assignedSymbols[sourceFile]
	if !computed {
		assigned = p.assignmentTargetSymbolsLocked(sourceFile)
		if p.assignedSymbols == nil {
			p.assignedSymbols = make(map[*ast.SourceFile]map[*ast.Symbol]struct{})
		}
		p.assignedSymbols[sourceFile] = assigned
	}
	_, written := assigned[target]
	return written
}

// isAssignmentTargetIdentifier reports whether an identifier is written by an
// assignment.
//
// It is GetAssignmentTarget alone, and the point is what it is *not* asked
// beside: the declaration-name filter. `({ current } = other)` writes
// `current`, and the identifier there is a ShorthandPropertyAssignment's name,
// which the compiler calls a declaration name — so a scan that skipped
// declaration names first called such a binding **unwritten**, which is the
// premise ADR 0034 and everything built on it rest on. The filter is also
// redundant: an ordinary declaration name is not an assignment target, so
// dropping it changes exactly the destructuring-assignment shapes and nothing
// else.
func isAssignmentTargetIdentifier(node *ast.Node) bool {
	return ast.GetAssignmentTarget(node) != nil
}

// assignedBindingSymbol resolves the **variable** an assignment-target
// identifier writes.
//
// For a shorthand property assignment inside a destructuring assignment,
// GetSymbolAtLocation answers the object literal's *property* symbol, not the
// variable, so a scan that used it alone matched nothing and called the
// binding unwritten. The checker has a dedicated resolution for that shape and
// this is where it belongs.
func (p *project) assignedBindingSymbol(fileChecker *checker.Checker, node *ast.Node) *ast.Symbol {
	if node.Parent != nil && ast.IsShorthandPropertyAssignment(node.Parent) {
		if symbol := checker.Checker_GetShorthandAssignmentValueSymbol(fileChecker, node.Parent); symbol != nil {
			return p.canonicalSymbol(symbol)
		}
	}
	return p.canonicalSymbol(fileChecker.GetSymbolAtLocation(node))
}

func (p *project) assignmentTargetSymbolsLocked(
	sourceFile *ast.SourceFile,
) map[*ast.Symbol]struct{} {
	assigned := make(map[*ast.Symbol]struct{})
	// The file's own checker: a premise twin's file (ADR 0038) was bound by the
	// twin program, every other file by the accepted one.
	fileChecker := p.checker
	if p.formTwin != nil && sourceFile == p.formTwin.file {
		fileChecker = p.formTwin.checker
	}
	var visit func(*ast.Node)
	visit = func(node *ast.Node) {
		if node == nil {
			return
		}
		// The declaration-name filter is asked *after* the assignment test, not
		// before it: a shorthand destructuring-assignment target
		// (`({ current } = other)`) is a declaration name by the compiler's
		// reckoning and a write by the language's.
		if ast.IsIdentifier(node) && !ast.IsPartOfTypeNode(node) &&
			isAssignmentTargetIdentifier(node) {
			if symbol := p.assignedBindingSymbol(fileChecker, node); symbol != nil {
				assigned[symbol] = struct{}{}
			}
		}
		node.ForEachChild(func(child *ast.Node) bool {
			visit(child)
			return false
		})
	}
	visit(sourceFile.AsNode())
	return assigned
}

// identityPreservingUnwrap strips the wrappers that produce no runtime value of
// their own: parentheses, and the `as` / `satisfies` / non-null assertions, all of
// which the compiler erases. The value inside is bit-for-bit the value outside.
func identityPreservingUnwrap(node *ast.Node) *ast.Node {
	for range maxReturnedCallableDepth {
		switch {
		case node == nil:
			return nil
		case ast.IsParenthesizedExpression(node),
			ast.IsAsExpression(node),
			ast.IsSatisfiesExpression(node),
			ast.IsNonNullExpression(node):
			node = node.Expression()
		default:
			return node
		}
	}
	return node
}

// objectLiteralPropertyValue is a property assignment's value expression, and nil
// for every other object-literal element kind.
//
// The kind is matched by the compiler's own generated name because
// ast.IsPropertyAssignment is outside this repository's pinned shim surface, and
// the gate has to be exact rather than best-effort: Node.Initializer() panics for
// the shorthand, spread and accessor kinds this rejects. Those three are refused
// on their merits too — a shorthand's name resolves to the literal's own property
// symbol rather than to the local binding it copies, a spread contributes an
// unfixed set of properties, and an accessor is a call rather than a stored
// reference.
func objectLiteralPropertyValue(property *ast.Node) *ast.Node {
	if property == nil || property.KindString() != "KindPropertyAssignment" {
		return nil
	}
	return property.Initializer()
}

// isDefaultLibraryObjectAssignLocked proves a call's callee is exactly the
// standard library's `Object.assign`. ES2015 §19.1.2.1 returns the target — its
// step 5 returns `to`, the first argument, by identity — so argument 0 of such a
// call is the value the call yields, and descending into it preserves identity.
//
// That guarantee belongs to one symbol, not to a spelling. A shadowed local
// `Object`, a hand-written helper of the same name, or a `declare global`
// augmentation of `ObjectConstructor` owns declarations outside the default
// library and is refused. `Object["assign"](…)` is refused too: only a property
// access is matched here, which is what the guarantee was verified against.
func (p *project) isDefaultLibraryObjectAssignLocked(call *ast.Node) bool {
	callee := call.Expression()
	if callee == nil || !ast.IsPropertyAccessExpression(callee) {
		return false
	}
	name := callee.Name()
	if name == nil || !ast.IsIdentifier(name) || name.Text() != "assign" {
		return false
	}
	receiver := callee.Expression()
	if receiver == nil || !ast.IsIdentifier(receiver) || receiver.Text() != "Object" {
		return false
	}
	return p.isDefaultLibrarySymbolLocked(p.checker.GetSymbolAtLocation(receiver), "Object", "") &&
		p.isDefaultLibrarySymbolLocked(
			p.checker.GetSymbolAtLocation(name), "assign", "ObjectConstructor",
		)
}

// isDefaultLibrarySymbolLocked requires the symbol to carry exactly `name`, to own
// at least one declaration, and for *every* declaration to sit in a file the
// compiler itself considers a default library — so a single user-file
// augmentation is enough to refuse the whole symbol. A non-empty `container` also
// requires every declaration's parent to be the named interface.
//
// The quantifier has a reachable negative case and a test that pins it: a
// `declare global { interface ObjectConstructor { assign… } }` augmentation.
// The container arm does not. Given the receiver check that precedes the only
// call passing a container, `Object.assign` cannot resolve to a default-library
// `assign` declared anywhere but `ObjectConstructor`, so no TypeScript source
// distinguishes this arm from its removal. It is kept as an explicit statement
// of what was verified rather than as a filter something reaches today.
func (p *project) isDefaultLibrarySymbolLocked(
	symbol *ast.Symbol,
	name string,
	container string,
) bool {
	symbol = p.canonicalSymbol(symbol)
	if symbol == nil || symbol.Name != name || len(symbol.Declarations) == 0 {
		return false
	}
	for _, declaration := range symbol.Declarations {
		sourceFile := ast.GetSourceFileOfNode(declaration)
		if sourceFile == nil || !p.program.IsSourceFileDefaultLibrary(sourceFile.Path()) {
			return false
		}
		if container == "" {
			continue
		}
		parent := declaration.Parent
		if parent == nil || parent.Name() == nil || parent.Name().Text() != container {
			return false
		}
	}
	return true
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
