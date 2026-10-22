//! Row writers (Go: `wire_transition_rows.go`). Each mirrors one decoder
//! reader in `v3.rs`, and both name their flags from [`crate::v3::rows`].

use super::EncodeError;
use super::plan::{PathOpRef, Slot, SymbolBody, SymbolOpRef};
use super::writer::{LocationState, RowWriter};
use crate::v3::{
    ASYNC_FUNCTION_FLAG_CAN_RETURN_ASYNC, BINDING_FLAG_ARRAY, FUNCTION_FLAG_ARROW,
    FUNCTION_FLAG_ASYNC, FUNCTION_FLAG_EXPORTED, TYPE_FACTS_TABLE_SCHEMA_V5,
    TYPE_FACTS_TABLE_SCHEMA_V6, TYPE_FACTS_TABLE_SCHEMA_V7, TYPE_FACTS_TABLE_SCHEMA_V8,
    TYPE_FACTS_TABLE_SCHEMA_V9, TYPE_FACTS_TABLE_SCHEMA_V11, TYPE_FACTS_TABLE_SCHEMA_V12,
    TYPE_FACTS_TABLE_SCHEMA_V13, TYPE_FACTS_TABLE_SCHEMA_V14, TYPE_FACTS_TABLE_SCHEMA_V15,
    TYPE_FACTS_TABLE_SCHEMA_V16, TYPE_FACTS_TABLE_SCHEMA_V17, TYPE_FACTS_TABLE_SCHEMA_V18,
    TYPE_FACTS_TABLE_SCHEMA_V19, TYPE_FACTS_TABLE_SCHEMA_V20, TYPE_FACTS_TABLE_SCHEMA_V21,
    TransitionMode, rows,
};
use crate::{
    ArgumentMapping, ArgumentMappingReason, ArgumentMappingStatus, ArrayShape, CallKind,
    Callability, ConstantValueKind, Constructability, ConstructionWitness, Declaration, EntityFact,
    FileFact, Location, ParameterFact, PrimitiveLiteralKind, PrimitiveValueDomain, ReferenceSpace,
    ResolvedCall, ResolvedCallValidity, ResolvedDeclaration, RuntimeBindingKind,
    RuntimeValueDomain, SourceCall, SourceHash, SymbolClaim, TypeDescriptor,
};

pub(super) fn write_path_op(
    w: &mut RowWriter<'_>,
    operation: &PathOpRef<'_>,
    schema: u64,
) -> Result<(), EncodeError> {
    if operation.path.is_empty() {
        return Err(EncodeError::invalid(
            "path operation has an empty path".to_owned(),
        ));
    }
    let flags = operation.source.tag() << rows::PATH_SOURCE_SHIFT
        | operation.entities.tag() << rows::PATH_ENTITY_SHIFT
        | operation.file.tag() << rows::PATH_FILE_SHIFT;
    w.text(operation.path);
    w.u64(flags);

    if let Slot::Replace(source) = operation.source {
        w.raw(&digest_bytes(source)?);
    }
    if let Slot::Replace(entities) = operation.entities {
        write_entity_run(w, operation.path, entities, schema).map_err(|error| {
            EncodeError::invalid(format!(
                "entity replacement for {:?}: {error}",
                operation.path
            ))
        })?;
    }
    if let Slot::Replace(file) = operation.file {
        if file.path.as_ref() != operation.path {
            return Err(EncodeError::invalid(format!(
                "file replacement path = {:?}, operation path = {:?}",
                file.path, operation.path
            )));
        }
        write_file_body(w, file)?;
    }
    Ok(())
}

