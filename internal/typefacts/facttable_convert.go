package typefacts

import (
	"crypto/sha256"
	"encoding/hex"
	"strings"
	"unicode/utf8"
)

func locationV2(value Location) LocationV2 {
	return LocationV2{Path: value.Path, StartByte: uint64(value.StartByte), EndByte: uint64(value.EndByte)}
}

func declarationV2(value Declaration) DeclarationV2 {
	return DeclarationV2{Name: wireSymbolName(value.Name), Kind: value.Kind, Location: locationV2(value.Location)}
}

func typeDescriptorV2(value *TypeDescriptor) *TypeDescriptorV2 {
	if value == nil {
		return nil
	}
	descriptor := &TypeDescriptorV2{Text: value.Text, OriginModule: value.OriginModule}
	for _, declaration := range value.AliasDeclarations {
		descriptor.AliasDeclarations = append(descriptor.AliasDeclarations, declarationV2(declaration))
	}
	return descriptor
}

func resolvedDeclarationV2(value *ResolvedDeclaration) *ResolvedDeclarationV2 {
	if value == nil {
		return nil
	}
	result := &ResolvedDeclarationV2{
		Symbol: string(value.Symbol), Name: wireSymbolName(value.Name), Kind: value.Kind,
		Location: locationV2(value.Location), QualifiedName: value.QualifiedName,
		OriginModule: value.OriginModule, SourceFile: value.SourceFile,
		StandardLibrary: value.StandardLibrary,
	}
	for _, owner := range value.Owners {
		result.Owners = append(result.Owners, DeclarationOwnerV2{
			Symbol: string(owner.Symbol), Name: wireSymbolName(owner.Name), Kind: owner.Kind,
			Location: locationV2(owner.Location),
		})
	}
	return result
}

func argumentMappingV2(value ArgumentMapping) ArgumentMappingV2 {
	result := ArgumentMappingV2{
		ArgumentIndex: uint64(value.ArgumentIndex),
		Status:        value.Status,
		Unresolved:    value.Unresolved,
	}
	if value.Parameter != nil {
		parameter := value.Parameter
		result.Parameter = &ParameterFactV2{
			Index: uint64(parameter.Index), Symbol: string(parameter.Symbol),
			Rest: parameter.Rest, Optional: parameter.Optional,
			Callability: parameter.Callability, TypeDescriptor: typeDescriptorV2(parameter.TypeDescriptor),
		}
		if parameter.Declaration != nil {
			declaration := declarationV2(*parameter.Declaration)
			result.Parameter.Declaration = &declaration
		}
	}
	return result
}

// TypeScript uses the invalid UTF-8 byte 0xfe as an unambiguous prefix for
// synthetic symbol names. Deterministic CBOR text must be valid UTF-8, so use
// TypeScript's public escaped-name spelling at the protocol boundary.
func wireSymbolName(name string) string {
	const internalSymbolNamePrefix = "\xfe"
	if strings.HasPrefix(name, internalSymbolNamePrefix) {
		name = "__" + strings.TrimPrefix(name, internalSymbolNamePrefix)
	}
	if !utf8.ValidString(name) {
		return strings.ToValidUTF8(name, "\uFFFD")
	}
	return name
}

func callV2(value SourceCall) SourceCallV2 {
	result := SourceCallV2{Location: locationV2(value.Location), Callee: locationV2(value.Callee), Target: string(value.Target)}
	for _, argument := range value.Arguments {
		result.Arguments = append(result.Arguments, locationV2(argument))
	}
	return result
}

func sourceDigestV2(source SourceFile) SourceDigestV2 {
	sum := sha256.Sum256(source.Source)
	return SourceDigestV2{Path: source.Path, SHA256: "sha256:" + hex.EncodeToString(sum[:])}
}

