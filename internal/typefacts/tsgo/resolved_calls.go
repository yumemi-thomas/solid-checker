package tsgo

import (
	"context"
	"path/filepath"
	"sort"
	"strings"

	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/microsoft/typescript-go/shim/scanner"
	"github.com/yumemi-thomas/solid-ts-facts/internal/typefacts"
)

func isCallLikeExpression(node *ast.Node) bool {
	return ast.IsCallExpression(node) || ast.IsNewExpression(node)
}

type resolvedDeclarationCacheKey struct {
	signature *checker.Signature
	fallback  *ast.Symbol
}

type resolvedParameterCacheKey struct {
	signature     *checker.Signature
	argumentIndex int
}

// resolvedCallDemand is the private seam between semantic location lookup and
// resolved-call construction. node is already normalized to a call or new
// expression and never escapes the checker generation.
type resolvedCallDemand struct {
	entityIndex   int
	node          *ast.Node
	argumentCount int
}

// semanticEvidence is built alongside facts so retained-state ownership does
// not require a second walk through every nested call and descriptor.
type semanticEvidence struct {
	path         string
	dependencies []string
	durable      bool
}

func newSemanticEvidence(path string) semanticEvidence {
	return semanticEvidence{path: path, durable: true}
}

func (p *project) currentSignatureDeclaration(signature *checker.Signature, target *ast.Symbol) *ast.Node {
	declaration := signature.Declaration()
	if declaration == nil {
		return nil
	}
	sourceFile := ast.GetSourceFileOfNode(declaration)
	if sourceFile == nil {
		return nil
	}
	if p.isCurrentSourceFile(sourceFile) {
		return declaration
	}
	target = p.canonicalSymbol(target)
	if target == nil {
		return nil
	}

	ordinal := 0
	if declarationSymbol := declaration.Symbol(); declarationSymbol != nil {
		for _, candidate := range declarationSymbol.Declarations {
			if candidate.Kind != declaration.Kind {
				continue
			}
			if candidate == declaration {
				break
			}
			ordinal++
		}
	}
	matches := make([]*ast.Node, 0, ordinal+1)
	for _, root := range target.Declarations {
		if root.Kind == declaration.Kind {
			matches = append(matches, root)
			continue
		}
		var visit func(*ast.Node) bool
		visit = func(node *ast.Node) bool {
			if node.Kind == declaration.Kind {
				matches = append(matches, node)
				return false
			}
			node.ForEachChild(visit)
			return false
		}
		root.ForEachChild(visit)
	}
	if ordinal < len(matches) {
		return matches[ordinal]
	}
	return nil
}

func (p *project) isCurrentSourceFile(sourceFile *ast.SourceFile) bool {
	if p.currentSourceFiles == nil {
		p.currentSourceFiles = make(map[*ast.SourceFile]struct{}, len(p.program.SourceFiles()))
		for _, current := range p.program.SourceFiles() {
			p.currentSourceFiles[current] = struct{}{}
		}
	}
	_, current := p.currentSourceFiles[sourceFile]
	return current
}