fn digest_bytes(hash: &SourceHash) -> Result<[u8; 32], EncodeError> {
    let digits = hash
        .as_str()
        .strip_prefix(crate::SHA256_PREFIX)
        .filter(|digits| digits.len() == 64)
        .ok_or_else(|| EncodeError::invalid(format!("source digest {hash} is not sha256")))?;
    let mut raw = [0_u8; 32];
    for (slot, pair) in raw.iter_mut().zip(digits.as_bytes().as_chunks::<2>().0) {
        let high = char::from(pair[0]).to_digit(16);
        let low = char::from(pair[1]).to_digit(16);
        let (Some(high), Some(low)) = (high, low) else {
            return Err(EncodeError::invalid(format!(
                "source digest {hash} is not hexadecimal"
            )));
        };
        *slot = (high * 16 + low) as u8;
    }
    Ok(raw)
}

fn validate_location(location: &Location) -> Result<(), EncodeError> {
    if location.end_byte < location.start_byte {
        return Err(EncodeError::invalid(format!(
            "end byte {} precedes start byte {}",
            location.end_byte, location.start_byte
        )));
    }
    Ok(())
}

fn write_entity_run(
    w: &mut RowWriter<'_>,
    path: &str,
    entities: &[EntityFact],
    schema: u64,
) -> Result<(), EncodeError> {
    w.u64(entities.len() as u64);
    let mut previous_start: i64 = 0;
    for (index, entity) in entities.iter().enumerate() {
        let in_entity = |error: EncodeError| error.within(&format!("entity {index}"));
        if entity.location.path.as_ref() != path {
            return Err(EncodeError::invalid(format!(
                "entity {index} path = {:?}, run path = {path:?}",
                entity.location.path
            )));
        }
        validate_location(&entity.location)
            .map_err(|error| error.within(&format!("entity {index} location")))?;
        match entity.symbol_claim {
            SymbolClaim::Unresolved if !entity.symbol.is_empty() => {
                return Err(EncodeError::invalid(format!(
                    "entity {index} cannot be both resolved and unresolved"
                )));
            }
            SymbolClaim::Covering if entity.symbol.is_empty() => {
                return Err(EncodeError::invalid(format!(
                    "entity {index} covers its span without a symbol"
                )));
            }
            _ => {}
        }

        let callability = entity
            .callability
            .map(|value| callability_code(value, schema));
        let constructability = entity.constructability.map(constructability_code);
        let reference_space = entity.reference_space.map(reference_space_code);
        let array_shape = entity.array_shape.map(array_shape_code);
        if let Some(constant) = &entity.constant_value
            && constant.kind == ConstantValueKind::Number
            && constant.number.is_nan()
        {
            return Err(in_entity(EncodeError::invalid(
                "constant number is NaN".to_owned(),
            )));
        }

        let start = i64::try_from(entity.location.start_byte)
            .map_err(|_| EncodeError::invalid(format!("entity {index} start overflows")))?;
        w.signed(start - previous_start);
        w.u64(entity.location.end_byte - entity.location.start_byte);
        w.text_arc(&entity.symbol);

        let library_types = entity
            .library_types
            .as_ref()
            .filter(|names| !names.is_empty());
        let literal_candidates = entity
            .primitive_literal_candidates
            .as_ref()
            .filter(|candidates| !candidates.is_empty());
        let mut flags = 0;
        let mut set = |present: bool, bit: u64| {
            if present {
                flags |= bit;
            }
        };
        set(
            entity.type_descriptor.is_some(),
            rows::ENTITY_TYPE_DESCRIPTOR,
        );
        set(entity.resolved_call.is_some(), rows::ENTITY_RESOLVED_CALL);
        set(callability.is_some(), rows::ENTITY_CALLABILITY);
        set(reference_space.is_some(), rows::ENTITY_REFERENCE_SPACE);
        set(
            !entity.runtime_identity.is_empty(),
            rows::ENTITY_RUNTIME_IDENTITY,
        );
        set(
            entity.runtime_value_domain.is_some(),
            rows::ENTITY_RUNTIME_VALUE_DOMAIN,
        );
        set(
            schema >= TYPE_FACTS_TABLE_SCHEMA_V7 && entity.call_result_domain.is_some(),
            rows::ENTITY_CALL_RESULT_DOMAIN,
        );
        set(
            schema >= TYPE_FACTS_TABLE_SCHEMA_V8 && entity.constant_value.is_some(),
            rows::ENTITY_CONSTANT_VALUE,
        );
        set(
            schema >= TYPE_FACTS_TABLE_SCHEMA_V9 && array_shape.is_some(),
            rows::ENTITY_ARRAY_SHAPE,
        );
        set(
            schema >= TYPE_FACTS_TABLE_SCHEMA_V11 && entity.tuple_shape.is_some(),
            rows::ENTITY_TUPLE_SHAPE,
        );
        set(
            schema >= TYPE_FACTS_TABLE_SCHEMA_V12 && library_types.is_some(),
            rows::ENTITY_LIBRARY_TYPES,
        );
        set(
            schema >= TYPE_FACTS_TABLE_SCHEMA_V13 && entity.primitive_value_domain.is_present(),
            rows::ENTITY_PRIMITIVE_VALUE_DOMAIN,
        );
        set(
            schema >= TYPE_FACTS_TABLE_SCHEMA_V14 && constructability.is_some(),
            rows::ENTITY_CONSTRUCTABILITY,
        );
        set(
            schema >= TYPE_FACTS_TABLE_SCHEMA_V16 && literal_candidates.is_some(),
            rows::ENTITY_PRIMITIVE_LITERAL_CANDIDATES,
        );
        set(
            schema >= TYPE_FACTS_TABLE_SCHEMA_V18 && entity.runtime_binding_kind.is_some(),
            rows::ENTITY_RUNTIME_BINDING_KIND,
        );
        set(
            schema >= TYPE_FACTS_TABLE_SCHEMA_V5 && entity.symbol_claim == SymbolClaim::Unresolved,
            rows::ENTITY_SYMBOL_FLAG,
        );
        // A producer at an older table schema drops the covering claim: the
        // reader then sees a symbol that does not cover its span, which
        // withholds it.
        set(
            schema >= TYPE_FACTS_TABLE_SCHEMA_V21 && entity.symbol_claim == SymbolClaim::Covering,
            rows::ENTITY_SYMBOL_FLAG,
        );
        w.u64(flags);

        if let Some(descriptor) = &entity.type_descriptor {
            write_type_descriptor(w, descriptor)?;
        }
        if let Some(call) = &entity.resolved_call {
            write_resolved_call(w, call, schema)
                .map_err(|error| error.within(&format!("entity {index} resolved call")))?;
        }
        if let Some(code) = callability {
            w.u64(code);
        }
        if let Some(code) = reference_space {
            w.u64(code);
        }
        if !entity.runtime_identity.is_empty() {
            w.text_arc(&entity.runtime_identity);
        }
        if let Some(domain) = entity.runtime_value_domain {
            w.u64(runtime_value_domain_bits(domain));
        }
        if schema >= TYPE_FACTS_TABLE_SCHEMA_V7
            && let Some(domain) = entity.call_result_domain
        {
            w.u64(runtime_value_domain_bits(domain));
        }
        if schema >= TYPE_FACTS_TABLE_SCHEMA_V8
            && let Some(constant) = &entity.constant_value
        {
            match constant.kind {
                ConstantValueKind::String => {
                    w.u64(0);
                    w.text_arc(&constant.string);
                }
                ConstantValueKind::Number => {
                    w.u64(1);
                    w.u64(constant.number.to_bits());
                }
            }
        }
        if schema >= TYPE_FACTS_TABLE_SCHEMA_V9
            && let Some(code) = array_shape
        {
            w.u64(code);
        }
        if schema >= TYPE_FACTS_TABLE_SCHEMA_V11
            && let Some(shape) = entity.tuple_shape
        {
            // One packed word: the fixed slot count with the rest flag in
            // bit 0, then element zero's callability code.
            w.u64(u64::from(shape.fixed_length()) << 1 | u64::from(shape.has_rest()));
            w.u64(callability_code(
                shape.element_zero().unwrap_or(Callability::Unknown),
                schema,
            ));
            w.u64(u64::from(shape.element_zero_min_parameters()));
            if schema >= TYPE_FACTS_TABLE_SCHEMA_V13 {
                w.u64(
                    shape
                        .exact_length()
                        .map_or(0, |length| u64::from(length) + 1),
                );
            }
        }
        if schema >= TYPE_FACTS_TABLE_SCHEMA_V12
            && let Some(names) = library_types
        {
            w.u64(names.len() as u64);
            for name in names.iter() {
                w.text_arc(name);
            }
        }
        if schema >= TYPE_FACTS_TABLE_SCHEMA_V13 && entity.primitive_value_domain.is_present() {
            w.u64(primitive_value_domain_bits(entity.primitive_value_domain));
        }
        if schema >= TYPE_FACTS_TABLE_SCHEMA_V14
            && let Some(code) = constructability
        {
            w.u64(code);
        }
        if schema >= TYPE_FACTS_TABLE_SCHEMA_V16
            && let Some(candidates) = literal_candidates
        {
            w.u64(candidates.len() as u64);
            for candidate in candidates.iter() {
                match candidate.kind {
                    PrimitiveLiteralKind::String => {
                        w.u64(0);
                        w.text_arc(&candidate.string);
                    }
                    PrimitiveLiteralKind::Number => {
                        if !candidate.number.is_finite() {
                            return Err(in_entity(EncodeError::invalid(
                                "non-finite primitive literal candidate".to_owned(),
                            )));
                        }
                        w.u64(1);
                        w.u64(candidate.number.to_bits());
                    }
                    PrimitiveLiteralKind::Boolean => {
                        w.u64(2);
                        w.bool(candidate.boolean);
                    }
                }
            }
        }
        if schema >= TYPE_FACTS_TABLE_SCHEMA_V18
            && let Some(kind) = entity.runtime_binding_kind
        {
            w.u64(match kind {
                RuntimeBindingKind::Callable => 0,
                RuntimeBindingKind::NonCallable => 1,
                RuntimeBindingKind::Mixed => 2,
                RuntimeBindingKind::Open => 3,
            });
        }
        previous_start = start;
    }
    Ok(())
}

