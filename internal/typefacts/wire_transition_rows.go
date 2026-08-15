package typefacts

import (
	"fmt"
)

const (
	wireTransitionSourceOpShift = 0
	wireTransitionEntityOpShift = 2
	wireTransitionFileOpShift   = 4
)

const (
	wireTransitionEntityHasTypeDescriptor = 1 << iota
	wireTransitionEntityHasResolvedCall
	wireTransitionEntityHasCallability
	wireTransitionEntityHasReferenceSpace
	wireTransitionEntityHasRuntimeIdentity
	wireTransitionEntityHasRuntimeValueDomain
	wireTransitionEntitySymbolUnresolved
)

const (
	wireTransitionParameterHasDeclaration = 1 << iota
	wireTransitionParameterIsRest
	wireTransitionParameterIsOptional
	wireTransitionParameterHasTypeDescriptor
)

func writeWireTransitionPathOp(w *packedWriter, operation *wireTransitionPathOp, tableSchema uint64) error {
	if operation.path == "" {
		return fmt.Errorf("path operation has an empty path")
	}
	flags, err := wireTransitionPathFlags(operation)
	if err != nil {
		return err
	}
	w.text(operation.path)
	w.u64(flags)

	if operation.sourceOp == wireTransitionReplace {
		if operation.source.Path != operation.path {
			return fmt.Errorf(
				"source replacement path = %q, operation path = %q",
				operation.source.Path,
				operation.path,
			)
		}
		w.raw(operation.source.SHA256[:])
	}
	if operation.entityOp == wireTransitionReplace {
		if err := writeWireTransitionEntityRun(w, operation.path, operation.entities, tableSchema); err != nil {
			return fmt.Errorf("entity replacement for %q: %w", operation.path, err)
		}
	}
	if operation.fileOp == wireTransitionReplace {
		if operation.file.Path != operation.path {
			return fmt.Errorf(
				"file replacement path = %q, operation path = %q",
				operation.file.Path,
				operation.path,
			)
		}
		writeWireTransitionFileBody(w, operation.file)
	}
	return nil
}

func wireTransitionPathFlags(operation *wireTransitionPathOp) (uint64, error) {
	if err := validateWireTransitionCollectionOp("source", operation.sourceOp); err != nil {
		return 0, err
	}
	if err := validateWireTransitionCollectionOp("entity", operation.entityOp); err != nil {
		return 0, err
	}
	if err := validateWireTransitionCollectionOp("file", operation.fileOp); err != nil {
		return 0, err
	}
	return uint64(operation.sourceOp)<<wireTransitionSourceOpShift |
		uint64(operation.entityOp)<<wireTransitionEntityOpShift |
		uint64(operation.fileOp)<<wireTransitionFileOpShift, nil
}

func validateWireTransitionCollectionOp(name string, operation wireTransitionCollectionOp) error {
	switch operation {
	case wireTransitionUnchanged, wireTransitionReplace, wireTransitionRemove:
		return nil
	default:
		return fmt.Errorf("%s collection operation = %d", name, operation)
	}
}

