//! Type-level entity facts (Go: the type half of `tsgo/semantic_entities.go`):
//! type descriptors, callability, constructability, runtime value domains,
//! primitive value domains and primitive literal candidates.
//!
//! Every function here reads checker types only and returns plain values: no
//! tsrs handle outlives the call (`unsafe_boundary` invariant 1). Rendered
//! type text feeds `TypeDescriptor.text` and nothing else; no domain is ever
//! derived from it.
//!
//! The recursive walks keep Go's `seen` set (a type being classified is
//! "cycling" and answers unknown), keyed by type address.

use crate::project::declarations_for_symbol;
use std::collections::BTreeSet;
use std::collections::HashSet;
use std::sync::Arc;
use tsrs_ast::{
    Node, Symbol, SymbolFlags, get_source_file_of_node, is_module_block, is_module_declaration,
    is_private_identifier, is_property_access_expression, is_source_file,
};
use tsrs_checker::types::{
    ElementFlags, LiteralValue, ObjectFlags, SignatureKind, Type, TypeExt, TypeFlags,
};
use tsrs_compiler::Checker;
use tsrs_compiler::Program;
use tsrs_core::P;
use typefacts::{
    ArrayShape, Callability, Constructability, PrimitiveLiteralCandidate, PrimitiveLiteralKind,
    PrimitiveValueDomain, RuntimeValueDomain, TupleShape, TypeDescriptor,
};

/// Go: `TypeFlagsAny|Unknown|Never|IncludesError`.
const OPEN: TypeFlags = TypeFlags::Any
    .union(TypeFlags::Unknown)
    .union(TypeFlags::Never)
    .union(TypeFlags::IncludesError);

/// Go: `maxPrimitiveLiteralCandidates`.
const MAX_PRIMITIVE_LITERAL_CANDIDATES: usize = 32;

/// Go: `declarationModule`: the name of the first enclosing module
/// declaration of any declaration of `symbol`, or empty.
pub(crate) fn declaration_module(symbol: P<Symbol>) -> String {
    for declaration in symbol.declarations() {
        let mut node = Some(*declaration);
        while let Some(current) = node {
            if is_module_declaration(current)
                && let Some(name) = current.name()
            {
                return name.text().to_owned();
            }
            node = current.parent();
        }
    }
    String::new()
}

/// Go: `typeDescriptorFor` without the memo (the caller memoizes per type).
pub fn type_descriptor_for(checker: &mut Checker, value: P<Type>) -> TypeDescriptor {
    let mut origin_module = String::new();
    let mut alias_declarations = Vec::new();
    if let Some(alias) = value.alias()
        && let Some(symbol) = alias.symbol()
    {
        alias_declarations = declarations_for_symbol(symbol);
        origin_module = declaration_module(symbol);
    }
    TypeDescriptor {
        text: checker.type_to_string_exported(value).into(),
        origin_module: origin_module.into(),
        alias_declarations: alias_declarations.into(),
    }
}

/// The per-call memo of `project.typeDescriptors`, keyed by type address. A
/// type address is valid for the checker that issued it, and this memo lives
/// no longer than one backend call.
#[derive(Default)]
pub struct DescriptorMemo(std::collections::HashMap<usize, Arc<TypeDescriptor>>);

impl DescriptorMemo {
    pub fn descriptor(&mut self, checker: &mut Checker, value: P<Type>) -> Arc<TypeDescriptor> {
        if let Some(held) = self.0.get(&value.addr()) {
            return Arc::clone(held);
        }
        let descriptor = Arc::new(type_descriptor_for(checker, value));
        self.0.insert(value.addr(), Arc::clone(&descriptor));
        descriptor
    }
}

fn is_open(value: P<Type>) -> bool {
    value.flags().intersects(OPEN)
}

/// Go: `callabilityOfType`.
pub fn callability_of_type(checker: &mut Checker, value: P<Type>) -> Callability {
    if is_open(value) {
        return Callability::Unknown;
    }
    let constituents = value.distributed();
    if constituents.is_empty() {
        return Callability::Unknown;
    }
    let (mut callable, mut untyped_callable, mut non_callable) = (false, false, false);
    for constituent in constituents {
        if is_open(constituent) {
            return Callability::Unknown;
        }
        if !checker
            .get_signatures_of_type_exported(constituent, SignatureKind::Call)
            .is_empty()
        {
            callable = true;
        } else if callable_without_signatures(checker, constituent) {
            untyped_callable = true;
        } else {
            non_callable = true;
        }
    }
    if non_callable && (callable || untyped_callable) {
        Callability::Mixed
    } else if untyped_callable {
        // At a union this rung promises only that every constituent answered
        // callable in some sense, per constituent: not that the union's call
        // as written compiles (see the Go comment for both directions).
        Callability::UntypedCallable
    } else if callable {
        Callability::Callable
    } else {
        Callability::NonCallable
    }
}