fn write_type_descriptor(
    w: &mut RowWriter<'_>,
    descriptor: &TypeDescriptor,
) -> Result<(), EncodeError> {
    w.text_arc(&descriptor.text);
    w.text_arc(&descriptor.origin_module);
    write_declarations(w, &descriptor.alias_declarations)
}

fn write_declarations(
    w: &mut RowWriter<'_>,
    declarations: &[Declaration],
) -> Result<(), EncodeError> {
    w.u64(declarations.len() as u64);
    let mut state = LocationState::default();
    for declaration in declarations {
        w.text_arc(&declaration.name);
        w.text_arc(&declaration.kind);
        w.location(&declaration.location, &mut state)?;
    }
    Ok(())
}

fn write_resolved_declaration(
    w: &mut RowWriter<'_>,
    declaration: &ResolvedDeclaration,
) -> Result<(), EncodeError> {
    w.text_arc(&declaration.symbol);
    w.text_arc(&declaration.name);
    w.text_arc(&declaration.kind);
    let mut state = LocationState::default();
    w.location(&declaration.location, &mut state)?;
    w.u64(declaration.owners.len() as u64);
    for owner in declaration.owners.iter() {
        w.text_arc(&owner.symbol);
        w.text_arc(&owner.name);
        w.text_arc(&owner.kind);
        w.location(&owner.location, &mut state)?;
    }
    w.text_arc(&declaration.qualified_name);
    w.text_arc(&declaration.origin_module);
    w.text_arc(&declaration.source_file);
    w.bool(declaration.standard_library);
    Ok(())
}

