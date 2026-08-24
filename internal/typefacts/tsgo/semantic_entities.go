package tsgo

import (
	"context"
	"path/filepath"

	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/yumemi-thomas/solid-ts-facts/internal/typefacts"
)

// SemanticEntities is the flat convenience interface used by focused
// compiler-fact callers. Contiguous file runs preserve caller order while all
// semantic work still crosses the production Semantic demand-run seam.
func (p *project) SemanticEntities(ctx context.Context, demands []typefacts.EntityDemand) ([]typefacts.EntityFact, error) {
	runs := make([]typefacts.SemanticDemandRun, 0)
	for start := 0; start < len(demands); {
		path := filepath.Clean(demands[start].Location.Path)
		end := start + 1
		for end < len(demands) && filepath.Clean(demands[end].Location.Path) == path {
			end++
		}
		runs = append(runs, typefacts.SemanticDemandRun{Path: path, Demands: demands[start:end]})
		start = end
	}
	runResults, err := p.SemanticDemandRuns(ctx, runs, typefacts.SemanticScope{})
	if err != nil {
		return nil, err
	}
	entities := make([]typefacts.EntityFact, 0, len(demands))
	for index := range runResults {
		entities = append(entities, runResults[index].Entities...)
	}
	return entities, nil
}

// jsxTagNameAt normalizes a node selected from a complete JSX name span to the
// complete compiler tag-name expression. A deliberately narrower demand, such
// as `Runtime` within `Runtime.Component`, keeps the identifier node so callers
// can ask whether the member root itself resolves.
func jsxTagNameAt(node *ast.Node, location typefacts.Location) *ast.Node {
	for current := node; current != nil; current = current.Parent {
		switch current.Kind {
		case ast.KindJsxOpeningElement, ast.KindJsxSelfClosingElement, ast.KindJsxClosingElement:
		default:
			continue
		}
		tagName := current.TagName()
		if tagName == nil ||
			location.StartByte != tagName.Pos() ||
			location.EndByte != tagName.End() {
			return node
		}
		return tagName
	}
	return node
}

// runtimeIdentitySymbol rejects aliases and declarations which the compiler
// represents semantically but which cannot denote a runtime value. That
// includes type-only import/export aliases and type-level property signatures
// such as JSX intrinsic entries.
func (p *project) runtimeIdentitySymbol(symbol *ast.Symbol) *ast.Symbol {
	if symbol.Flags&ast.SymbolFlagsAlias != 0 {
		for _, declaration := range symbol.Declarations {
			if ast.IsPartOfTypeOnlyImportOrExportDeclaration(declaration) {
				return nil
			}
		}
	}
	symbol = p.canonicalSymbol(symbol)
	if symbol == nil ||
		symbol.Flags&ast.SymbolFlagsValue == 0 ||
		symbol.ValueDeclaration == nil ||
		symbol.ValueDeclaration.Kind == ast.KindPropertySignature {
		return nil
	}
	return symbol
}

func (p *project) typeDescriptorFor(value *checker.Type) *typefacts.TypeDescriptor {
	if descriptor := p.typeDescriptors[value]; descriptor != nil {
		return descriptor
	}
	descriptor := &typefacts.TypeDescriptor{Text: p.checker.TypeToString(value)}
	if alias := value.Alias(); alias != nil && alias.Symbol() != nil {
		descriptor.AliasDeclarations = declarationsForSymbol(alias.Symbol())
		descriptor.OriginModule = declarationModule(alias.Symbol())
	}
	if p.typeDescriptors == nil {
		p.typeDescriptors = make(map[*checker.Type]*typefacts.TypeDescriptor)
	}
	p.typeDescriptors[value] = descriptor
	return descriptor
}