/// Go: `callableWithoutSignatures`: the compiler's own untyped-call rule
/// (`isUntypedFunctionCall`) asked with the apparent type its call-resolution
/// sites use, for a constituent with no call or construct signature. The
/// caller has already excluded any/unknown/never/error constituents; the
/// exclusion is repeated against the apparent type as a defensive guard.
fn callable_without_signatures(checker: &mut Checker, value: P<Type>) -> bool {
    if !checker
        .get_signatures_of_type_exported(value, SignatureKind::Construct)
        .is_empty()
    {
        return false;
    }
    let apparent = checker.get_reduced_apparent_type_exported(value);
    if is_open(apparent) {
        return false;
    }
    checker.is_untyped_function_call_exported(value, apparent, 0, 0)
}

/// Go: `constructabilityOfType`: callability asked of construct signatures,
/// with no `untypedCallable` counterpart (`new x()` has no untyped fallback).
pub fn constructability_of_type(checker: &mut Checker, value: P<Type>) -> Constructability {
    if is_open(value) {
        return Constructability::Unknown;
    }
    let constituents = value.distributed();
    if constituents.is_empty() {
        return Constructability::Unknown;
    }
    let (mut constructable, mut non_constructable) = (false, false);
    for constituent in constituents {
        if is_open(constituent) {
            return Constructability::Unknown;
        }
        if checker
            .get_signatures_of_type_exported(constituent, SignatureKind::Construct)
            .is_empty()
        {
            non_constructable = true;
        } else {
            constructable = true;
        }
    }
    match (constructable, non_constructable) {
        (true, true) => Constructability::Mixed,
        (true, false) => Constructability::Constructable,
        _ => Constructability::NonConstructable,
    }
}

/// Go: `unknownRuntimeValueDomain`.
#[must_use]
pub const fn unknown_runtime_value_domain() -> RuntimeValueDomain {
    RuntimeValueDomain::new(true, true, true, true)
}

/// The constraint Go reads for an instantiable type: a type parameter's
/// declared constraint, any other instantiable type's base constraint.
fn instantiable_constraint(checker: &mut Checker, value: P<Type>) -> Option<P<Type>> {
    if value.flags().intersects(TypeFlags::TypeParameter) {
        checker.get_constraint_of_type_parameter_exported(value)
    } else {
        checker.get_base_constraint_of_type(value)
    }
}

/// Go: `runtimeValueDomainOfType`.
pub fn runtime_value_domain_of_type(checker: &mut Checker, value: P<Type>) -> RuntimeValueDomain {
    runtime_value_domain_seen(checker, value, &mut HashSet::new())
}

fn runtime_value_domain_seen(
    checker: &mut Checker,
    value: P<Type>,
    seen: &mut HashSet<usize>,
) -> RuntimeValueDomain {
    let flags = value.flags();
    if flags.intersects(TypeFlags::Any | TypeFlags::Unknown | TypeFlags::IncludesError) {
        return unknown_runtime_value_domain();
    }
    if flags.intersects(TypeFlags::Never) {
        return RuntimeValueDomain::default();
    }
    if !seen.insert(value.addr()) {
        return unknown_runtime_value_domain();
    }
    let domain = runtime_value_domain_walk(checker, value, flags, seen);
    seen.remove(&value.addr());
    domain
}

