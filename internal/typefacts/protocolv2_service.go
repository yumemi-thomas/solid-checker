package typefacts

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"path/filepath"
	"slices"
	"strings"
	"unicode/utf8"
)

// ClosureResponseFor materializes a frozen-v2 response from the retained
// TS-Go project. The request generation must match the live closure
// generation; v2 deliberately has no update operation.
func (p *ClosureProject) ClosureResponseFor(ctx context.Context, request ClosureRequest) (ClosureResponse, error) {
	if err := ValidateClosureRequest(request); err != nil {
		return ClosureResponse{}, err
	}
	p.mu.Lock()
	if request.Generation != p.generation {
		p.mu.Unlock()
		return ClosureResponse{}, ErrGenerationMismatch
	}
	seeds, err := locationsFromV2(request.CompilerSpans)
	if err != nil {
		p.mu.Unlock()
		return ClosureResponse{}, err
	}
	if !slices.Equal(p.compilerSeeds, seeds) {
		p.compilerSeeds = seeds
		p.table = nil
		p.closedSyms = nil
		p.fullTier = nil
	}
	p.mu.Unlock()

	table, err := p.Table(ctx)
	if err != nil {
		return ClosureResponse{}, err
	}
	response := ClosureResponse{
		Schema:     TypeFactsSchemaVersionV2,
		ProjectID:  request.ProjectID,
		Generation: request.Generation,
		Table:      tableV2(*table, request.ProjectID, request.Generation),
	}
	if err := ValidateClosureResponse(request, response); err != nil {
		return ClosureResponse{}, err
	}
	return response, nil
}

func locationsFromV2(values []LocationV2) ([]Location, error) {
	result := make([]Location, 0, len(values))
	for _, value := range values {
		start, end := int(value.StartByte), int(value.EndByte)
		if start < 0 || end < start || uint64(start) != value.StartByte || uint64(end) != value.EndByte {
			return nil, fmt.Errorf("invalid compiler span %s:%d..%d", value.Path, value.StartByte, value.EndByte)
		}
		result = append(result, Location{Path: filepath.Clean(value.Path), StartByte: start, EndByte: end})
	}
	return result, nil
}

func locationV2(value Location) LocationV2 {
	return LocationV2{Path: value.Path, StartByte: uint64(value.StartByte), EndByte: uint64(value.EndByte)}
}

func declarationV2(value Declaration) DeclarationV2 {
	return DeclarationV2{Name: wireSymbolName(value.Name), Kind: value.Kind, Location: locationV2(value.Location)}
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
	converted := EntityFactV2{Location: locationV2(entity.Location), Symbol: string(entity.Symbol)}
	if entity.TypeDescriptor != nil {
		descriptor := TypeDescriptorV2{Text: entity.TypeDescriptor.Text, OriginModule: entity.TypeDescriptor.OriginModule}
		for _, declaration := range entity.TypeDescriptor.AliasDeclarations {
			descriptor.AliasDeclarations = append(descriptor.AliasDeclarations, declarationV2(declaration))
		}
		converted.TypeDescriptor = &descriptor
	}
	if entity.ResolvedCall != nil {
		converted.ResolvedCall = &CallV2{Target: string(entity.ResolvedCall.Target), ReturnTypeText: entity.ResolvedCall.ReturnTypeText}
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
		Schema:     TypeFactsSchemaVersionV2,
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

func tableV2(table FactTable, projectID string, generation uint64) FactTableV2 {
	return FactTableV2From(table, projectID, generation)
}