func writeWireTransitionEntityRun(
	w *packedWriter,
	path string,
	entities []EntityFact,
	tableSchema uint64,
) error {
	w.u64(uint64(len(entities)))
	var previousStart int64
	for index := range entities {
		entity := &entities[index]
		if entity.Location.Path != path {
			return fmt.Errorf(
				"entity %d path = %q, run path = %q",
				index,
				entity.Location.Path,
				path,
			)
		}
		if err := validateWireTransitionLocation(entity.Location); err != nil {
			return fmt.Errorf("entity %d location: %w", index, err)
		}
		if entity.Symbol != "" && entity.SymbolUnresolved {
			return fmt.Errorf("entity %d cannot be both resolved and unresolved", index)
		}

		var callability uint64
		var err error
		if entity.Callability != "" {
			callability, err = wireTransitionCallabilityCode(entity.Callability)
			if err != nil {
				return fmt.Errorf("entity %d: %w", index, err)
			}
		}
		var referenceSpace uint64
		if entity.ReferenceSpace != "" {
			referenceSpace, err = wireTransitionReferenceSpaceCode(entity.ReferenceSpace)
			if err != nil {
				return fmt.Errorf("entity %d: %w", index, err)
			}
		}

		start := int64(entity.Location.StartByte)
		w.signed(start - previousStart)
		w.u64(uint64(entity.Location.EndByte - entity.Location.StartByte))
		w.text(string(entity.Symbol))
		flags := uint64(0)
		if entity.TypeDescriptor != nil {
			flags |= wireTransitionEntityHasTypeDescriptor
		}
		if entity.ResolvedCall != nil {
			flags |= wireTransitionEntityHasResolvedCall
		}
		if entity.Callability != "" {
			flags |= wireTransitionEntityHasCallability
		}
		if entity.ReferenceSpace != "" {
			flags |= wireTransitionEntityHasReferenceSpace
		}
		if entity.RuntimeIdentity != "" {
			flags |= wireTransitionEntityHasRuntimeIdentity
		}
		if entity.RuntimeValueDomain != nil {
			flags |= wireTransitionEntityHasRuntimeValueDomain
		}
		if tableSchema >= TypeFactsTableSchemaVersionV5 && entity.SymbolUnresolved {
			flags |= wireTransitionEntitySymbolUnresolved
		}
		w.u64(flags)
		if entity.TypeDescriptor != nil {
			w.internalTypeDescriptor(entity.TypeDescriptor)
		}
		if entity.ResolvedCall != nil {
			if err := writeWireTransitionResolvedCall(w, entity.ResolvedCall, tableSchema); err != nil {
				return fmt.Errorf("entity %d resolved call: %w", index, err)
			}
		}
		if entity.Callability != "" {
			w.u64(callability)
		}
		if entity.ReferenceSpace != "" {
			w.u64(referenceSpace)
		}
		if entity.RuntimeIdentity != "" {
			w.text(string(entity.RuntimeIdentity))
		}
		if entity.RuntimeValueDomain != nil {
			w.u64(wireTransitionRuntimeValueDomainBits(*entity.RuntimeValueDomain))
		}
		previousStart = start
	}
	return nil
}

func writeWireTransitionResolvedCall(w *packedWriter, call *Call, tableSchema uint64) error {
	validity, err := wireTransitionValidityCode(call.Validity)
	if err != nil {
		return err
	}
	kind, err := wireTransitionCallKindCode(call.Kind)
	if err != nil {
		return err
	}

	w.text(string(call.Target))
	w.text(call.ReturnTypeText)
	w.u64(validity)
	w.u64(kind)
	w.u64(boolBit(call.Declaration != nil))
	if call.Declaration != nil {
		w.internalResolvedDeclaration(call.Declaration)
	}
	if tableSchema >= TypeFactsTableSchemaVersionV6 {
		w.u64(boolBit(call.Targets != nil))
		if call.Targets != nil {
			if len(call.Targets.Candidates) == 0 {
				return fmt.Errorf("resolved-call target set has no candidates")
			}
			w.u64(boolBit(call.Targets.Exhaustive))
			w.u64(uint64(len(call.Targets.Candidates)))
			for index := range call.Targets.Candidates {
				candidate := &call.Targets.Candidates[index]
				if candidate.Symbol == "" {
					return fmt.Errorf("target candidate %d has no symbol", index)
				}
				w.internalResolvedDeclaration(candidate)
			}
		}
	}
	w.u64(uint64(len(call.Arguments)))
	for index := range call.Arguments {
		mapping := &call.Arguments[index]
		if mapping.ArgumentIndex < 0 {
			return fmt.Errorf(
				"argument mapping %d has negative argument index %d",
				index,
				mapping.ArgumentIndex,
			)
		}
		status, err := wireTransitionMappingStatusCode(mapping.Status)
		if err != nil {
			return fmt.Errorf("argument mapping %d: %w", index, err)
		}
		reason, err := wireTransitionMappingReasonCode(mapping.Unresolved)
		if err != nil {
			return fmt.Errorf("argument mapping %d: %w", index, err)
		}
		if (status == 0) != (reason == 0) {
			return fmt.Errorf(
				"argument mapping %d status %q and reason %q disagree",
				index,
				mapping.Status,
				mapping.Unresolved,
			)
		}

		w.u64(uint64(mapping.ArgumentIndex))
		w.u64(status)
		w.u64(reason)
		w.u64(boolBit(mapping.Parameter != nil))
		if mapping.Parameter == nil {
			continue
		}
		if err := writeWireTransitionParameter(w, index, mapping.Parameter); err != nil {
			return err
		}
	}
	return nil
}