fn runtime_value_domain_walk(
    checker: &mut Checker,
    value: P<Type>,
    flags: TypeFlags,
    seen: &mut HashSet<usize>,
) -> RuntimeValueDomain {
    // A nil constraint is the conservative signal for an unconstrained or
    // circular generic.
    if flags.intersects(TypeFlags::Instantiable) {
        let Some(constraint) = instantiable_constraint(checker, value) else {
            return unknown_runtime_value_domain();
        };
        if constraint != value {
            return runtime_value_domain_seen(checker, constraint, seen);
        }
    }
    if flags.intersects(TypeFlags::Union) {
        let (mut callable, mut undefined, mut other, mut unknown) = (false, false, false, false);
        for constituent in value.types() {
            let part = runtime_value_domain_seen(checker, *constituent, seen);
            callable |= part.may_be_callable();
            undefined |= part.may_be_undefined();
            other |= part.may_be_other();
            unknown |= part.unknown();
        }
        return RuntimeValueDomain::new(callable, undefined, other, unknown);
    }
    if flags.intersects(TypeFlags::Undefined) {
        return RuntimeValueDomain::new(false, true, false, false);
    }
    // A `void` result is undefined at runtime, trusted exactly as every other
    // declared return type is (see the Go comment on return-type bivariance).
    if flags.intersects(TypeFlags::Void) {
        return RuntimeValueDomain::new(false, true, false, false);
    }
    if !checker
        .get_signatures_of_type_exported(value, SignatureKind::Call)
        .is_empty()
    {
        return RuntimeValueDomain::new(true, false, false, false);
    }
    // Matters for intersections such as `T & undefined`: when the remaining
    // source type is assignable to undefined, every inhabitant is undefined.
    let undefined = checker.get_undefined_type();
    if checker.is_type_assignable_to(value, undefined) {
        return RuntimeValueDomain::new(false, true, false, false);
    }
    RuntimeValueDomain::new(false, false, true, false)
}

/// Go: `unknownPrimitiveValueDomain`.
#[must_use]
pub const fn unknown_primitive_value_domain() -> PrimitiveValueDomain {
    PrimitiveValueDomain::new(
        false, false, false, false, false, false, false, false, false, true,
    )
}

/// One primitive category: Go's `NewPrimitiveValueDomain` with a single flag.
const fn only(index: usize) -> PrimitiveValueDomain {
    let mut flags = [false; 8];
    flags[index] = true;
    PrimitiveValueDomain::new(
        flags[0], flags[1], flags[2], flags[3], flags[4], flags[5], flags[6], flags[7], false,
        false,
    )
}

const STRING: usize = 0;
const NUMBER: usize = 1;
const BOOLEAN: usize = 2;
const BIG_INT: usize = 3;
const SYMBOL: usize = 4;
const NULL: usize = 5;
const UNDEFINED: usize = 6;
const OBJECT: usize = 7;

/// Go: `PrimitiveValueDomain.Union`.
#[must_use]
pub fn union_primitive(
    left: PrimitiveValueDomain,
    right: PrimitiveValueDomain,
) -> PrimitiveValueDomain {
    if left.unknown() || right.unknown() {
        return unknown_primitive_value_domain();
    }
    let may_be_number = left.may_be_number() || right.may_be_number();
    let finite = may_be_number
        && (!left.may_be_number() || left.numbers_are_finite())
        && (!right.may_be_number() || right.numbers_are_finite());
    let kinds = [
        left.may_be_string() || right.may_be_string(),
        may_be_number,
        left.may_be_boolean() || right.may_be_boolean(),
        left.may_be_big_int() || right.may_be_big_int(),
        left.may_be_symbol() || right.may_be_symbol(),
        left.may_be_null() || right.may_be_null(),
        left.may_be_undefined() || right.may_be_undefined(),
        left.may_be_object() || right.may_be_object(),
    ];
    if !kinds.iter().any(|kind| *kind) {
        // Go: no bits set stays absent unless a side was a demanded `never`.
        return if left.is_present() || right.is_present() {
            PrimitiveValueDomain::empty()
        } else {
            PrimitiveValueDomain::default()
        };
    }
    PrimitiveValueDomain::new(
        kinds[STRING],
        kinds[NUMBER],
        kinds[BOOLEAN],
        kinds[BIG_INT],
        kinds[SYMBOL],
        kinds[NULL],
        kinds[UNDEFINED],
        kinds[OBJECT],
        finite,
        false,
    )
}

/// Go: `checker.NumberLiteralIsFinite`: false for every non-literal number.
fn number_literal_is_finite(value: P<Type>) -> bool {
    if !value.flags().intersects(TypeFlags::NumberLiteral) {
        return false;
    }
    matches!(
        value.as_literal_type().value(),
        Some(LiteralValue::Number(number)) if number.0.is_finite()
    )
}

/// Go: `primitiveValueDomainOfType`: classifies the same checker type as
/// [`runtime_value_domain_of_type`], but preserves JavaScript's primitive
/// categories. Constraints and unions are closed recursively; all remaining
/// inhabited, compiler-known types are objects/functions.
pub fn primitive_value_domain_of_type(
    checker: &mut Checker,
    value: P<Type>,
) -> PrimitiveValueDomain {
    primitive_value_domain_seen(checker, value, &mut HashSet::new())
}