func (p *project) resolvedDeclaration(signature *checker.Signature, node *ast.Node, fallbackSymbol *ast.Symbol) *typefacts.ResolvedDeclaration {
	key := resolvedDeclarationCacheKey{signature: signature, fallback: fallbackSymbol}
	if cached := p.resolvedDeclarations[key]; cached != nil {
		return cached
	}
	if node == nil {
		return nil
	}
	sourceFile := ast.GetSourceFileOfNode(node)
	if sourceFile == nil {
		return nil
	}
	nameNode := node.Name()
	if nameNode == nil {
		nameNode = node
	}
	symbol := node.Symbol()
	if (ast.IsArrowFunction(node) || ast.IsFunctionExpression(node)) && fallbackSymbol != nil {
		// Anonymous function expressions often carry an internal signature
		// symbol. The callable declaration exposed to consumers is the
		// compiler-resolved callee symbol that owns that expression.
		symbol = fallbackSymbol
	}
	if symbol == nil && node.Name() != nil {
		symbol = p.checker.GetSymbolAtLocation(node.Name())
	}
	if symbol == nil {
		symbol = fallbackSymbol
	}
	kind := strings.TrimPrefix(node.KindString(), "Kind")
	result := &typefacts.ResolvedDeclaration{
		Name:       "",
		Kind:       kind,
		Location:   typefacts.Location{Path: filepath.Clean(sourceFile.FileName()), StartByte: scanner.SkipTrivia(sourceFile.Text(), nameNode.Pos()), EndByte: nameNode.End()},
		SourceFile: filepath.Clean(sourceFile.FileName()),
	}
	if symbol != nil {
		symbol = p.canonicalSymbol(symbol)
		result.Symbol = p.idFor(symbol)
		result.OriginModule = declarationModule(symbol)
		if !strings.HasPrefix(symbol.Name, ast.InternalSymbolNamePrefix) {
			result.Name = symbol.Name
		}
	}
	switch kind {
	case "Constructor":
		result.Name = "constructor"
	case "ArrowFunction", "FunctionExpression":
		if result.Name == "" {
			result.Name = "call"
		}
	case "CallSignature", "FunctionType":
		result.Name = "call"
	case "ConstructSignature", "ConstructorType":
		result.Name = "construct"
	}
	for owner := node.Parent; owner != nil && owner.Parent != nil; owner = owner.Parent {
		ownerSymbol := owner.Symbol()
		if ownerSymbol == nil && owner.Name() != nil {
			ownerSymbol = p.checker.GetSymbolAtLocation(owner.Name())
		}
		if ownerSymbol == nil {
			continue
		}
		ownerSource := ast.GetSourceFileOfNode(owner)
		if ownerSource == nil {
			continue
		}
		ownerSymbol = p.canonicalSymbol(ownerSymbol)
		if ownerSymbol == symbol {
			continue
		}
		name := owner.Name()
		if name == nil {
			name = owner
		}
		result.Owners = append(result.Owners, typefacts.DeclarationOwner{
			Symbol: p.idFor(ownerSymbol),
			Name:   ownerSymbol.Name,
			Kind:   strings.TrimPrefix(owner.KindString(), "Kind"),
			Location: typefacts.Location{
				Path:      filepath.Clean(ownerSource.FileName()),
				StartByte: scanner.SkipTrivia(ownerSource.Text(), name.Pos()),
				EndByte:   name.End(),
			},
		})
	}
	for left, right := 0, len(result.Owners)-1; left < right; left, right = left+1, right-1 {
		result.Owners[left], result.Owners[right] = result.Owners[right], result.Owners[left]
	}
	qualified := make([]string, 0, len(result.Owners)+1)
	for _, owner := range result.Owners {
		if owner.Name != "" && !strings.HasPrefix(owner.Name, ast.InternalSymbolNamePrefix) {
			qualified = append(qualified, owner.Name)
		}
	}
	if result.Name != "" {
		qualified = append(qualified, result.Name)
	}
	result.QualifiedName = strings.Join(qualified, ".")
	result.StandardLibrary = p.program.IsSourceFileDefaultLibrary(sourceFile.Path())
	if p.resolvedDeclarations == nil {
		p.resolvedDeclarations = make(map[resolvedDeclarationCacheKey]*typefacts.ResolvedDeclaration)
	}
	p.resolvedDeclarations[key] = result
	return result
}

func (e *semanticEvidence) symbol(id typefacts.SymbolID) {
	if !typefacts.DurableSymbolID(id) {
		e.durable = false
	}
}

func (e *semanticEvidence) dependency(location typefacts.Location) {
	if location.Path == "" {
		return
	}
	path := location.Path
	if path != e.path {
		e.dependencies = append(e.dependencies, path)
	}
}

func (e *semanticEvidence) descriptor(descriptor *typefacts.TypeDescriptor) {
	if descriptor == nil {
		return
	}
	for _, declaration := range descriptor.AliasDeclarations {
		e.dependency(declaration.Location)
	}
}

func (e *semanticEvidence) declaration(declaration *typefacts.ResolvedDeclaration) {
	if declaration == nil {
		return
	}
	e.symbol(declaration.Symbol)
	e.dependency(declaration.Location)
	for _, owner := range declaration.Owners {
		e.symbol(owner.Symbol)
		e.dependency(owner.Location)
	}
}

func (e *semanticEvidence) parameter(parameter *typefacts.ParameterFact) {
	if parameter == nil {
		return
	}
	e.symbol(parameter.Symbol)
	if parameter.Declaration != nil {
		e.dependency(parameter.Declaration.Location)
	}
	e.descriptor(parameter.TypeDescriptor)
}