fn write_resolved_call(
    w: &mut RowWriter<'_>,
    call: &ResolvedCall,
    schema: u64,
) -> Result<(), EncodeError> {
    let validity = validity_code(call.validity);
    let kind = call_kind_code(call.kind);

    w.text_arc(&call.target);
    w.text_arc(&call.return_type_text);
    w.u64(validity);
    w.u64(kind);
    w.bool(call.declaration.is_some());
    if let Some(declaration) = &call.declaration {
        write_resolved_declaration(w, declaration)?;
    }
    if schema >= TYPE_FACTS_TABLE_SCHEMA_V6 {
        w.bool(call.targets.is_some());
        if let Some(targets) = &call.targets {
            if targets.candidates.is_empty() {
                return Err(EncodeError::invalid(
                    "resolved-call target set has no candidates".to_owned(),
                ));
            }
            w.bool(targets.exhaustive);
            w.u64(targets.candidates.len() as u64);
            for (index, candidate) in targets.candidates.iter().enumerate() {
                if candidate.symbol.is_empty() {
                    return Err(EncodeError::invalid(format!(
                        "target candidate {index} has no symbol"
                    )));
                }
                write_resolved_declaration(w, candidate)?;
            }
        }
    }
    w.u64(call.arguments.len() as u64);
    for (index, mapping) in call.arguments.iter().enumerate() {
        write_argument_mapping(w, index, mapping, schema)?;
    }
    if schema >= TYPE_FACTS_TABLE_SCHEMA_V19 {
        w.u64(
            (u64::from(call.assertion_signature) * rows::CALL_ASSERTION_SIGNATURE)
                | (u64::from(call.never_returns) * rows::CALL_NEVER_RETURNS),
        );
    }
    if schema >= TYPE_FACTS_TABLE_SCHEMA_V20 {
        w.bool(call.default_library_binding.is_some());
        if let Some(binding) = &call.default_library_binding {
            write_resolved_declaration(w, binding)?;
        }
    }
    Ok(())
}