fn primitive_value_domain_seen(
    checker: &mut Checker,
    value: P<Type>,
    seen: &mut HashSet<usize>,
) -> PrimitiveValueDomain {
    let flags = value.flags();
    if flags.intersects(TypeFlags::Any | TypeFlags::Unknown) {
        return unknown_primitive_value_domain();
    }
    if flags.intersects(TypeFlags::Never) {
        return PrimitiveValueDomain::empty();
    }
    if !seen.insert(value.addr()) {
        return unknown_primitive_value_domain();
    }
    let domain = primitive_value_domain_walk(checker, value, flags, seen);
    seen.remove(&value.addr());
    domain
}

fn primitive_value_domain_walk(
    checker: &mut Checker,
    value: P<Type>,
    flags: TypeFlags,
    seen: &mut HashSet<usize>,
) -> PrimitiveValueDomain {
    if flags.intersects(TypeFlags::Instantiable) {
        let Some(constraint) = instantiable_constraint(checker, value) else {
            return unknown_primitive_value_domain();
        };
        if constraint != value {
            return primitive_value_domain_seen(checker, constraint, seen);
        }
    }
    if flags.intersects(TypeFlags::Union) {
        let mut domain = PrimitiveValueDomain::default();
        for constituent in value.types() {
            let part = primitive_value_domain_seen(checker, *constituent, seen);
            domain = union_primitive(domain, part);
        }
        return domain;
    }
    if flags.intersects(TypeFlags::StringLike) {
        return only(STRING);
    }
    if flags.intersects(TypeFlags::NumberLike) {
        return PrimitiveValueDomain::new(
            false,
            true,
            false,
            false,
            false,
            false,
            false,
            false,
            number_literal_is_finite(value),
            false,
        );
    }
    if flags.intersects(TypeFlags::BooleanLike) {
        return only(BOOLEAN);
    }
    if flags.intersects(TypeFlags::BigIntLike) {
        return only(BIG_INT);
    }
    if flags.intersects(TypeFlags::ESSymbolLike) {
        return only(SYMBOL);
    }
    if flags.intersects(TypeFlags::Null) {
        return only(NULL);
    }
    if flags.intersects(TypeFlags::Undefined | TypeFlags::Void) {
        return only(UNDEFINED);
    }
    if flags.intersects(TypeFlags::IncludesError) {
        return unknown_primitive_value_domain();
    }
    let string = checker.get_string_type();
    if checker.is_type_assignable_to(value, string) {
        return only(STRING);
    }
    let number = checker.get_number_type();
    if checker.is_type_assignable_to(value, number) {
        return only(NUMBER);
    }
    let boolean = checker.get_boolean_type();
    if checker.is_type_assignable_to(value, boolean) {
        return only(BOOLEAN);
    }
    let big_int = checker.get_big_int_type();
    if checker.is_type_assignable_to(value, big_int) {
        return only(BIG_INT);
    }
    let symbol = checker.get_es_symbol_type();
    if checker.is_type_assignable_to(value, symbol) {
        return only(SYMBOL);
    }
    let null = checker.get_null_type();
    if checker.is_type_assignable_to(value, null) {
        return only(NULL);
    }
    let undefined = checker.get_undefined_type();
    if checker.is_type_assignable_to(value, undefined) {
        return only(UNDEFINED);
    }
    only(OBJECT)
}

/// Go: `checker.PrimitiveStringLiteral` and siblings: a plain (non-enum)
/// literal's value.
fn primitive_literal(value: P<Type>) -> Option<PrimitiveLiteralCandidate> {
    let flags = value.flags();
    let plain =
        |literal: TypeFlags| flags.intersects(literal) && !flags.intersects(TypeFlags::EnumLiteral);
    if plain(TypeFlags::StringLiteral) {
        if let Some(LiteralValue::String(text)) = value.as_literal_type().value() {
            return Some(PrimitiveLiteralCandidate {
                kind: PrimitiveLiteralKind::String,
                string: text.into(),
                number: 0.0,
                boolean: false,
            });
        }
    } else if plain(TypeFlags::NumberLiteral) {
        if let Some(LiteralValue::Number(number)) = value.as_literal_type().value()
            && number.0.is_finite()
        {
            return Some(PrimitiveLiteralCandidate {
                kind: PrimitiveLiteralKind::Number,
                string: "".into(),
                number: number.0,
                boolean: false,
            });
        }
    } else if flags.intersects(TypeFlags::BooleanLiteral)
        && let Some(LiteralValue::Boolean(boolean)) = value.as_literal_type().value()
    {
        return Some(PrimitiveLiteralCandidate {
            kind: PrimitiveLiteralKind::Boolean,
            string: "".into(),
            number: 0.0,
            boolean,
        });
    }
    None
}