func entityFactV2(entity EntityFact) EntityFactV2 {
	converted := EntityFactV2{
		Location: locationV2(entity.Location), Symbol: string(entity.Symbol),
		Callability: entity.Callability, ReferenceSpace: entity.ReferenceSpace,
		RuntimeIdentity: string(entity.RuntimeIdentity),
	}
	if entity.TypeDescriptor != nil {
		converted.TypeDescriptor = typeDescriptorV2(entity.TypeDescriptor)
	}
	if entity.ResolvedCall != nil {
		call := entity.ResolvedCall
		converted.ResolvedCall = &CallV2{
			Target: string(call.Target), ReturnTypeText: call.ReturnTypeText, Validity: call.Validity,
			Kind: call.Kind, Declaration: resolvedDeclarationV2(call.Declaration),
		}
		for _, mapping := range call.Arguments {
			converted.ResolvedCall.Arguments = append(converted.ResolvedCall.Arguments, argumentMappingV2(mapping))
		}
	}
	return converted
}

func symbolFactV2(symbol SymbolFact) SymbolFactV2 {
	converted := SymbolFactV2{ID: string(symbol.ID), AliasTarget: string(symbol.AliasTarget)}
	for _, declaration := range symbol.Declarations {
		converted.Declarations = append(converted.Declarations, declarationV2(declaration))
	}
	if symbol.AliasTarget == "" {
		for _, reference := range symbol.References {
			converted.References = append(converted.References, locationV2(reference))
		}
	}
	return converted
}

func fileFactV2(file FileFact) FileFactV2 {
	converted := FileFactV2{Path: file.Path}
	for _, call := range file.Calls {
		converted.Calls = append(converted.Calls, callV2(call))
	}
	for _, binding := range file.Bindings {
		item := SourceBindingV2{Array: binding.Array, Names: []LocationV2{}, Initializer: callV2(binding.Initializer)}
		for _, name := range binding.Names {
			item.Names = append(item.Names, locationV2(name))
		}
		converted.Bindings = append(converted.Bindings, item)
	}
	for _, function := range file.Functions {
		item := SourceFunctionV2{Name: locationV2(function.Name), Body: locationV2(function.Body), Exported: function.Exported, Async: function.Async, Arrow: function.Arrow}
		for _, parameter := range function.Parameters {
			item.Parameters = append(item.Parameters, locationV2(parameter))
		}
		converted.Functions = append(converted.Functions, item)
	}
	for _, function := range file.AsyncFunctions {
		item := AsyncFunctionFactV2{Expression: locationV2(function.Expression), Symbol: string(function.Symbol), Target: string(function.Target), CanReturnAsync: function.CanReturnAsync}
		for _, call := range function.CallsAfterAwait {
			item.CallsAfterAwait = append(item.CallsAfterAwait, locationV2(call))
		}
		converted.AsyncFunctions = append(converted.AsyncFunctions, item)
	}
	return converted
}

// FactTableV2From converts a canonical internal table into the complete v2
// wire representation. Stateful v3 analysis should prefer the direct delta
// converter so unchanged rows are not allocated again.
func FactTableV2From(table FactTable, projectID string, generation uint64) FactTableV2 {
	result := FactTableV2{
		Schema:     TypeFactsTableSchemaVersion,
		Generation: generation,
		ProjectID:  projectID,
		Sources:    []SourceDigestV2{},
		Entities:   []EntityFactV2{},
		Symbols:    []SymbolFactV2{},
		Files:      []FileFactV2{},
	}
	for _, source := range table.Sources {
		result.Sources = append(result.Sources, sourceDigestV2(source))
	}
	for _, entity := range table.Entities {
		result.Entities = append(result.Entities, entityFactV2(entity))
	}
	table.rangeSymbolFacts(func(symbol SymbolFact) {
		result.Symbols = append(result.Symbols, symbolFactV2(symbol))
	})
	for _, file := range table.Files {
		result.Files = append(result.Files, fileFactV2(file))
	}
	return result
}