fn write_argument_mapping(
    w: &mut RowWriter<'_>,
    index: usize,
    mapping: &ArgumentMapping,
    schema: u64,
) -> Result<(), EncodeError> {
    let status = match mapping.status {
        ArgumentMappingStatus::Resolved => 0,
        ArgumentMappingStatus::Unresolved => 1,
    };
    let reason = match mapping.unresolved {
        None => 0,
        Some(ArgumentMappingReason::CallUnresolved) => 1,
        Some(ArgumentMappingReason::RecoverySignature) => 2,
        Some(ArgumentMappingReason::CompositeSignature) => 3,
        Some(ArgumentMappingReason::SpreadArgument) => 4,
        Some(ArgumentMappingReason::ParameterUnavailable) => 5,
    };
    if (status == 0) != (reason == 0) {
        return Err(EncodeError::invalid(format!(
            "argument mapping {index} status {:?} and reason {:?} disagree",
            mapping.status, mapping.unresolved
        )));
    }
    w.u64(mapping.argument_index);
    w.u64(status);
    w.u64(reason);
    w.bool(mapping.parameter.is_some());
    if let Some(parameter) = &mapping.parameter {
        write_parameter(w, parameter, schema)?;
    }
    Ok(())
}

fn write_parameter(
    w: &mut RowWriter<'_>,
    parameter: &ParameterFact,
    schema: u64,
) -> Result<(), EncodeError> {
    let object_shape = (schema >= TYPE_FACTS_TABLE_SCHEMA_V17)
        .then_some(parameter.object_shape.as_ref())
        .flatten();
    w.u64(parameter.index);
    w.text_arc(&parameter.symbol);
    let mut flags = 0;
    if parameter.declaration.is_some() {
        flags |= rows::PARAMETER_DECLARATION;
    }
    if parameter.rest {
        flags |= rows::PARAMETER_REST;
    }
    if parameter.optional {
        flags |= rows::PARAMETER_OPTIONAL;
    }
    if parameter.type_descriptor.is_some() {
        flags |= rows::PARAMETER_TYPE_DESCRIPTOR;
    }
    if object_shape.is_some() {
        flags |= rows::PARAMETER_OBJECT_SHAPE;
    }
    w.u64(flags);
    if let Some(declaration) = &parameter.declaration {
        w.text_arc(&declaration.name);
        w.text_arc(&declaration.kind);
        let mut state = LocationState::default();
        w.location(&declaration.location, &mut state)?;
    }
    w.u64(callability_code(parameter.callability, schema));
    if let Some(descriptor) = &parameter.type_descriptor {
        write_type_descriptor(w, descriptor)?;
    }
    if let Some(shape) = object_shape {
        w.u64(shape.required_properties.len() as u64);
        for property in shape.required_properties.iter() {
            w.text_arc(&property.name);
            w.u64(match property.witness {
                ConstructionWitness::Unknown => 0,
                ConstructionWitness::EmptyArray => 1,
                ConstructionWitness::EmptyObject => 2,
            });
        }
    }
    Ok(())
}