func (e *semanticEvidence) finish() ([]string, bool) {
	if len(e.dependencies) == 0 {
		return nil, e.durable
	}
	sort.Strings(e.dependencies)
	write := 1
	for read := 1; read < len(e.dependencies); read++ {
		if e.dependencies[read] == e.dependencies[write-1] {
			continue
		}
		e.dependencies[write] = e.dependencies[read]
		write++
	}
	e.dependencies = e.dependencies[:write:write]
	return e.dependencies, e.durable
}

// resolveCallRunLocked resolves every call demand in one Semantic demand run.
// Calls and argument mappings live in two exact run-owned arenas; returned
// facts are immutable and therefore safe for retained contributions.
func (p *project) resolveCallRunLocked(
	ctx context.Context,
	sourceFile *ast.SourceFile,
	demands []resolvedCallDemand,
	entities []typefacts.EntityFact,
	evidence *semanticEvidence,
) error {
	totalArguments := 0
	for index := range demands {
		totalArguments += demands[index].argumentCount
	}
	calls := make([]typefacts.Call, len(demands))
	mappings := make([]typefacts.ArgumentMapping, totalArguments)
	mappingOffset := 0

	for demandIndex := range demands {
		if err := ctx.Err(); err != nil {
			return err
		}
		demand := &demands[demandIndex]
		call := &calls[demandIndex]
		call.Validity = typefacts.ResolvedCallUnresolved
		call.Kind = typefacts.CallKindUnknown
		entities[demand.entityIndex].ResolvedCall = call
		if demand.node == nil {
			continue
		}

		node := demand.node
		if ast.IsNewExpression(node) {
			call.Kind = typefacts.CallKindConstruct
		} else {
			call.Kind = typefacts.CallKindCall
		}
		argumentEnd := mappingOffset + demand.argumentCount
		call.Arguments = mappings[mappingOffset:argumentEnd:argumentEnd]
		mappingOffset = argumentEnd

		callee := node.Expression()
		target := p.checker.GetSymbolAtLocation(callee)
		signature := checker.Checker_getResolvedSignature(p.checker, node, nil, checker.CheckModeNormal)
		if target != nil {
			target = p.canonicalSymbol(target)
			call.Target = p.idFor(target)
			evidence.symbol(call.Target)
			if current, ok := p.symbolFor(call.Target); ok {
				target = current
			}
		}
		if signature == nil {
			fillUnresolvedArgumentMappings(call.Arguments, typefacts.ArgumentMappingCallUnresolved)
			continue
		}

		var calleeType *checker.Type
		if signature.Flags()&checker.SignatureFlagsIsSignatureCandidateForOverloadFailure != 0 &&
			!resolvedCallHasSpreadArgument(node) {
			call.Validity = typefacts.ResolvedCallRecovery
		} else if p.resolvedCallNeedsDiagnosticsBeforeCalleeType(node) {
			validity, err := p.resolvedCallDiagnosticValidityLocked(ctx, sourceFile, node)
			if err != nil {
				return err
			}
			call.Validity = validity
			if validity == typefacts.ResolvedCallValid {
				calleeType = p.checker.GetTypeAtLocation(callee)
			}
		} else {
			calleeType = p.checker.GetTypeAtLocation(callee)
			validity, err := p.resolvedCallValidityLocked(ctx, sourceFile, node, signature, target, calleeType)
			if err != nil {
				return err
			}
			call.Validity = validity
		}
		if returnType := checker.Checker_getReturnTypeOfSignature(p.checker, signature); returnType != nil {
			call.ReturnTypeText = p.typeDescriptorFor(returnType).Text
		}
		if call.Validity != typefacts.ResolvedCallValid {
			fillUnresolvedArgumentMappings(call.Arguments, typefacts.ArgumentMappingRecoverySignature)
			continue
		}
		if calleeType != nil && calleeType.Flags()&checker.TypeFlagsUnion != 0 {
			call.Targets = p.unionCallTargetsLocked(node, calleeType, evidence)
			fillUnresolvedArgumentMappings(call.Arguments, typefacts.ArgumentMappingCompositeSignature)
			continue
		}

		declaration := p.currentSignatureDeclaration(signature, target)
		call.Declaration = p.resolvedDeclaration(signature, declaration, target)
		evidence.declaration(call.Declaration)
		p.fillArgumentMappingsLocked(call.Arguments, node, signature, declaration, evidence)
	}
	return nil
}

