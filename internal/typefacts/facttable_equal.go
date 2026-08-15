package typefacts

import "slices"

// Hand-written row equality for the delta diffs. reflect.DeepEqual used to do
// this work reflectively, allocating per comparison and — unlike these —
// distinguishing nil slices from empty ones, a distinction the wire encoders
// erase anyway.

// locationsEqual short-circuits on shared backing before comparing elements:
// retained rows share their slices with the previous generation's table, so
// unchanged lists are usually pointer-identical.
func locationsEqual(left, right []Location) bool {
	if len(left) != len(right) {
		return false
	}
	if len(left) == 0 || &left[0] == &right[0] {
		return true
	}
	return slices.Equal(left, right)
}

func symbolFactEqual(left, right SymbolFact) bool {
	return left.ID == right.ID &&
		left.AliasTarget == right.AliasTarget &&
		slices.Equal(left.Declarations, right.Declarations) &&
		locationsEqual(left.References, right.References)
}

func typeDescriptorEqual(left, right *TypeDescriptor) bool {
	if left == nil || right == nil {
		return left == right
	}
	return left.Text == right.Text &&
		left.OriginModule == right.OriginModule &&
		slices.Equal(left.AliasDeclarations, right.AliasDeclarations)
}

func resolvedCallEqual(left, right *Call) bool {
	if left == nil || right == nil {
		return left == right
	}
	return left.Target == right.Target &&
		left.ReturnTypeText == right.ReturnTypeText &&
		left.Validity == right.Validity &&
		left.Kind == right.Kind &&
		resolvedDeclarationEqual(left.Declaration, right.Declaration) &&
		callTargetSetEqual(left.Targets, right.Targets) &&
		slices.EqualFunc(left.Arguments, right.Arguments, argumentMappingEqual)
}

func callTargetSetEqual(left, right *CallTargetSet) bool {
	if left == nil || right == nil {
		return left == right
	}
	return left.Exhaustive == right.Exhaustive &&
		slices.EqualFunc(left.Candidates, right.Candidates, func(l, r ResolvedDeclaration) bool {
			return resolvedDeclarationEqual(&l, &r)
		})
}

func resolvedDeclarationEqual(left, right *ResolvedDeclaration) bool {
	if left == nil || right == nil {
		return left == right
	}
	return left.Symbol == right.Symbol &&
		left.Name == right.Name &&
		left.Kind == right.Kind &&
		left.Location == right.Location &&
		left.QualifiedName == right.QualifiedName &&
		left.OriginModule == right.OriginModule &&
		left.SourceFile == right.SourceFile &&
		left.StandardLibrary == right.StandardLibrary &&
		slices.Equal(left.Owners, right.Owners)
}

func parameterFactEqual(left, right *ParameterFact) bool {
	if left == nil || right == nil {
		return left == right
	}
	if left.Index != right.Index ||
		left.Symbol != right.Symbol ||
		left.Rest != right.Rest ||
		left.Optional != right.Optional ||
		left.Callability != right.Callability ||
		!typeDescriptorEqual(left.TypeDescriptor, right.TypeDescriptor) {
		return false
	}
	if left.Declaration == nil || right.Declaration == nil {
		return left.Declaration == right.Declaration
	}
	return *left.Declaration == *right.Declaration
}

func argumentMappingEqual(left, right ArgumentMapping) bool {
	return left.ArgumentIndex == right.ArgumentIndex &&
		left.Status == right.Status &&
		left.Unresolved == right.Unresolved &&
		parameterFactEqual(left.Parameter, right.Parameter)
}

func runtimeValueDomainEqual(left, right *RuntimeValueDomain) bool {
	if left == nil || right == nil {
		return left == right
	}
	return *left == *right
}

func entityFactEqual(left, right EntityFact) bool {
	return left.Location == right.Location &&
		left.Symbol == right.Symbol &&
		left.SymbolUnresolved == right.SymbolUnresolved &&
		left.Callability == right.Callability &&
		left.ReferenceSpace == right.ReferenceSpace &&
		left.RuntimeIdentity == right.RuntimeIdentity &&
		runtimeValueDomainEqual(left.RuntimeValueDomain, right.RuntimeValueDomain) &&
		typeDescriptorEqual(left.TypeDescriptor, right.TypeDescriptor) &&
		resolvedCallEqual(left.ResolvedCall, right.ResolvedCall)
}

func entityFactsEqual(left, right []EntityFact) bool {
	if len(left) != len(right) {
		return false
	}
	if len(left) == 0 || &left[0] == &right[0] {
		return true
	}
	return slices.EqualFunc(left, right, entityFactEqual)
}

func sourceCallEqual(left, right SourceCall) bool {
	return left.Location == right.Location &&
		left.Callee == right.Callee &&
		left.Target == right.Target &&
		locationsEqual(left.Arguments, right.Arguments)
}

func sourceBindingEqual(left, right SourceBinding) bool {
	return left.Array == right.Array &&
		locationsEqual(left.Names, right.Names) &&
		sourceCallEqual(left.Initializer, right.Initializer)
}

func sourceFunctionEqual(left, right SourceFunction) bool {
	return left.Name == right.Name &&
		left.Body == right.Body &&
		left.Exported == right.Exported &&
		left.Async == right.Async &&
		left.Arrow == right.Arrow &&
		locationsEqual(left.Parameters, right.Parameters)
}

func asyncFunctionFactEqual(left, right AsyncFunctionFact) bool {
	return left.Expression == right.Expression &&
		left.Symbol == right.Symbol &&
		left.Target == right.Target &&
		left.CanReturnAsync == right.CanReturnAsync &&
		locationsEqual(left.CallsAfterAwait, right.CallsAfterAwait)
}

func fileFactEqual(left, right FileFact) bool {
	return left.Path == right.Path &&
		slices.EqualFunc(left.Calls, right.Calls, sourceCallEqual) &&
		slices.EqualFunc(left.Bindings, right.Bindings, sourceBindingEqual) &&
		slices.EqualFunc(left.Functions, right.Functions, sourceFunctionEqual) &&
		slices.EqualFunc(left.AsyncFunctions, right.AsyncFunctions, asyncFunctionFactEqual)
}