func callabilityOfType(typeChecker *checker.Checker, value *checker.Type) typefacts.Callability {
	if value == nil || value.Flags()&(checker.TypeFlagsAny|checker.TypeFlagsUnknown|checker.TypeFlagsNever|checker.TypeFlagsIncludesError) != 0 {
		return typefacts.CallabilityUnknown
	}
	constituents := value.Distributed()
	if len(constituents) == 0 {
		return typefacts.CallabilityUnknown
	}
	callable, untypedCallable, nonCallable := false, false, false
	for _, constituent := range constituents {
		if constituent == nil || constituent.Flags()&(checker.TypeFlagsAny|checker.TypeFlagsUnknown|checker.TypeFlagsNever|checker.TypeFlagsIncludesError) != 0 {
			return typefacts.CallabilityUnknown
		}
		switch {
		case len(typeChecker.GetSignaturesOfType(constituent, checker.SignatureKindCall)) != 0:
			callable = true
		case callableWithoutSignatures(typeChecker, constituent):
			untypedCallable = true
		default:
			nonCallable = true
		}
	}
	switch {
	case nonCallable && (callable || untypedCallable):
		return typefacts.CallabilityMixed
	case untypedCallable:
		// At a union this rung promises only that every constituent answered
		// callable in some sense, per constituent -- not that the union's
		// call as written compiles, and not that no constituent's signature
		// is readable. Both directions occur: Function | (() => void) still
		// carries one readable, arity-enforced call signature (tsc flags a
		// wrong argument count with TS2554), while Function | Merged -- two
		// constituents each individually untyped-callable, such as a
		// `declare class C {}` merged with `interface C extends Function`
		// -- has tsc refuse the call outright (TS2349), because the
		// untyped-call rule that grants each constituent its own fallback
		// signature explicitly excludes unions. Either way the answer stays
		// conservative: a consumer reading it as "callable, signature
		// unread" only under-checks what it could have proven and does not
		// itself claim the call as written type-checks. Before this rung
		// existed both shapes answered nonCallable or mixed.
		return typefacts.CallabilityUntypedCallable
	case callable:
		return typefacts.CallabilityCallable
	default:
		return typefacts.CallabilityNonCallable
	}
}

// callableWithoutSignatures reports whether the compiler permits calling a
// constituent that exposes no call *or* construct signature — the
// signature-less Function-supertype family. It is the compiler's own TS 1.0
// §4.12 rule, asked with the same apparent type its call-resolution sites use,
// so the fact answers what `constituent()` would actually do rather than
// re-deriving assignability here.
//
// It is deliberately the untyped-call rule and not checker.isFunctionObjectType,
// the `typeof x === "function"` narrowing predicate. The two agree on
// `Function`, `CallableFunction`, `NewableFunction` and any alias or interface
// reaching them, and both reject `object`, `{}`, `Record<string, unknown>` and
// an interface that merely declares `bind`. They diverge on an intersection
// such as `Function & { tag: 1 }`: isFunctionObjectType's `bind`-member
// quick-out reads resolveStructuredTypeMembers().members, which the compiler
// leaves empty for every intersection by construction, so that predicate
// answers false there while the call is in fact permitted. Only callability
// asks this question of the fact vocabulary, so it asks the rule that decides
// the call.
//
// The caller must already have excluded any/unknown/never/error constituents;
// this only adds the signature-less case to what GetSignaturesOfType saw. The
// same exclusion is repeated against the *apparent* type as a defensive guard
// against the rule's own text, which has a disjunct admitting any type
// parameter whose constraint is `any`. Measured, that disjunct never actually
// fires here: `function f<T extends any>(x: T) { x(); }` reduces T's apparent
// type to `unknown`, not `any`, and the compiler itself refuses the call
// (TS2349, "Type 'unknown' has no call signatures"). So the constituent falls
// through to `nonCallable` regardless, which agrees with tsc -- there is no
// false negative here to close. The guard stays in for the disjunct as
// written, not for an observed gap.
func callableWithoutSignatures(typeChecker *checker.Checker, value *checker.Type) bool {
	if len(typeChecker.GetSignaturesOfType(value, checker.SignatureKindConstruct)) != 0 {
		return false
	}
	apparent := checker.Checker_getReducedApparentType(typeChecker, value)
	if apparent == nil ||
		apparent.Flags()&(checker.TypeFlagsAny|checker.TypeFlagsUnknown|checker.TypeFlagsNever|checker.TypeFlagsIncludesError) != 0 {
		return false
	}
	return checker.Checker_isUntypedFunctionCall(typeChecker, value, apparent, 0, 0)
}