fn write_source_call(w: &mut RowWriter<'_>, call: &SourceCall) -> Result<(), EncodeError> {
    let mut state = LocationState::default();
    w.location(&call.location, &mut state)?;
    w.location(&call.callee, &mut state)?;
    w.locations(&call.arguments)?;
    w.text_arc(&call.target);
    Ok(())
}

fn write_file_body(w: &mut RowWriter<'_>, file: &FileFact) -> Result<(), EncodeError> {
    w.u64(file.calls.len() as u64);
    for call in file.calls.iter() {
        write_source_call(w, call)?;
    }
    w.u64(file.bindings.len() as u64);
    for binding in file.bindings.iter() {
        w.u64(if binding.array { BINDING_FLAG_ARRAY } else { 0 });
        w.locations(&binding.names)?;
        write_source_call(w, &binding.initializer)?;
    }
    w.u64(file.functions.len() as u64);
    for function in file.functions.iter() {
        let mut state = LocationState::default();
        w.location(&function.name, &mut state)?;
        w.location(&function.body, &mut state)?;
        w.locations(&function.parameters)?;
        let mut flags = 0;
        if function.exported {
            flags |= FUNCTION_FLAG_EXPORTED;
        }
        if function.r#async {
            flags |= FUNCTION_FLAG_ASYNC;
        }
        if function.arrow {
            flags |= FUNCTION_FLAG_ARROW;
        }
        w.u64(flags);
    }
    w.u64(file.async_functions.len() as u64);
    for function in file.async_functions.iter() {
        let mut state = LocationState::default();
        w.location(&function.expression, &mut state)?;
        w.text_arc(&function.symbol);
        w.text_arc(&function.target);
        w.u64(if function.can_return_async {
            ASYNC_FUNCTION_FLAG_CAN_RETURN_ASYNC
        } else {
            0
        });
        w.locations(&function.calls_after_await)?;
    }
    Ok(())
}

pub(super) fn write_symbol_op(
    w: &mut RowWriter<'_>,
    mode: TransitionMode,
    operation: &SymbolOpRef<'_>,
) -> Result<(), EncodeError> {
    if operation.id.is_empty() {
        return Err(EncodeError::invalid(
            "symbol operation has an empty ID".to_owned(),
        ));
    }
    let id = w.intern(operation.id);
    match mode {
        TransitionMode::Full => {
            if !matches!(operation.body, SymbolBody::Replace(_)) {
                return Err(EncodeError::invalid(format!(
                    "full symbol operation tag = {}, want replace",
                    operation.body.tag()
                )));
            }
            w.u64(id);
        }
        TransitionMode::Delta => w.u64(id << 2 | operation.body.tag()),
    }
    match operation.body {
        SymbolBody::Replace(fact) => {
            if fact.id.as_ref() != operation.id {
                return Err(EncodeError::invalid(format!(
                    "symbol replacement ID = {:?}, operation ID = {:?}",
                    fact.id, operation.id
                )));
            }
            w.text_arc(&fact.alias_target);
            write_declarations(w, &fact.declarations)?;
            if fact.alias_target.is_empty() {
                w.locations(&fact.references)?;
            } else {
                w.u64(0);
            }
        }
        SymbolBody::Remove => {}
        SymbolBody::ReplaceReferencePath { path, references } => {
            if path.is_empty() {
                return Err(EncodeError::invalid(
                    "symbol reference operation has an empty path".to_owned(),
                ));
            }
            w.text(path);
            for (index, reference) in references.iter().enumerate() {
                if reference.path.as_ref() != path {
                    return Err(EncodeError::invalid(format!(
                        "reference {index} path = {:?}, operation path = {path:?}",
                        reference.path
                    )));
                }
                validate_location(reference)
                    .map_err(|error| error.within(&format!("reference {index} location")))?;
            }
            w.locations(references)?;
        }
    }
    Ok(())
}