// isExactCallableImplementationKind names declaration kinds that identify one
// runtime callable implementation. Type-level signature declarations —
// interface members, function types, call/construct/index signatures — only
// describe a structural shape, so they never become dispatch candidates.
func isExactCallableImplementationKind(kind string) bool {
	switch kind {
	case "FunctionDeclaration", "MethodDeclaration", "ArrowFunction", "FunctionExpression", "Constructor":
		return true
	default:
		return false
	}
}

// unionCallTargetsLocked derives the exact candidate declarations of a valid
// call whose callee type is a union. The set is emitted only when the
// compiler proves it exhaustive: every union constituent is one closed
// concrete callable and every one of its call (or construct) signatures
// names one exact implementation declaration with a canonical symbol. Any
// open, recovery, type-level, or declaration-less constituent leaves the
// composite call without a target set; an incomplete candidate set is never
// emitted, because it would read as the complete runtime dispatch set.
func (p *project) unionCallTargetsLocked(
	node *ast.Node,
	calleeType *checker.Type,
	evidence *semanticEvidence,
) *typefacts.CallTargetSet {
	signatureKind := checker.SignatureKindCall
	if ast.IsNewExpression(node) {
		signatureKind = checker.SignatureKindConstruct
	}
	// Instantiable covers unconstrained and constrained generics: a
	// constraint bounds behavior structurally but does not enumerate exact
	// implementations, so any generic constituent voids the proof.
	const openFlags = checker.TypeFlagsAny |
		checker.TypeFlagsUnknown |
		checker.TypeFlagsNever |
		checker.TypeFlagsUndefined |
		checker.TypeFlagsNull |
		checker.TypeFlagsInstantiable |
		checker.TypeFlagsUnion |
		checker.TypeFlagsIntersection |
		checker.TypeFlagsIncludesError
	var candidates []typefacts.ResolvedDeclaration
	for _, constituent := range calleeType.Types() {
		if constituent == nil || constituent.Flags()&openFlags != 0 {
			return nil
		}
		signatures := p.checker.GetSignaturesOfType(constituent, signatureKind)
		if len(signatures) == 0 {
			return nil
		}
		target := constituent.Symbol()
		for _, signature := range signatures {
			if signature == nil ||
				signature.Flags()&checker.SignatureFlagsIsSignatureCandidateForOverloadFailure != 0 {
				return nil
			}
			declaration := p.currentSignatureDeclaration(signature, target)
			if declaration == nil {
				return nil
			}
			resolved := p.resolvedDeclaration(signature, declaration, target)
			if resolved == nil ||
				resolved.Symbol == "" ||
				!isExactCallableImplementationKind(resolved.Kind) {
				return nil
			}
			candidates = append(candidates, *resolved)
		}
	}
	if len(candidates) == 0 {
		return nil
	}
	sort.Slice(candidates, func(left, right int) bool {
		if candidates[left].Location.Path != candidates[right].Location.Path {
			return candidates[left].Location.Path < candidates[right].Location.Path
		}
		if candidates[left].Location.StartByte != candidates[right].Location.StartByte {
			return candidates[left].Location.StartByte < candidates[right].Location.StartByte
		}
		if candidates[left].Location.EndByte != candidates[right].Location.EndByte {
			return candidates[left].Location.EndByte < candidates[right].Location.EndByte
		}
		return candidates[left].Symbol < candidates[right].Symbol
	})
	write := 1
	for read := 1; read < len(candidates); read++ {
		previous := &candidates[write-1]
		if candidates[read].Symbol == previous.Symbol && candidates[read].Location == previous.Location {
			continue
		}
		candidates[write] = candidates[read]
		write++
	}
	candidates = candidates[:write:write]
	for index := range candidates {
		evidence.declaration(&candidates[index])
	}
	return &typefacts.CallTargetSet{Exhaustive: true, Candidates: candidates}
}

func resolvedCallHasSpreadArgument(node *ast.Node) bool {
	for _, argument := range node.Arguments() {
		if ast.IsSpreadElement(argument) {
			return true
		}
	}
	return false
}

