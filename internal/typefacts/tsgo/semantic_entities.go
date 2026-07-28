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

// jsxTagNameAt normalizes a node selected from a JSX name span to the complete
// compiler tag-name expression. In particular, a position at the start of
// `Runtime.Component` selects the `Runtime` identifier; asking the checker
// about that child resolves the namespace object, while asking about the
// enclosing property-access tag resolves `Component`.
func jsxTagNameAt(node *ast.Node, location typefacts.Location) *ast.Node {
	for current := node; current != nil; current = current.Parent {
		switch current.Kind {
		case ast.KindJsxOpeningElement, ast.KindJsxSelfClosingElement, ast.KindJsxClosingElement:
		default:
			continue
		}
		tagName := current.TagName()
		if tagName == nil ||
			location.StartByte < tagName.Pos() ||
			location.StartByte >= tagName.End() ||
			location.EndByte > tagName.End() {
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
	callable, nonCallable := false, false
	for _, constituent := range constituents {
		if constituent == nil || constituent.Flags()&(checker.TypeFlagsAny|checker.TypeFlagsUnknown|checker.TypeFlagsNever|checker.TypeFlagsIncludesError) != 0 {
			return typefacts.CallabilityUnknown
		}
		if len(typeChecker.GetSignaturesOfType(constituent, checker.SignatureKindCall)) != 0 {
			callable = true
		} else {
			nonCallable = true
		}
	}
	switch {
	case callable && nonCallable:
		return typefacts.CallabilityMixed
	case callable:
		return typefacts.CallabilityCallable
	default:
		return typefacts.CallabilityNonCallable
	}
}