/// Go: `primitiveLiteralCandidatesOfType`: exact, compiler-proven inhabitants
/// of a demanded type, bounded, sorted by kind then value and deduplicated.
pub fn primitive_literal_candidates_of_type(
    checker: &mut Checker,
    value: P<Type>,
) -> Vec<PrimitiveLiteralCandidate> {
    fn collect(
        checker: &mut Checker,
        current: P<Type>,
        seen: &mut HashSet<usize>,
        candidates: &mut Vec<PrimitiveLiteralCandidate>,
    ) {
        if candidates.len() >= MAX_PRIMITIVE_LITERAL_CANDIDATES {
            return;
        }
        let flags = current.flags();
        if flags.intersects(OPEN) || !seen.insert(current.addr()) {
            return;
        }
        if flags.intersects(TypeFlags::Instantiable) {
            if let Some(constraint) = instantiable_constraint(checker, current)
                && constraint != current
            {
                collect(checker, constraint, seen, candidates);
            }
        } else if flags.intersects(TypeFlags::Union) {
            for constituent in current.types() {
                collect(checker, *constituent, seen, candidates);
            }
        } else if let Some(candidate) = primitive_literal(current) {
            candidates.push(candidate);
        }
        seen.remove(&current.addr());
    }

    let mut candidates = Vec::with_capacity(4);
    collect(checker, value, &mut HashSet::new(), &mut candidates);
    let kind_name = |kind: PrimitiveLiteralKind| match kind {
        PrimitiveLiteralKind::String => "string",
        PrimitiveLiteralKind::Number => "number",
        PrimitiveLiteralKind::Boolean => "boolean",
    };
    candidates.sort_by(|left, right| {
        kind_name(left.kind)
            .cmp(kind_name(right.kind))
            .then_with(|| match left.kind {
                PrimitiveLiteralKind::String => left.string.cmp(&right.string),
                PrimitiveLiteralKind::Number => left
                    .number
                    .partial_cmp(&right.number)
                    .unwrap_or(std::cmp::Ordering::Equal),
                PrimitiveLiteralKind::Boolean => left.boolean.cmp(&right.boolean),
            })
    });
    candidates.dedup();
    candidates
}

/// Go: `openArrayShapeFlags`: the flags that leave an array/tuple claim
/// unprovable in either direction.
const OPEN_ARRAY_SHAPE: TypeFlags = OPEN.union(TypeFlags::Instantiable);

/// Go: `arrayShapeOfType`.
pub fn array_shape_of_type(checker: &mut Checker, value: P<Type>) -> ArrayShape {
    if value.flags().intersects(OPEN_ARRAY_SHAPE) {
        return ArrayShape::Unknown;
    }
    let constituents = value.distributed();
    if constituents.is_empty() {
        return ArrayShape::Unknown;
    }
    let (mut array, mut not_array) = (false, false);
    for constituent in constituents {
        if constituent.flags().intersects(OPEN_ARRAY_SHAPE) {
            return ArrayShape::Unknown;
        }
        if checker.is_array_or_tuple_type(constituent) {
            array = true;
        } else {
            not_array = true;
        }
    }
    match (array, not_array) {
        (true, true) => ArrayShape::Mixed,
        (true, false) => ArrayShape::Array,
        _ => ArrayShape::NotArray,
    }
}

/// Go: `arrayShapeAtLocked`: the type of exactly the demanded expression.
pub fn array_shape_at(checker: &mut Checker, node: P<Node>) -> ArrayShape {
    let value = checker.get_type_at_location(node);
    array_shape_of_type(checker, value)
}

/// One tuple's shape before it is packed (Go: `typefacts.TupleShape`).
#[derive(Clone, Copy)]
struct Tuple {
    fixed_length: u32,
    has_rest: bool,
    element_zero: Callability,
    element_zero_minimum_parameters: u32,
    exact_length: Option<u32>,
}