fn validity_code(value: ResolvedCallValidity) -> u64 {
    match value {
        ResolvedCallValidity::Valid => 0,
        ResolvedCallValidity::Recovery => 1,
        ResolvedCallValidity::Unresolved => 2,
    }
}

fn call_kind_code(value: CallKind) -> u64 {
    match value {
        CallKind::Unknown => 0,
        CallKind::Call => 1,
        CallKind::Construct => 2,
    }
}

/// Maps callability onto its closed tag space. Tag 4 (untypedCallable) joined
/// the space at table schema v15; a decoder for v14 or earlier refuses it, so
/// emission at those schemas degrades the value to unknown, the one answer that
/// stays true when the vocabulary carrying it cannot be expressed:
/// untypedCallable is "callable, with no signature to read" and unknown claims
/// nothing at all. It never degrades to nonCallable, which would turn an absent
/// answer into a negative one.
pub(super) fn callability_code(value: Callability, schema: u64) -> u64 {
    let value = if value == Callability::UntypedCallable && schema < TYPE_FACTS_TABLE_SCHEMA_V15 {
        Callability::Unknown
    } else {
        value
    };
    match value {
        Callability::Callable => 0,
        Callability::NonCallable => 1,
        Callability::Mixed => 2,
        Callability::Unknown => 3,
        Callability::UntypedCallable => 4,
    }
}

/// Constructability keeps its own code space rather than borrowing
/// callability's. The two facts are separate rows and nothing may silently
/// decode one as the other if either vocabulary ever grows.
fn constructability_code(value: Constructability) -> u64 {
    match value {
        Constructability::Constructable => 0,
        Constructability::NonConstructable => 1,
        Constructability::Mixed => 2,
        Constructability::Unknown => 3,
    }
}

fn array_shape_code(value: ArrayShape) -> u64 {
    match value {
        ArrayShape::Array => 0,
        ArrayShape::NotArray => 1,
        ArrayShape::Mixed => 2,
        ArrayShape::Unknown => 3,
    }
}

fn reference_space_code(value: ReferenceSpace) -> u64 {
    match value {
        ReferenceSpace::Value => 0,
        ReferenceSpace::Type => 1,
        ReferenceSpace::Both => 2,
        ReferenceSpace::Neither => 3,
    }
}

fn runtime_value_domain_bits(value: RuntimeValueDomain) -> u64 {
    u64::from(value.may_be_callable())
        | u64::from(value.may_be_undefined()) << 1
        | u64::from(value.may_be_other()) << 2
        | u64::from(value.unknown()) << 3
}

fn primitive_value_domain_bits(value: PrimitiveValueDomain) -> u64 {
    u64::from(value.may_be_string())
        | u64::from(value.may_be_number()) << 1
        | u64::from(value.may_be_boolean()) << 2
        | u64::from(value.may_be_big_int()) << 3
        | u64::from(value.may_be_symbol()) << 4
        | u64::from(value.may_be_null()) << 5
        | u64::from(value.may_be_undefined()) << 6
        | u64::from(value.may_be_object()) << 7
        | u64::from(value.unknown()) << 8
        | u64::from(value.numbers_are_finite()) << 9
}