// constructabilityOfType is callabilityOfType asked of construct signatures.
// The two walk the same constituents of the same type and fail closed on the
// same type flags, so a caller demanding both gets two answers partitioned
// alike.
//
// Abstract construct signatures are deliberately not filtered out: the
// question is whether the type has a construct signature, and an abstract
// class is still a function object at runtime. A caller that needs
// instantiability must ask something else.
//
// It has no counterpart to callability's untypedCallable, and that asymmetry is
// the compiler's: `new x()` on a signature-less Function-supertype value is a
// compile error, because resolveNewExpression carries no untyped fallback the
// way call, tagged-template and decorator resolution do. nonConstructable there
// is what the type system itself says, so answering anything else would invent
// a claim tsc contradicts.
func constructabilityOfType(typeChecker *checker.Checker, value *checker.Type) typefacts.Constructability {
	if value == nil || value.Flags()&(checker.TypeFlagsAny|checker.TypeFlagsUnknown|checker.TypeFlagsNever|checker.TypeFlagsIncludesError) != 0 {
		return typefacts.ConstructabilityUnknown
	}
	constituents := value.Distributed()
	if len(constituents) == 0 {
		return typefacts.ConstructabilityUnknown
	}
	constructable, nonConstructable := false, false
	for _, constituent := range constituents {
		if constituent == nil || constituent.Flags()&(checker.TypeFlagsAny|checker.TypeFlagsUnknown|checker.TypeFlagsNever|checker.TypeFlagsIncludesError) != 0 {
			return typefacts.ConstructabilityUnknown
		}
		if len(typeChecker.GetSignaturesOfType(constituent, checker.SignatureKindConstruct)) != 0 {
			constructable = true
		} else {
			nonConstructable = true
		}
	}
	switch {
	case constructable && nonConstructable:
		return typefacts.ConstructabilityMixed
	case constructable:
		return typefacts.ConstructabilityConstructable
	default:
		return typefacts.ConstructabilityNonConstructable
	}
}

func unknownRuntimeValueDomain() typefacts.RuntimeValueDomain {
	return typefacts.RuntimeValueDomain{
		MayBeCallable: true, MayBeUndefined: true, MayBeOther: true, Unknown: true,
	}
}

func unknownPrimitiveValueDomain() typefacts.PrimitiveValueDomain {
	return typefacts.UnknownPrimitiveValueDomain()
}

// primitiveValueDomainOfType classifies the same checker type as
// runtimeValueDomainOfType, but preserves JavaScript's primitive categories.
// Constraints and unions are closed recursively; all remaining inhabited,
// compiler-known types are objects/functions.
func primitiveValueDomainOfType(typeChecker *checker.Checker, value *checker.Type) typefacts.PrimitiveValueDomain {
	return primitiveValueDomainOfTypeSeen(typeChecker, value, make(map[*checker.Type]struct{}))
}