/// Go: `tupleShapeAtLocked`: the tuple at exactly the demanded expression, or
/// `None` when that type does not resolve to one. A union answers when every
/// constituent that can hold a value is a tuple; the result is their
/// conservative meet. Nullish constituents carry no structure and are skipped.
pub fn tuple_shape_at(checker: &mut Checker, node: P<Node>) -> Option<TupleShape> {
    let value = checker.get_type_at_location(node);
    if value.flags().intersects(OPEN_ARRAY_SHAPE) {
        return None;
    }
    let mut merged: Option<Tuple> = None;
    for constituent in value.distributed() {
        if constituent.flags().intersects(OPEN_ARRAY_SHAPE) {
            return None;
        }
        if constituent
            .flags()
            .intersects(TypeFlags::Undefined | TypeFlags::Null | TypeFlags::Void)
        {
            continue;
        }
        let shape = tuple_shape_of_type(checker, constituent)?;
        merged = Some(match merged {
            None => shape,
            Some(current) => meet_tuple_shapes(current, shape),
        });
    }
    let merged = merged?;
    TupleShape::for_producer(
        merged.fixed_length,
        merged.has_rest,
        merged.element_zero,
        merged.element_zero_minimum_parameters,
        merged.exact_length,
    )
    .ok()
}

/// Go: `tupleShapeOfType`: one non-union type, or `None` when it is not a tuple.
fn tuple_shape_of_type(checker: &mut Checker, value: P<Type>) -> Option<Tuple> {
    if !value.is_tuple_type() {
        return None;
    }
    let target = value.target_tuple_type();
    let fixed_length = u32::try_from(target.fixed_length()).ok()?;
    let mut shape = Tuple {
        fixed_length,
        has_rest: false,
        element_zero: Callability::Unknown,
        element_zero_minimum_parameters: 0,
        exact_length: Some(fixed_length),
    };
    for flags in target.element_flags() {
        if flags.intersects(ElementFlags::NonRequired) {
            shape.exact_length = None;
        }
        if flags.intersects(ElementFlags::Variable) {
            shape.has_rest = true;
        }
    }
    let elements = checker.get_type_arguments(value);
    if shape.fixed_length > 0
        && let Some(first) = elements.first()
    {
        shape.element_zero = callability_of_type(checker, *first);
        shape.element_zero_minimum_parameters = minimum_parameter_count(checker, *first);
    }
    Some(shape)
}

/// Go: `meetTupleShapes`: keeps only what both shapes guarantee.
fn meet_tuple_shapes(left: Tuple, right: Tuple) -> Tuple {
    let callable_in_either_sense =
        |value: Callability| matches!(value, Callability::Callable | Callability::UntypedCallable);
    let element_zero = if left.element_zero == right.element_zero {
        left.element_zero
    } else if left.element_zero == Callability::Unknown
        || right.element_zero == Callability::Unknown
    {
        Callability::Unknown
    } else if callable_in_either_sense(left.element_zero)
        && callable_in_either_sense(right.element_zero)
    {
        // Mirrors `callability_of_type`'s own union rung: the meet can promise
        // only the weaker of the two.
        Callability::UntypedCallable
    } else {
        Callability::Mixed
    };
    Tuple {
        fixed_length: left.fixed_length.min(right.fixed_length),
        has_rest: left.has_rest && right.has_rest,
        element_zero,
        element_zero_minimum_parameters: left
            .element_zero_minimum_parameters
            .max(right.element_zero_minimum_parameters),
        exact_length: match (left.exact_length, right.exact_length) {
            (Some(a), Some(b)) if a == b => Some(a),
            _ => None,
        },
    }
}

/// Go: `minimumParameterCount`: the fewest arguments any call signature of
/// `value` requires; zero when it has none.
fn minimum_parameter_count(checker: &mut Checker, value: P<Type>) -> u32 {
    let mut minimum: Option<usize> = None;
    for signature in checker.get_signatures_of_type_exported(value, SignatureKind::Call) {
        let count = signature.min_argument_count();
        minimum = Some(minimum.map_or(count, |current| current.min(count)));
    }
    u32::try_from(minimum.unwrap_or(0)).unwrap_or(u32::MAX)
}

/// ADR 0436: the property names the type at exactly the demanded expression
/// declares, or `None` unless they are all known. Known means every
/// constituent that can hold a value is an object type (a union adds the
/// names of each member, an intersection of each member) with no index
/// signature of any key type, whose own property list the compiler resolves
/// completely. `any`, `unknown`, `never`, a primitive, a type parameter or any
/// other instantiable type, a generic mapped type, a mapped type with an `as`
/// clause, a reverse-mapped or evolving type, and a property the compiler names
/// internally (a symbol key) each make the answer `None`; so does a type whose
/// only constituents are nullish, and `{}` or any object type with no property
/// and no signature, which admits any non-nullish value. Names are sorted and deduplicated.
///
/// The answer is the declared type's, not the value's: a value typed
/// `{ class?: string }` may hold more keys at run time (structural subtyping).
/// ADR 0436 states that premise.
pub fn declared_keys_at(checker: &mut Checker, node: P<Node>) -> Option<Vec<Arc<str>>> {
    let value = checker.get_type_at_location(node);
    let mut names: BTreeSet<String> = BTreeSet::new();
    let mut objects = 0usize;
    collect_declared_keys(checker, value, &mut names, &mut objects)?;
    if objects == 0 {
        return None;
    }
    Some(names.into_iter().map(Arc::from).collect())
}

