package tsgo

import (
	"sort"

	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/yumemi-thomas/solid-ts-facts/internal/typefacts"
)

// objectConstructionShape returns bounded, side-effect-free candidates derived
// from the selected declaration parameter and its compiler constraints. It
// never renders or parses type text. The candidates become proof only when the
// consumer validates the completed synthetic call with resolvedCall; unknown
// witnesses remain explicit so the consumer must enumerate or stop.
func (p *project) objectConstructionShape(value *checker.Type, evidence *semanticEvidence) *typefacts.ObjectConstructionShape {
	if value == nil || value.Flags()&openConstructionShapeFlags != 0 {
		return nil
	}
	evidence.descriptor(p.typeDescriptorFor(value))
	properties := p.checker.GetPropertiesOfType(value)
	result := &typefacts.ObjectConstructionShape{
		RequiredProperties: make([]typefacts.ObjectConstructionProperty, 0, len(properties)),
	}
	for _, property := range properties {
		if property == nil || property.Flags&ast.SymbolFlagsOptional != 0 {
			continue
		}
		propertyType := p.checker.GetTypeOfPropertyOfType(value, property.Name)
		if propertyType == nil {
			return nil
		}
		evidence.descriptor(p.typeDescriptorFor(propertyType))
		result.RequiredProperties = append(result.RequiredProperties, typefacts.ObjectConstructionProperty{
			Name:    property.Name,
			Witness: p.constructionWitness(propertyType),
		})
	}
	sort.Slice(result.RequiredProperties, func(left, right int) bool {
		return result.RequiredProperties[left].Name < result.RequiredProperties[right].Name
	})
	return result
}

const openConstructionShapeFlags = checker.TypeFlagsAny |
	checker.TypeFlagsUnknown |
	checker.TypeFlagsNever |
	checker.TypeFlagsIncludesError

func (p *project) constructionWitness(value *checker.Type) typefacts.ConstructionWitness {
	if value == nil || value.Flags()&openConstructionShapeFlags != 0 {
		return typefacts.ConstructionWitnessUnknown
	}
	if arrayShapeOfType(p.checker, value) == typefacts.ArrayShapeArray {
		for _, constituent := range value.Distributed() {
			if checker.IsTupleType(constituent) {
				target := constituent.TargetTupleType()
				if target == nil {
					return typefacts.ConstructionWitnessUnknown
				}
				for _, flags := range target.ElementFlags() {
					if flags&checker.ElementFlagsNonRequired == 0 {
						return typefacts.ConstructionWitnessUnknown
					}
				}
			}
		}
		return typefacts.ConstructionWitnessEmptyArray
	}
	if p.emptyObjectInhabits(value, make(map[*checker.Type]struct{})) {
		return typefacts.ConstructionWitnessEmptyObject
	}
	return typefacts.ConstructionWitnessUnknown
}

func (p *project) emptyObjectInhabits(value *checker.Type, seen map[*checker.Type]struct{}) bool {
	if value == nil || value.Flags()&openConstructionShapeFlags != 0 {
		return false
	}
	if _, cyclic := seen[value]; cyclic {
		return false
	}
	seen[value] = struct{}{}
	defer delete(seen, value)
	if value.Flags()&checker.TypeFlagsTypeParameter != 0 {
		return p.emptyObjectInhabits(checker.Checker_getBaseConstraintOfType(p.checker, value), seen)
	}
	if value.Flags()&checker.TypeFlagsIntersection != 0 {
		constituents := value.Types()
		if len(constituents) == 0 {
			return false
		}
		for _, constituent := range constituents {
			if !p.emptyObjectInhabits(constituent, seen) {
				return false
			}
		}
		return true
	}
	if value.Flags()&checker.TypeFlagsUnion != 0 {
		for _, constituent := range value.Types() {
			if p.emptyObjectInhabits(constituent, seen) {
				return true
			}
		}
		return false
	}
	if value.Flags()&checker.TypeFlagsObject == 0 {
		constraint := checker.Checker_getBaseConstraintOfType(p.checker, value)
		return constraint != nil && constraint != value && p.emptyObjectInhabits(constraint, seen)
	}
	constituents := value.Distributed()
	if len(constituents) == 0 {
		return false
	}
	for _, constituent := range constituents {
		if constituent == nil ||
			len(p.checker.GetSignaturesOfType(constituent, checker.SignatureKindCall)) != 0 ||
			len(p.checker.GetSignaturesOfType(constituent, checker.SignatureKindConstruct)) != 0 {
			return false
		}
		for _, property := range p.checker.GetPropertiesOfType(constituent) {
			if property != nil && property.Flags&ast.SymbolFlagsOptional == 0 {
				return false
			}
		}
	}
	return true
}