func (p *project) resolvedCallNeedsDiagnosticsBeforeCalleeType(node *ast.Node) bool {
	if ast.IsNewExpression(node) || len(node.TypeArguments()) != 0 {
		return true
	}
	for _, argument := range node.Arguments() {
		if ast.IsSpreadElement(argument) ||
			checker.Checker_isContextSensitive(p.checker, argument) {
			return true
		}
	}
	return false
}

// resolvedCallValidityLocked keeps whole-file diagnostics behind a conservative
// fallback. Ordinary calls are proven from concrete signature, arity and
// assignability evidence. Nullable, union, context-sensitive, explicit-generic,
// construct and declaration-less calls retain the exact diagnostic path because
// their selected signature alone is not proof.
func (p *project) resolvedCallValidityLocked(
	ctx context.Context,
	sourceFile *ast.SourceFile,
	node *ast.Node,
	signature *checker.Signature,
	target *ast.Symbol,
	calleeType *checker.Type,
) (typefacts.ResolvedCallValidity, error) {
	if proof := p.proveResolvedCallValidity(node, signature, target, calleeType); proof != "" {
		return proof, nil
	}
	return p.resolvedCallDiagnosticValidityLocked(ctx, sourceFile, node)
}

func (p *project) resolvedCallDiagnosticValidityLocked(
	ctx context.Context,
	sourceFile *ast.SourceFile,
	node *ast.Node,
) (typefacts.ResolvedCallValidity, error) {
	diagnostics, err := p.callDiagnosticIndexLocked(ctx, sourceFile)
	if err != nil {
		return "", err
	}
	if _, failed := diagnostics[node]; failed {
		return typefacts.ResolvedCallRecovery, nil
	}
	return typefacts.ResolvedCallValid, nil
}

func (p *project) proveResolvedCallValidity(
	node *ast.Node,
	signature *checker.Signature,
	target *ast.Symbol,
	calleeType *checker.Type,
) typefacts.ResolvedCallValidity {
	if signature.Declaration() == nil || target == nil || calleeType == nil {
		return ""
	}
	const uncertain = checker.TypeFlagsAny |
		checker.TypeFlagsUnknown |
		checker.TypeFlagsNever |
		checker.TypeFlagsUndefined |
		checker.TypeFlagsNull |
		checker.TypeFlagsUnion |
		checker.TypeFlagsIncludesError
	if calleeType.Flags()&uncertain != 0 {
		return ""
	}
	arguments := node.Arguments()
	if len(arguments) < signature.MinArgumentCount() ||
		(!signature.HasRestParameter() && len(arguments) > len(signature.Parameters())) {
		return typefacts.ResolvedCallRecovery
	}
	for index, argument := range arguments {
		argumentType := p.checker.GetTypeAtLocation(argument)
		parameterType := checker.Checker_getTypeAtPosition(p.checker, signature, index)
		if argumentType == nil || parameterType == nil {
			return ""
		}
		if !p.checker.IsTypeAssignableTo(argumentType, parameterType) {
			return typefacts.ResolvedCallRecovery
		}
	}
	return typefacts.ResolvedCallValid
}