/// ADR 0460: whether `node` is a property access whose property symbol the
/// checker marks optional and not readonly, the condition under which `delete
/// node` raises neither TS2704 (readonly operand) nor TS2790 (non-optional
/// operand) under any compiler option. The symbol is the one `checkDeleteExpression`
/// reads: the access's resolved symbol. An element access, a private name, an
/// unresolved access (`any`, a missing property) and an index signature (its
/// symbol is a signature, not a property) answer `false`, which is no claim.
pub fn writable_optional_property_at(checker: &mut Checker, node: P<Node>) -> bool {
    if !is_property_access_expression(node) {
        return false;
    }
    let Some(name) = node.name() else {
        return false;
    };
    if is_private_identifier(name) {
        return false;
    }
    let Some(symbol) = checker.get_symbol_at_location_exported(name) else {
        return false;
    };
    symbol
        .flags()
        .contains(SymbolFlags::Property | SymbolFlags::Optional)
        && !checker.is_readonly_symbol_exported(symbol)
}

/// Whether `value` is an instantiation of a top-level type alias whose type
/// arguments are all [`closed_type`].
fn closed_top_level_alias(checker: &mut Checker, value: P<Type>, depth: usize) -> bool {
    let Some(alias) = value.alias() else {
        return false;
    };
    let Some(symbol) = alias.symbol() else {
        return false;
    };
    symbol.declarations().iter().all(|declaration| {
        declaration
            .parent()
            .is_some_and(|parent| is_source_file(parent) || is_module_block(parent))
    }) && alias
        .type_arguments()
        .iter()
        .all(|argument| closed_type(checker, *argument, depth + 1))
}

/// Whether `value` mentions no type variable on a path that can fix a key set:
/// an instantiable type, a reference argument or a mapped type's alias argument.
fn closed_type(checker: &mut Checker, value: P<Type>, depth: usize) -> bool {
    if depth > 8 || value.flags().intersects(TypeFlags::Instantiable) {
        return false;
    }
    if value
        .flags()
        .intersects(TypeFlags::Union | TypeFlags::Intersection)
    {
        return value
            .types()
            .iter()
            .all(|member| closed_type(checker, *member, depth + 1));
    }
    if !value.flags().intersects(TypeFlags::Object) {
        return true;
    }
    if value.object_flags().intersects(ObjectFlags::Mapped) {
        return closed_top_level_alias(checker, value, depth);
    }
    if value.object_flags().intersects(ObjectFlags::Reference) {
        return checker
            .get_type_arguments_exported(value)
            .iter()
            .all(|argument| closed_type(checker, *argument, depth + 1));
    }
    true
}