func primitiveValueDomainOfTypeSeen(
	typeChecker *checker.Checker,
	value *checker.Type,
	seen map[*checker.Type]struct{},
) typefacts.PrimitiveValueDomain {
	if value == nil {
		return unknownPrimitiveValueDomain()
	}
	flags := value.Flags()
	if flags&(checker.TypeFlagsAny|checker.TypeFlagsUnknown) != 0 {
		return unknownPrimitiveValueDomain()
	}
	if flags&checker.TypeFlagsNever != 0 {
		return typefacts.NewPrimitiveValueDomain(false, false, false, false, false, false, false, false, false)
	}
	if _, cycling := seen[value]; cycling {
		return unknownPrimitiveValueDomain()
	}
	seen[value] = struct{}{}
	defer delete(seen, value)

	if flags&checker.TypeFlagsInstantiable != 0 {
		var constraint *checker.Type
		if flags&checker.TypeFlagsTypeParameter != 0 {
			constraint = typeChecker.GetConstraintOfTypeParameter(value)
		} else {
			constraint = checker.Checker_getBaseConstraintOfType(typeChecker, value)
		}
		if constraint == nil {
			return unknownPrimitiveValueDomain()
		}
		if constraint != value {
			return primitiveValueDomainOfTypeSeen(typeChecker, constraint, seen)
		}
	}

	if flags&checker.TypeFlagsUnion != 0 {
		var domain typefacts.PrimitiveValueDomain
		for _, constituent := range value.Types() {
			part := primitiveValueDomainOfTypeSeen(typeChecker, constituent, seen)
			domain = domain.Union(part)
		}
		return domain
	}
	switch {
	case flags&checker.TypeFlagsStringLike != 0:
		return typefacts.NewPrimitiveValueDomain(true, false, false, false, false, false, false, false, false)
	case flags&checker.TypeFlagsNumberLike != 0:
		return typefacts.NewPrimitiveValueDomain(false, true, false, false, false, false, false, false, checker.NumberLiteralIsFinite(value))
	case flags&checker.TypeFlagsBooleanLike != 0:
		return typefacts.NewPrimitiveValueDomain(false, false, true, false, false, false, false, false, false)
	case flags&checker.TypeFlagsBigIntLike != 0:
		return typefacts.NewPrimitiveValueDomain(false, false, false, true, false, false, false, false, false)
	case flags&checker.TypeFlagsESSymbolLike != 0:
		return typefacts.NewPrimitiveValueDomain(false, false, false, false, true, false, false, false, false)
	case flags&checker.TypeFlagsNull != 0:
		return typefacts.NewPrimitiveValueDomain(false, false, false, false, false, true, false, false, false)
	case flags&(checker.TypeFlagsUndefined|checker.TypeFlagsVoid) != 0:
		return typefacts.NewPrimitiveValueDomain(false, false, false, false, false, false, true, false, false)
	}
	if flags&checker.TypeFlagsIncludesError != 0 {
		return unknownPrimitiveValueDomain()
	}
	switch {
	case typeChecker.IsTypeAssignableTo(value, typeChecker.GetStringType()):
		return typefacts.NewPrimitiveValueDomain(true, false, false, false, false, false, false, false, false)
	case typeChecker.IsTypeAssignableTo(value, typeChecker.GetNumberType()):
		return typefacts.NewPrimitiveValueDomain(false, true, false, false, false, false, false, false, false)
	case typeChecker.IsTypeAssignableTo(value, typeChecker.GetBooleanType()):
		return typefacts.NewPrimitiveValueDomain(false, false, true, false, false, false, false, false, false)
	case typeChecker.IsTypeAssignableTo(value, typeChecker.GetBigIntType()):
		return typefacts.NewPrimitiveValueDomain(false, false, false, true, false, false, false, false, false)
	case typeChecker.IsTypeAssignableTo(value, typeChecker.GetESSymbolType()):
		return typefacts.NewPrimitiveValueDomain(false, false, false, false, true, false, false, false, false)
	case typeChecker.IsTypeAssignableTo(value, typeChecker.GetNullType()):
		return typefacts.NewPrimitiveValueDomain(false, false, false, false, false, true, false, false, false)
	case typeChecker.IsTypeAssignableTo(value, typeChecker.GetUndefinedType()):
		return typefacts.NewPrimitiveValueDomain(false, false, false, false, false, false, true, false, false)
	default:
		return typefacts.NewPrimitiveValueDomain(false, false, false, false, false, false, false, true, false)
	}
}