func writeWireTransitionParameter(
	w *packedWriter,
	mappingIndex int,
	parameter *ParameterFact,
) error {
	if parameter.Index < 0 {
		return fmt.Errorf(
			"argument mapping %d has negative parameter index %d",
			mappingIndex,
			parameter.Index,
		)
	}
	callability, err := wireTransitionCallabilityCode(parameter.Callability)
	if err != nil {
		return fmt.Errorf("argument mapping %d parameter: %w", mappingIndex, err)
	}

	w.u64(uint64(parameter.Index))
	w.text(string(parameter.Symbol))
	flags := uint64(0)
	if parameter.Declaration != nil {
		flags |= wireTransitionParameterHasDeclaration
	}
	if parameter.Rest {
		flags |= wireTransitionParameterIsRest
	}
	if parameter.Optional {
		flags |= wireTransitionParameterIsOptional
	}
	if parameter.TypeDescriptor != nil {
		flags |= wireTransitionParameterHasTypeDescriptor
	}
	w.u64(flags)
	if parameter.Declaration != nil {
		w.text(wireSymbolName(parameter.Declaration.Name))
		w.text(parameter.Declaration.Kind)
		var state packedLocationState
		w.internalLocation(parameter.Declaration.Location, &state)
	}
	w.u64(callability)
	if parameter.TypeDescriptor != nil {
		w.internalTypeDescriptor(parameter.TypeDescriptor)
	}
	return nil
}

func writeWireTransitionFileBody(w *packedWriter, file FileFact) {
	w.u64(uint64(len(file.Calls)))
	for _, call := range file.Calls {
		w.internalSourceCall(call)
	}
	w.u64(uint64(len(file.Bindings)))
	for _, binding := range file.Bindings {
		flags := uint64(0)
		if binding.Array {
			flags |= bindingFlagArray
		}
		w.u64(flags)
		w.internalLocations(binding.Names)
		w.internalSourceCall(binding.Initializer)
	}
	w.u64(uint64(len(file.Functions)))
	for _, function := range file.Functions {
		var state packedLocationState
		w.internalLocation(function.Name, &state)
		w.internalLocation(function.Body, &state)
		w.internalLocations(function.Parameters)
		flags := uint64(0)
		if function.Exported {
			flags |= functionFlagExported
		}
		if function.Async {
			flags |= functionFlagAsync
		}
		if function.Arrow {
			flags |= functionFlagArrow
		}
		w.u64(flags)
	}
	w.u64(uint64(len(file.AsyncFunctions)))
	for _, function := range file.AsyncFunctions {
		var state packedLocationState
		w.internalLocation(function.Expression, &state)
		w.text(string(function.Symbol))
		w.text(string(function.Target))
		flags := uint64(0)
		if function.CanReturnAsync {
			flags |= asyncFunctionFlagCanReturnAsync
		}
		w.u64(flags)
		w.internalLocations(function.CallsAfterAwait)
	}
}