/// One constituent of [`declared_keys_at`]'s walk. `None` is "not exact".
fn collect_declared_keys(
    checker: &mut Checker,
    value: P<Type>,
    names: &mut BTreeSet<String>,
    objects: &mut usize,
) -> Option<()> {
    let flags = value.flags();
    if flags.intersects(OPEN | TypeFlags::Instantiable) {
        return None;
    }
    if flags.intersects(TypeFlags::Union | TypeFlags::Intersection) {
        for member in value.types() {
            // A nullish member spreads as nothing; it adds no key.
            if member
                .flags()
                .intersects(TypeFlags::Undefined | TypeFlags::Null | TypeFlags::Void)
            {
                continue;
            }
            collect_declared_keys(checker, *member, names, objects)?;
        }
        return Some(());
    }
    if flags != TypeFlags::Object
        || value.object_flags().intersects(
            ObjectFlags::ReverseMapped
                | ObjectFlags::EvolvingArray
                | ObjectFlags::InstantiationExpressionType,
        )
    {
        return None;
    }
    let properties = checker.get_properties_of_type_exported(value);
    if !checker.get_index_infos_of_type_exported(value).is_empty() {
        return None;
    }
    // `{}` (and an empty interface or mapped type) admits any non-nullish
    // value: with no property and no signature it declares nothing, so its
    // empty key set would prove every spread of it handler-free.
    if properties.is_empty()
        && checker
            .get_signatures_of_type_exported(value, SignatureKind::Call)
            .is_empty()
        && checker
            .get_signatures_of_type_exported(value, SignatureKind::Construct)
            .is_empty()
    {
        return None;
    }
    if value.object_flags().intersects(ObjectFlags::Mapped) {
        // Resolving the members above computed the constraint. Only a
        // constraint that is a union of literal keys fixes the key set.
        let mapped = value.as_mapped_type();
        // An `as` clause renames each key through a type the checker drops
        // silently when it stays deferred (a conditional on an unresolved type
        // parameter), so the key set is trusted only for a top-level alias
        // instantiated with arguments free of type variables: `@solidjs/signals`'
        // `Omit<T, K>` (`[P in keyof T as Exclude<P, K[number]>]: T[P]`). lib's
        // `Omit<T, K>` is `Pick<T, Exclude<keyof T, K>>`, a mapped type with no
        // `as` clause, judged by its constraint alone.
        if mapped
            .declaration
            .get()?
            .as_mapped_type_node()
            .name_type()
            .is_some()
            && !closed_top_level_alias(checker, value, 0)
        {
            return None;
        }
        let constraint = mapped.constraint_type()?;
        if !constraint
            .distributed()
            .iter()
            .all(|key| key.flags().intersects(TypeFlags::StringLiteral))
        {
            return None;
        }
    }
    for property in properties {
        let name = property.name();
        if name.starts_with(tsrs_ast::INTERNAL_SYMBOL_NAME_PREFIX) {
            return None;
        }
        names.insert(name.to_owned());
    }
    *objects += 1;
    Some(())
}

/// Go: `libraryTypesAtLocked`: the standard-library types the type at exactly
/// the demanded expression is built from, at its top level (the type, its union
/// and intersection constituents, and one array-element unwrap). Names are
/// sorted and deduplicated; only declarations in default-library files count.
pub fn library_types_at(
    checker: &mut Checker,
    program: &Program,
    node: P<Node>,
) -> Option<Vec<Arc<str>>> {
    let value = checker.get_type_at_location(node);
    let mut names: BTreeSet<String> = BTreeSet::new();
    for constituent in value.distributed() {
        collect_library_types(checker, program, constituent, &mut names, true);
    }
    if names.is_empty() {
        return None;
    }
    Some(names.into_iter().map(Arc::from).collect())
}

/// Go: `collectLibraryTypesLocked`.
fn collect_library_types(
    checker: &mut Checker,
    program: &Program,
    value: P<Type>,
    names: &mut BTreeSet<String>,
    descend: bool,
) {
    if value.flags().intersects(TypeFlags::Intersection) {
        for member in value.types() {
            collect_library_types(checker, program, *member, names, descend);
        }
        return;
    }
    if let Some(name) = library_type_name(program, value) {
        names.insert(name);
    }
    if !descend {
        return;
    }
    if checker.is_array_or_tuple_type(value) {
        for element in checker.get_type_arguments(value) {
            for constituent in element.distributed() {
                collect_library_types(checker, program, constituent, names, false);
            }
        }
    }
}

/// Go: `libraryTypeNameLocked`: the declared name when a declaration lives in a
/// default-library file. Anonymous object and function types answer `None`.
fn library_type_name(program: &Program, value: P<Type>) -> Option<String> {
    let symbol = value.symbol()?;
    for declaration in symbol.declarations() {
        let Some(file) = get_source_file_of_node(*declaration) else {
            continue;
        };
        if program.is_source_file_default_library(file.path()) {
            return Some(library_symbol_name(symbol.name()));
        }
    }
    None
}

/// The name Go puts on the wire for a library symbol. An internal name (an
/// anonymous type literal, `__type`) starts with Go's `0xFE` marker, which Go
/// writes as a raw byte and every client decoder reads back as U+FFFD; tsrs
/// spells the marker U+007F. A Rust `String` cannot hold the raw byte, so the
/// name is spelled as the client decodes it, which keeps the two producers
/// indistinguishable on the wire. (The fact is junk either way, no consumer
/// asks for such a name. When the Go producer is deleted the right fix is to
/// skip internal names.) The checker's counters are left out
/// (`\u{7f}@iterator@<symbol id>`, ADR 0367): they follow the order the checker
/// numbered symbols in, which is history.
pub(crate) fn library_symbol_name(name: &str) -> String {
    let name = crate::durable::display_name(name);
    match name.strip_prefix('\u{7f}') {
        Some(rest) => format!("\u{fffd}{rest}"),
        None => name.into_owned(),
    }
}