func (p *project) fillArgumentMappingsLocked(
	result []typefacts.ArgumentMapping,
	call *ast.Node,
	signature *checker.Signature,
	signatureDeclaration *ast.Node,
	evidence *semanticEvidence,
) {
	arguments := call.Arguments()
	parameters := signature.Parameters()
	var currentParameters []*ast.Node
	if signatureDeclaration != nil {
		currentParameters = signatureDeclaration.Parameters()
	}
	for argumentIndex := range result {
		mapping := &result[argumentIndex]
		mapping.ArgumentIndex = argumentIndex
		if ast.IsSpreadElement(arguments[argumentIndex]) {
			mapping.Status = typefacts.ArgumentMappingUnresolved
			mapping.Unresolved = typefacts.ArgumentMappingSpreadArgument
			continue
		}
		if cached := p.resolvedParameters[resolvedParameterCacheKey{
			signature:     signature,
			argumentIndex: argumentIndex,
		}]; cached != nil {
			mapping.Status = typefacts.ArgumentMappingResolved
			mapping.Parameter = cached
			evidence.parameter(cached)
			continue
		}

		formalIndex := argumentIndex
		if formalIndex >= len(parameters) {
			if !signature.HasRestParameter() || len(parameters) == 0 {
				mapping.Status = typefacts.ArgumentMappingUnresolved
				mapping.Unresolved = typefacts.ArgumentMappingParameterUnavailable
				continue
			}
			formalIndex = len(parameters) - 1
		}
		parameter := parameters[formalIndex]
		if formalIndex < len(currentParameters) {
			currentParameter := currentParameters[formalIndex]
			if currentParameter.Symbol() != nil {
				parameter = currentParameter.Symbol()
			} else if currentParameter.Name() != nil {
				if symbol := p.checker.GetSymbolAtLocation(currentParameter.Name()); symbol != nil {
					parameter = symbol
				}
			}
		}
		parameterType := checker.Checker_getTypeAtPosition(p.checker, signature, argumentIndex)
		rest := signature.HasRestParameter() && formalIndex == len(parameters)-1
		fact := &typefacts.ParameterFact{
			Index:       formalIndex,
			Rest:        rest,
			Optional:    !rest && formalIndex >= signature.MinArgumentCount(),
			Callability: callabilityOfType(p.checker, parameterType),
		}
		if parameter != nil {
			fact.Optional = fact.Optional || !rest && parameter.Flags&ast.SymbolFlagsOptional != 0
			fact.Symbol = p.idFor(parameter)
			if declarations := declarationsForSymbol(parameter); len(declarations) != 0 {
				fact.Declaration = &declarations[0]
			}
		}
		if parameterType != nil {
			fact.TypeDescriptor = p.typeDescriptorFor(parameterType)
		}
		if p.resolvedParameters == nil {
			p.resolvedParameters = make(map[resolvedParameterCacheKey]*typefacts.ParameterFact)
		}
		p.resolvedParameters[resolvedParameterCacheKey{
			signature:     signature,
			argumentIndex: argumentIndex,
		}] = fact
		mapping.Status = typefacts.ArgumentMappingResolved
		mapping.Parameter = fact
		evidence.parameter(fact)
	}
}

func fillUnresolvedArgumentMappings(result []typefacts.ArgumentMapping, reason typefacts.ArgumentMappingReason) {
	for index := range result {
		result[index] = typefacts.ArgumentMapping{
			ArgumentIndex: index,
			Status:        typefacts.ArgumentMappingUnresolved,
			Unresolved:    reason,
		}
	}
}

type callDiagnosticIndex map[*ast.Node]struct{}

func (p *project) callDiagnosticIndexLocked(ctx context.Context, sourceFile *ast.SourceFile) (callDiagnosticIndex, error) {
	if index, loaded := p.callDiagnostics[sourceFile]; loaded {
		return index, nil
	}
	diagnostics := p.program.GetSemanticDiagnostics(ctx, sourceFile)
	if err := ctx.Err(); err != nil {
		return nil, err
	}
	var index callDiagnosticIndex
	for _, diagnostic := range diagnostics {
		if !isCallResolutionDiagnostic(diagnostic.Code()) || diagnostic.Pos() < 0 || diagnostic.End() < diagnostic.Pos() {
			continue
		}
		node := deepestNodeAt(ast.GetNodeAtPosition(sourceFile, diagnostic.Pos(), false), diagnostic.Pos())
		for node != nil && !isCallLikeExpression(node) {
			node = node.Parent
		}
		if node == nil || diagnostic.Pos() < node.Pos() || diagnostic.End() > node.End() {
			continue
		}
		if index == nil {
			index = make(callDiagnosticIndex)
		}
		index[node] = struct{}{}
	}
	if err := ctx.Err(); err != nil {
		return nil, err
	}
	if p.callDiagnostics == nil {
		p.callDiagnostics = make(map[*ast.SourceFile]callDiagnosticIndex)
	}
	p.callDiagnostics[sourceFile] = index
	return index, nil
}

func isCallResolutionDiagnostic(code int32) bool {
	switch code {
	case 2344, 2345, 2348, 2349, 2350, 2379, 2554, 2555, 2558, 2635,
		2677, 2721, 2722, 2723, 2757, 2769, 2794, 2810, 6234:
		return true
	default:
		return false
	}
}