func writeWireTransitionSymbolOp(
	w *packedWriter,
	mode wireTransitionMode,
	operation *wireTransitionSymbolOp,
) error {
	if operation.id == "" {
		return fmt.Errorf("symbol operation has an empty ID")
	}
	switch operation.tag {
	case wireTransitionReplaceSymbol,
		wireTransitionRemoveSymbol,
		wireTransitionReplaceReferencePath:
	default:
		return fmt.Errorf("symbol operation tag = %d", operation.tag)
	}
	id := w.dict.intern(string(operation.id))
	if mode == wireTransitionFull {
		if operation.tag != wireTransitionReplaceSymbol {
			return fmt.Errorf("full symbol operation tag = %d, want replace", operation.tag)
		}
		w.u64(id)
	} else {
		w.u64(id<<2 | uint64(operation.tag))
	}
	switch operation.tag {
	case wireTransitionReplaceSymbol:
		if operation.fact.ID != operation.id {
			return fmt.Errorf(
				"symbol replacement ID = %q, operation ID = %q",
				operation.fact.ID,
				operation.id,
			)
		}
		w.text(string(operation.fact.AliasTarget))
		w.internalDeclarations(operation.fact.Declarations)
		if operation.fact.AliasTarget == "" {
			w.internalLocations(operation.fact.References)
		} else {
			w.u64(0)
		}
	case wireTransitionRemoveSymbol:
	case wireTransitionReplaceReferencePath:
		if operation.referencePath == "" {
			return fmt.Errorf("symbol reference operation has an empty path")
		}
		w.text(operation.referencePath)
		for index, reference := range operation.references {
			if reference.Path != operation.referencePath {
				return fmt.Errorf(
					"reference %d path = %q, operation path = %q",
					index,
					reference.Path,
					operation.referencePath,
				)
			}
			if err := validateWireTransitionLocation(reference); err != nil {
				return fmt.Errorf("reference %d location: %w", index, err)
			}
		}
		w.internalLocations(operation.references)
	}
	return nil
}

func validateWireTransitionLocation(location Location) error {
	if location.StartByte < 0 {
		return fmt.Errorf("negative start byte %d", location.StartByte)
	}
	if location.EndByte < location.StartByte {
		return fmt.Errorf(
			"end byte %d precedes start byte %d",
			location.EndByte,
			location.StartByte,
		)
	}
	return nil
}

func wireTransitionValidityCode(value ResolvedCallValidity) (uint64, error) {
	switch value {
	case ResolvedCallValid:
		return 0, nil
	case ResolvedCallRecovery:
		return 1, nil
	case ResolvedCallUnresolved:
		return 2, nil
	default:
		return 0, fmt.Errorf("unknown resolved-call validity %q", value)
	}
}

func wireTransitionCallKindCode(value CallKind) (uint64, error) {
	switch value {
	case CallKindUnknown:
		return 0, nil
	case CallKindCall:
		return 1, nil
	case CallKindConstruct:
		return 2, nil
	default:
		return 0, fmt.Errorf("unknown call kind %q", value)
	}
}

func wireTransitionMappingStatusCode(value ArgumentMappingStatus) (uint64, error) {
	switch value {
	case ArgumentMappingResolved:
		return 0, nil
	case ArgumentMappingUnresolved:
		return 1, nil
	default:
		return 0, fmt.Errorf("unknown argument-mapping status %q", value)
	}
}

func wireTransitionMappingReasonCode(value ArgumentMappingReason) (uint64, error) {
	switch value {
	case "":
		return 0, nil
	case ArgumentMappingCallUnresolved:
		return 1, nil
	case ArgumentMappingRecoverySignature:
		return 2, nil
	case ArgumentMappingCompositeSignature:
		return 3, nil
	case ArgumentMappingSpreadArgument:
		return 4, nil
	case ArgumentMappingParameterUnavailable:
		return 5, nil
	default:
		return 0, fmt.Errorf("unknown argument-mapping reason %q", value)
	}
}

func wireTransitionCallabilityCode(value Callability) (uint64, error) {
	switch value {
	case CallabilityCallable:
		return 0, nil
	case CallabilityNonCallable:
		return 1, nil
	case CallabilityMixed:
		return 2, nil
	case CallabilityUnknown:
		return 3, nil
	default:
		return 0, fmt.Errorf("unknown callability %q", value)
	}
}

func wireTransitionRuntimeValueDomainBits(value RuntimeValueDomain) uint64 {
	var bits uint64
	if value.MayBeCallable {
		bits |= 1 << 0
	}
	if value.MayBeUndefined {
		bits |= 1 << 1
	}
	if value.MayBeOther {
		bits |= 1 << 2
	}
	if value.Unknown {
		bits |= 1 << 3
	}
	return bits
}

func wireTransitionReferenceSpaceCode(value ReferenceSpace) (uint64, error) {
	switch value {
	case ReferenceSpaceValue:
		return 0, nil
	case ReferenceSpaceType:
		return 1, nil
	case ReferenceSpaceBoth:
		return 2, nil
	case ReferenceSpaceNeither:
		return 3, nil
	default:
		return 0, fmt.Errorf("unknown reference space %q", value)
	}
}