// runtimeValueDomainOfType classifies checker types by runtime value kind.
// Union members are combined, constrained generics are classified through the
// checker's resolved base constraint, callable structured types are detected
// from real call signatures, and undefined is recognized from checker flags or
// assignability. Rendered type text never participates.
func runtimeValueDomainOfType(typeChecker *checker.Checker, value *checker.Type) typefacts.RuntimeValueDomain {
	return runtimeValueDomainOfTypeSeen(typeChecker, value, make(map[*checker.Type]struct{}))
}

func runtimeValueDomainOfTypeSeen(
	typeChecker *checker.Checker,
	value *checker.Type,
	seen map[*checker.Type]struct{},
) typefacts.RuntimeValueDomain {
	if value == nil {
		return unknownRuntimeValueDomain()
	}
	flags := value.Flags()
	if flags&(checker.TypeFlagsAny|checker.TypeFlagsUnknown|checker.TypeFlagsIncludesError) != 0 {
		return unknownRuntimeValueDomain()
	}
	if flags&checker.TypeFlagsNever != 0 {
		return typefacts.RuntimeValueDomain{}
	}
	if _, cycling := seen[value]; cycling {
		return unknownRuntimeValueDomain()
	}
	seen[value] = struct{}{}
	defer delete(seen, value)

	// TypeScript-Go resolves nested type-parameter constraints and the base
	// constraints of other instantiable forms. A nil constraint is the
	// conservative signal for an unconstrained or circular generic.
	if flags&checker.TypeFlagsInstantiable != 0 {
		var constraint *checker.Type
		if flags&checker.TypeFlagsTypeParameter != 0 {
			constraint = typeChecker.GetConstraintOfTypeParameter(value)
		} else {
			constraint = checker.Checker_getBaseConstraintOfType(typeChecker, value)
		}
		if constraint == nil {
			return unknownRuntimeValueDomain()
		}
		if constraint != value {
			return runtimeValueDomainOfTypeSeen(typeChecker, constraint, seen)
		}
	}

	if flags&checker.TypeFlagsUnion != 0 {
		var domain typefacts.RuntimeValueDomain
		for _, constituent := range value.Types() {
			part := runtimeValueDomainOfTypeSeen(typeChecker, constituent, seen)
			domain.MayBeCallable = domain.MayBeCallable || part.MayBeCallable
			domain.MayBeUndefined = domain.MayBeUndefined || part.MayBeUndefined
			domain.MayBeOther = domain.MayBeOther || part.MayBeOther
			domain.Unknown = domain.Unknown || part.Unknown
		}
		return domain
	}
	if flags&checker.TypeFlagsUndefined != 0 {
		return typefacts.RuntimeValueDomain{MayBeUndefined: true}
	}
	// A `void` result is undefined at runtime, and is trusted here exactly as
	// every other declared return type is. The known hole is return-type
	// bivariance: `const f: () => void = () => 42` is legal, so calling through
	// a bivariantly assigned function value can yield a real value where the
	// type says void. That is a property of the assignment, invisible at the
	// call, and treating every void call as open instead would refuse
	// `console.log(value)` in an effect's apply -- idiomatic, correct code.
	if flags&checker.TypeFlagsVoid != 0 {
		return typefacts.RuntimeValueDomain{MayBeUndefined: true}
	}
	if len(typeChecker.GetSignaturesOfType(value, checker.SignatureKindCall)) != 0 {
		return typefacts.RuntimeValueDomain{MayBeCallable: true}
	}
	// This matters for intersections such as T & undefined. The checker has
	// already reduced impossible intersections to never where it can; when the
	// remaining source type is assignable to undefined, every inhabitant is an
	// undefined value.
	if typeChecker.IsTypeAssignableTo(value, typeChecker.GetUndefinedType()) {
		return typefacts.RuntimeValueDomain{MayBeUndefined: true}
	}
	return typefacts.RuntimeValueDomain{MayBeOther: true}
}
