//! Resolved calls (Go: `tsgo/resolved_calls.go`, the parameter object shape of
//! `tsgo/object_construction_shapes.go`, and `callResultDomainAtLocked` of
//! `tsgo/semantic_runs.go`).
//!
//! A demanded call resolves to the compiler's selected signature: its target
//! symbol, return type text, validity, declaration, default-library binding,
//! union target set and per-argument parameter facts, plus the assertion and
//! never-returning bits (ADR 0279).
//!
//! # Validity
//!
//! Ordinary calls are proven from the concrete signature, arity and
//! assignability. Nullable, union, context-sensitive, explicit-generic,
//! construct and declaration-less calls take the diagnostic path: the file's
//! semantic diagnostics, restricted to the call-resolution codes, decide
//! whether a call-like node failed (`unsafe_boundary::semantic_diagnostics`,
//! one checker, so there is one report per file).
//!
//! # Ownership
//!
//! Go keeps `resolvedDeclarations`, `resolvedParameters`, `callDiagnostics`,
//! `libraryPropertyWrites` and `currentSourceFiles` on the project for the
//! life of a generation. Here they live in [`CallCaches`], created per
//! `semantic_runs::run` call: a tsrs address is valid only for the generation
//! that issued it (`unsafe_boundary` invariant 1), and an address-keyed map
//! that outlived a call could answer for a different, later object. The
//! answers are equal; only the memo's lifetime is shorter (the diagnostics and
//! the library write census are recomputed once per backend call that needs
//! them, not once per generation).

use crate::backend::BackendError;
use crate::context::Context;
use crate::gocompat::clean;
use crate::identity::Env;
use crate::nodes::{is_call_like_expression, node_at};
use crate::project::{declarations_for_symbol, wire_symbol_name};
use crate::references::ReferenceIndex;
use crate::semantic_runs::Evidence;
use crate::type_facts::{
    DescriptorMemo, array_shape_of_type, callability_of_type, declaration_module,
    runtime_value_domain_of_type, unknown_runtime_value_domain,
};
use crate::unsafe_boundary::semantic_diagnostics;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tsrs_ast::{
    Kind, Node, SourceFile, Symbol, SymbolFlags, get_assignment_target, get_source_file_of_node,
    is_arrow_function, is_assertion_expression, is_delete_expression, is_element_access_expression,
    is_function_declaration, is_function_expression, is_identifier, is_new_expression,
    is_no_substitution_template_literal, is_non_null_expression, is_optional_chain,
    is_parenthesized_expression, is_property_access_expression, is_satisfies_expression,
    is_spread_element, is_string_literal,
};
use tsrs_checker::types::{
    ElementFlags, Signature, SignatureFlags, SignatureKind, Type, TypeExt, TypeFlags,
    TypePredicateKind,
};
use tsrs_core::P;
use tsrs_scanner::skip_trivia;
use typefacts::{
    ArgumentMapping, ArgumentMappingReason, ArgumentMappingStatus, ArrayShape, CallKind,
    CallTargetSet, ConstructionWitness, DeclarationOwner, EntityFact, Location,
    ObjectConstructionProperty, ObjectConstructionShape, ParameterFact, ResolvedCall,
    ResolvedCallValidity, ResolvedDeclaration, RuntimeValueDomain,
};

/// Go: `resolvedCallDemand`: the private seam between semantic location lookup
/// and resolved-call construction.
pub struct CallDemand {
    pub entity_index: usize,
    pub node: Option<P<Node>>,
    pub object_shape: bool,
}

/// Go: the project's resolved-call memos (see the module docs for why they are
/// per backend call here). Every key is a tsrs address; a missing signature or
/// symbol is `0`.
#[derive(Default)]
pub struct CallCaches {
    /// Go: `resolvedDeclarations`, keyed by `(signature, fallback, node)`.
    declarations: HashMap<(usize, usize, usize), Arc<ResolvedDeclaration>>,
    /// Go: `resolvedParameters`, keyed by `(signature, argument, objectShape)`.
    parameters: HashMap<(usize, usize, bool), ParameterFact>,
    /// Go: `callDiagnostics`: per source file, the call-like nodes a
    /// call-resolution diagnostic landed in.
    diagnostics: HashMap<usize, HashSet<usize>>,
    /// Go: `libraryPropertyWrites`.
    library_property_writes: Option<HashSet<String>>,
    /// Go: `currentSourceFiles`.
    current_files: Option<HashSet<usize>>,
}

fn addr<T>(value: Option<P<T>>) -> usize {
    value.map_or(0, |value| value.addr())
}

fn check(ctx: &Context) -> Result<(), BackendError> {
    match ctx.err() {
        Some(reason) => Err(BackendError(reason.to_owned())),
        None => Ok(()),
    }
}

/// Go: `invocationPropertyName`.
fn invocation_property_name(name: &str) -> Option<&str> {
    (!name.is_empty() && !name.starts_with(tsrs_ast::InternalSymbolNamePrefix)).then_some(name)
}

/// Go: `transparentCallCallee`.
fn transparent_call_callee(mut callee: P<Node>) -> P<Node> {
    while is_parenthesized_expression(callee)
        || is_assertion_expression(callee)
        || is_non_null_expression(callee)
        || is_satisfies_expression(callee)
    {
        let Some(inner) = callee.expression() else {
            break;
        };
        callee = inner;
    }
    callee
}

/// Go: `node.Parameters()` on a node that may not be function-like.
fn parameters_of(node: P<Node>) -> &'static [P<Node>] {
    if node.function_like_data().is_some() {
        node.parameters()
    } else {
        &[]
    }
}

fn name_location(file: P<SourceFile>, name: P<Node>) -> Option<Location> {
    let (pos, end) = (name.pos().source_offset()?, name.end().source_offset()?);
    Some(Location {
        path: clean(file.file_name()).into(),
        start_byte: u64::from(skip_trivia(file.text(), pos)),
        end_byte: u64::from(end),
    })
}

/// Go: `isExactCallableImplementationKind`: declaration kinds that identify one
/// runtime callable implementation. Type-level signature declarations only
/// describe a structural shape, so they never become dispatch candidates.
fn is_exact_callable_implementation_kind(kind: &str) -> bool {
    matches!(
        kind,
        "FunctionDeclaration"
            | "MethodDeclaration"
            | "ArrowFunction"
            | "FunctionExpression"
            | "Constructor"
    )
}

fn has_flag(value: P<Type>, flags: TypeFlags) -> bool {
    value.flags().intersects(flags)
}

/// What one resolved-call run reads and writes.
pub struct CallRun<'a, 'e> {
    pub ctx: &'a Context,
    pub env: &'a mut Env<'e>,
    pub references: &'a mut ReferenceIndex,
    pub caches: &'a mut CallCaches,
    pub memo: &'a mut DescriptorMemo,
    pub file: P<SourceFile>,
    pub evidence: &'a mut Evidence,
}

impl CallRun<'_, '_> {
    /// Go: `isCurrentSourceFile`.
    fn is_current_source_file(&mut self, file: P<SourceFile>) -> bool {
        let program = self.env.program;
        self.caches
            .current_files
            .get_or_insert_with(|| {
                program
                    .source_files()
                    .iter()
                    .map(|file| file.addr())
                    .collect()
            })
            .contains(&file.addr())
    }

    /// Go: `currentSignatureDeclaration`.
    fn current_signature_declaration(
        &mut self,
        signature: P<Signature>,
        target: Option<P<Symbol>>,
    ) -> Option<P<Node>> {
        let declaration = signature.declaration()?;
        let file = get_source_file_of_node(declaration)?;
        if self.is_current_source_file(file) {
            return Some(declaration);
        }
        let target = self.env.canonical_symbol(target?);
        let mut ordinal = 0;
        if let Some(declaration_symbol) = declaration.symbol() {
            for candidate in declaration_symbol.declarations() {
                if candidate.kind() != declaration.kind() {
                    continue;
                }
                if *candidate == declaration {
                    break;
                }
                ordinal += 1;
            }
        }
        let mut matches: Vec<P<Node>> = Vec::with_capacity(ordinal + 1);
        fn visit(node: P<Node>, kind: Kind, matches: &mut Vec<P<Node>>) -> bool {
            if node.kind() == kind {
                matches.push(node);
                return false;
            }
            node.for_each_child(&mut |child| visit(child, kind, matches));
            false
        }
        for root in target.declarations() {
            if root.kind() == declaration.kind() {
                matches.push(*root);
                continue;
            }
            let kind = declaration.kind();
            root.for_each_child(&mut |child| visit(child, kind, &mut matches));
        }
        matches.get(ordinal).copied()
    }

    /// Go: `resolvedDeclaration`.
    fn resolved_declaration(
        &mut self,
        signature: Option<P<Signature>>,
        node: Option<P<Node>>,
        fallback: Option<P<Symbol>>,
    ) -> Option<Arc<ResolvedDeclaration>> {
        let key = (addr(signature), addr(fallback), addr(node));
        if let Some(cached) = self.caches.declarations.get(&key) {
            return Some(Arc::clone(cached));
        }
        let node = node?;
        let file = get_source_file_of_node(node)?;
        let name_node = node.name().unwrap_or(node);
        let mut symbol = node.symbol();
        if (is_arrow_function(node) || is_function_expression(node)) && fallback.is_some() {
            // Anonymous function expressions often carry an internal signature
            // symbol. The callable declaration exposed to consumers is the
            // compiler-resolved callee symbol that owns that expression.
            symbol = fallback;
        }
        if symbol.is_none()
            && let Some(name) = node.name()
        {
            symbol = self.env.checker.get_symbol_at_location_exported(name);
        }
        if symbol.is_none() {
            symbol = fallback;
        }
        let kind = node.kind_string();
        let mut result = ResolvedDeclaration {
            symbol: "".into(),
            name: "".into(),
            kind: kind.as_str().into(),
            location: name_location(file, name_node)?,
            owners: Arc::from(Vec::new()),
            qualified_name: "".into(),
            origin_module: "".into(),
            source_file: clean(file.file_name()).into(),
            standard_library: false,
        };
        let mut name = String::new();
        if let Some(found) = symbol {
            let found = self.env.canonical_symbol(found);
            symbol = Some(found);
            result.symbol = self.env.id_for(found);
            result.origin_module = declaration_module(found).into();
            if !found.name().starts_with(tsrs_ast::InternalSymbolNamePrefix) {
                name = found.name().to_owned();
            }
        }
        match kind.as_str() {
            "Constructor" => name = "constructor".to_owned(),
            "ArrowFunction" | "FunctionExpression" => {
                if name.is_empty() {
                    name = "call".to_owned();
                }
            }
            "CallSignature" | "FunctionType" => name = "call".to_owned(),
            "ConstructSignature" | "ConstructorType" => name = "construct".to_owned(),
            _ => {}
        }
        let mut owners: Vec<DeclarationOwner> = Vec::new();
        let mut owner = node.parent();
        while let Some(current) = owner
            && current.parent().is_some()
        {
            owner = current.parent();
            let mut owner_symbol = current.symbol();
            if owner_symbol.is_none()
                && let Some(owner_name) = current.name()
            {
                owner_symbol = self.env.checker.get_symbol_at_location_exported(owner_name);
            }
            let Some(owner_symbol) = owner_symbol else {
                continue;
            };
            let Some(owner_file) = get_source_file_of_node(current) else {
                continue;
            };
            let owner_symbol = self.env.canonical_symbol(owner_symbol);
            if Some(owner_symbol.addr()) == symbol.map(|symbol| symbol.addr()) {
                continue;
            }
            let owner_name = invocation_property_name(owner_symbol.name()).unwrap_or("");
            let owner_name_node = current.name().unwrap_or(current);
            let Some(location) = name_location(owner_file, owner_name_node) else {
                continue;
            };
            owners.push(DeclarationOwner {
                symbol: self.env.id_for(owner_symbol),
                name: owner_name.into(),
                kind: current.kind_string().as_str().into(),
                location,
            });
        }
        owners.reverse();
        let mut qualified: Vec<&str> = Vec::with_capacity(owners.len() + 1);
        for owner in &owners {
            if !owner.name.is_empty() && !owner.name.starts_with(tsrs_ast::InternalSymbolNamePrefix)
            {
                qualified.push(&owner.name);
            }
        }
        if !name.is_empty() {
            qualified.push(&name);
        }
        result.qualified_name = qualified.join(".").into();
        result.name = name.into();
        result.owners = owners.into();
        result.standard_library = self.env.program.is_source_file_default_library(file.path());
        let result = Arc::new(result);
        self.caches.declarations.insert(key, Arc::clone(&result));
        Some(result)
    }

    /// Go: `resolveCallRunLocked`: resolves every call demand in one Semantic
    /// demand run.
    pub fn resolve(
        &mut self,
        demands: &[CallDemand],
        entities: &mut [EntityFact],
    ) -> Result<(), BackendError> {
        for demand in demands {
            check(self.ctx)?;
            let call = self.resolve_one(demand)?;
            entities[demand.entity_index].resolved_call = Some(Arc::new(call));
        }
        Ok(())
    }

    #[allow(clippy::too_many_lines)]
    fn resolve_one(&mut self, demand: &CallDemand) -> Result<ResolvedCall, BackendError> {
        let mut call = ResolvedCall {
            target: "".into(),
            return_type_text: "".into(),
            assertion_signature: false,
            never_returns: false,
            validity: ResolvedCallValidity::Unresolved,
            kind: CallKind::Unknown,
            declaration: None,
            default_library_binding: None,
            targets: None,
            arguments: Arc::from(Vec::new()),
        };
        let Some(node) = demand.node else {
            return Ok(call);
        };
        call.kind = if is_new_expression(node) {
            CallKind::Construct
        } else {
            CallKind::Call
        };
        let argument_count = node.arguments().len();
        let callee = node.expression().expect("a call-like node has a callee");
        let mut target = self
            .env
            .checker
            .get_symbol_at_location_exported(transparent_call_callee(callee));
        let signature = self.env.checker.get_resolved_signature_exported(node);
        if let Some(found) = target {
            let found = self.env.canonical_symbol(found);
            target = Some(found);
            let id = self.env.id_for(found);
            self.evidence.symbol(&id);
            if let Some(current) = self.env.symbol_for(&id) {
                target = Some(current);
            }
            call.target = id;
        }

        let (validity, callee_type) = self.validity_and_callee_type(node, signature, target)?;
        call.validity = validity;
        let return_type = self
            .env
            .checker
            .get_return_type_of_signature_exported(signature);
        call.return_type_text = self
            .memo
            .descriptor(self.env.checker, return_type)
            .text
            .clone();
        call.never_returns = has_flag(return_type, TypeFlags::Never);
        if let Some(predicate) = self
            .env
            .checker
            .get_type_predicate_of_signature_exported(signature)
        {
            let kind = predicate.kind();
            call.assertion_signature = kind == TypePredicateKind::AssertsIdentifier
                || kind == TypePredicateKind::AssertsThis;
        }
        if call.validity != ResolvedCallValidity::Valid {
            call.arguments =
                unresolved_mappings(argument_count, ArgumentMappingReason::RecoverySignature);
            return Ok(call);
        }
        if let Some(callee_type) = callee_type
            && has_flag(callee_type, TypeFlags::Union)
            && !single_optional_call_constituent(node, callee_type)
        {
            call.targets = self.union_call_targets(node, callee_type);
            call.arguments =
                unresolved_mappings(argument_count, ArgumentMappingReason::CompositeSignature);
            return Ok(call);
        }

        let declaration = self.current_signature_declaration(signature, target);
        let resolved = self.resolved_declaration(Some(signature), declaration, target);
        self.evidence.declaration(resolved.as_deref());
        call.declaration = resolved.as_deref().cloned();
        if call.declaration.is_some() && call.kind == CallKind::Call {
            let binding = self.default_library_binding(callee, target);
            self.evidence.declaration(binding.as_deref());
            call.default_library_binding = binding;
        }
        call.arguments = self.argument_mappings(node, signature, declaration, demand.object_shape);
        Ok(call)
    }

    /// Go: `defaultLibraryBinding`. Overload selection may choose an ambient
    /// augmentation even though the exact direct global binding also has a
    /// default-library declaration. Keep these two facts separate: the selected
    /// overload still owns every parameter and return. Imports, local aliases,
    /// structural properties and runtime implementations do not gain library
    /// identity from their types or names.
    fn default_library_binding(
        &mut self,
        callee: P<Node>,
        target: Option<P<Symbol>>,
    ) -> Option<Arc<ResolvedDeclaration>> {
        let callee = transparent_call_callee(callee);
        let target = target?;
        if !is_identifier(callee) {
            return None;
        }
        let symbol = self.env.checker.get_symbol_at_location_exported(callee)?;
        if symbol.flags().intersects(SymbolFlags::Alias)
            || self.env.canonical_symbol(symbol).addr() != target.addr()
        {
            return None;
        }
        let program = self.env.program;
        let mut library: Option<Arc<ResolvedDeclaration>> = None;
        let mut runtime_implementation = false;
        for declaration in target.declarations() {
            let file = get_source_file_of_node(*declaration)?;
            let default_library = program.is_source_file_default_library(file.path());
            if !file.is_declaration_file() && !default_library {
                runtime_implementation = true;
            }
            if default_library && is_function_declaration(*declaration) {
                let candidate = self.resolved_declaration(None, Some(*declaration), Some(target));
                if let Some(candidate) = candidate
                    && candidate.owners.is_empty()
                {
                    library = Some(candidate);
                }
            }
        }
        let library = library?;
        // This negative write census crosses source files. Re-evaluate the run
        // in every generation, including when a write or implementation is
        // removed; a previous absence of references is never an invalidation
        // dependency.
        self.evidence.durable = false;
        if runtime_implementation {
            return None;
        }
        let writes = self.caches.library_property_writes.get_or_insert_with(|| {
            let mut writes = HashSet::new();
            for file in program.source_files() {
                if !file.is_declaration_file() {
                    file.as_node()
                        .for_each_child(&mut |child| collect_property_writes(child, &mut writes));
                }
            }
            writes
        });
        if writes.contains(&*library.name) {
            return None;
        }
        self.references.ensure(self.env);
        let id = self.env.id_for(target);
        for reference in self.references.query(self.env, &id) {
            let source = program.get_source_file(&reference.path)?;
            let position = u32::try_from(reference.start_byte).ok()?;
            let mut node = node_at(source, position);
            if let Some(parent) = node.parent()
                && is_property_access_expression(parent)
                && parent.name() == Some(node)
            {
                node = parent;
            }
            if get_assignment_target(node).is_some() {
                return None;
            }
        }
        Some(library)
    }

    /// Go: `unionCallTargetsLocked`: the exact candidate declarations of a
    /// valid call whose callee type is a union. The set is emitted only when
    /// the compiler proves it exhaustive: every union constituent is one
    /// closed concrete callable and every one of its call (or construct)
    /// signatures names one exact implementation declaration with a canonical
    /// symbol. Any open, recovery, type-level, or declaration-less constituent
    /// leaves the composite call without a target set; an incomplete candidate
    /// set is never emitted, because it would read as the complete runtime
    /// dispatch set.
    fn union_call_targets(&mut self, node: P<Node>, callee_type: P<Type>) -> Option<CallTargetSet> {
        let signature_kind = if is_new_expression(node) {
            SignatureKind::Construct
        } else {
            SignatureKind::Call
        };
        // Instantiable covers unconstrained and constrained generics: a
        // constraint bounds behavior structurally but does not enumerate exact
        // implementations, so any generic constituent voids the proof.
        let open = TypeFlags::Any
            | TypeFlags::Unknown
            | TypeFlags::Never
            | TypeFlags::Undefined
            | TypeFlags::Null
            | TypeFlags::Instantiable
            | TypeFlags::Union
            | TypeFlags::Intersection
            | TypeFlags::IncludesError;
        let mut candidates: Vec<ResolvedDeclaration> = Vec::new();
        for constituent in callee_type.types() {
            if has_flag(*constituent, open) {
                return None;
            }
            let signatures = self
                .env
                .checker
                .get_signatures_of_type_exported(*constituent, signature_kind);
            if signatures.is_empty() {
                return None;
            }
            let target = constituent.symbol();
            for &signature in signatures {
                if signature
                    .flags()
                    .intersects(SignatureFlags::IsSignatureCandidateForOverloadFailure)
                {
                    return None;
                }
                let declaration = self.current_signature_declaration(signature, target)?;
                let resolved =
                    self.resolved_declaration(Some(signature), Some(declaration), target)?;
                if resolved.symbol.is_empty()
                    || !is_exact_callable_implementation_kind(&resolved.kind)
                {
                    return None;
                }
                candidates.push((*resolved).clone());
            }
        }
        if candidates.is_empty() {
            return None;
        }
        candidates.sort_by(|left, right| {
            left.location
                .path
                .cmp(&right.location.path)
                .then(left.location.start_byte.cmp(&right.location.start_byte))
                .then(left.location.end_byte.cmp(&right.location.end_byte))
                .then(left.symbol.cmp(&right.symbol))
        });
        candidates.dedup_by(|current, previous| {
            current.symbol == previous.symbol && current.location == previous.location
        });
        for candidate in &candidates {
            self.evidence.declaration(Some(candidate));
        }
        Some(CallTargetSet {
            exhaustive: true,
            candidates: candidates.into(),
        })
    }

    /// Go: `resolvedCallNeedsDiagnosticsBeforeCalleeType`.
    fn needs_diagnostics_before_callee_type(&mut self, node: P<Node>) -> bool {
        if is_new_expression(node) || !node.type_arguments().is_empty() {
            return true;
        }
        node.arguments().iter().any(|argument| {
            is_spread_element(*argument)
                || self.env.checker.is_context_sensitive_exported(*argument)
        })
    }

    /// Go: `resolvedCallValidityAndCalleeTypeLocked`, the single recovery
    /// policy for resolved calls and call-result domains. The diagnostic-first
    /// cases must not inspect a potentially misleading callee type before
    /// diagnostics have been checked, and overload-failure signatures are never
    /// proof of a valid call.
    pub fn validity_and_callee_type(
        &mut self,
        node: P<Node>,
        signature: P<Signature>,
        target: Option<P<Symbol>>,
    ) -> Result<(ResolvedCallValidity, Option<P<Type>>), BackendError> {
        if signature
            .flags()
            .intersects(SignatureFlags::IsSignatureCandidateForOverloadFailure)
            && !node
                .arguments()
                .iter()
                .any(|argument| is_spread_element(*argument))
        {
            return Ok((ResolvedCallValidity::Recovery, None));
        }
        if self.needs_diagnostics_before_callee_type(node) {
            let validity = self.diagnostic_validity(node)?;
            if validity == ResolvedCallValidity::Valid {
                let callee = node.expression().expect("a call-like node has a callee");
                return Ok((
                    validity,
                    Some(self.env.checker.get_type_at_location(callee)),
                ));
            }
            return Ok((validity, None));
        }
        let callee = node.expression().expect("a call-like node has a callee");
        let callee_type = self.env.checker.get_type_at_location(callee);
        // Go: `resolvedCallValidityLocked`: the proof, else the diagnostics.
        let proof = self.prove_validity(node, signature, target, callee_type);
        let validity = match proof {
            Some(proof) => proof,
            None => self.diagnostic_validity(node)?,
        };
        Ok((validity, Some(callee_type)))
    }

    /// Go: `resolvedCallDiagnosticValidityLocked`.
    fn diagnostic_validity(&mut self, node: P<Node>) -> Result<ResolvedCallValidity, BackendError> {
        let failed = self.call_diagnostics()?.contains(&node.addr());
        Ok(if failed {
            ResolvedCallValidity::Recovery
        } else {
            ResolvedCallValidity::Valid
        })
    }

    /// Go: `callDiagnosticIndexLocked`.
    fn call_diagnostics(&mut self) -> Result<&HashSet<usize>, BackendError> {
        let file = self.file;
        if !self.caches.diagnostics.contains_key(&file.addr()) {
            let diagnostics = semantic_diagnostics(self.env.program, self.env.checker, file);
            check(self.ctx)?;
            let mut index: HashSet<usize> = HashSet::new();
            for diagnostic in diagnostics {
                let (Some(pos), Some(end)) = (diagnostic.pos, diagnostic.end) else {
                    continue;
                };
                if !is_call_resolution_diagnostic(diagnostic.code) || end < pos {
                    continue;
                }
                let mut node = Some(node_at(file, pos));
                while let Some(current) = node
                    && !is_call_like_expression(current)
                {
                    node = current.parent();
                }
                let Some(node) = node else {
                    continue;
                };
                let (Some(node_pos), Some(node_end)) =
                    (node.pos().source_offset(), node.end().source_offset())
                else {
                    continue;
                };
                if pos < node_pos || end > node_end {
                    continue;
                }
                index.insert(node.addr());
            }
            check(self.ctx)?;
            self.caches.diagnostics.insert(file.addr(), index);
        }
        Ok(&self.caches.diagnostics[&file.addr()])
    }

    /// Go: `proveResolvedCallValidity`.
    fn prove_validity(
        &mut self,
        node: P<Node>,
        signature: P<Signature>,
        target: Option<P<Symbol>>,
        callee_type: P<Type>,
    ) -> Option<ResolvedCallValidity> {
        if signature.declaration().is_none() || target.is_none() {
            return None;
        }
        let uncertain = TypeFlags::Any
            | TypeFlags::Unknown
            | TypeFlags::Never
            | TypeFlags::Undefined
            | TypeFlags::Null
            | TypeFlags::Union
            | TypeFlags::IncludesError;
        if has_flag(callee_type, uncertain) {
            return None;
        }
        let arguments = node.arguments();
        if arguments.len() < signature.min_argument_count()
            || (!signature.has_rest_parameter() && arguments.len() > signature.parameters().len())
        {
            return Some(ResolvedCallValidity::Recovery);
        }
        for (index, argument) in arguments.iter().enumerate() {
            let argument_type = self.env.checker.get_type_at_location(*argument);
            let parameter_type = self
                .env
                .checker
                .get_type_at_position_exported(signature, index);
            if !self
                .env
                .checker
                .is_type_assignable_to(argument_type, parameter_type)
            {
                return Some(ResolvedCallValidity::Recovery);
            }
        }
        Some(ResolvedCallValidity::Valid)
    }

    /// Go: `fillArgumentMappingsLocked`.
    #[allow(clippy::too_many_lines)]
    fn argument_mappings(
        &mut self,
        call: P<Node>,
        signature: P<Signature>,
        signature_declaration: Option<P<Node>>,
        object_shape: bool,
    ) -> Arc<[ArgumentMapping]> {
        let arguments = call.arguments();
        let parameters = signature.parameters();
        let current_parameters = signature_declaration.map_or(&[][..], parameters_of);
        let mut result: Vec<ArgumentMapping> = Vec::with_capacity(arguments.len());
        for (argument_index, argument) in arguments.iter().enumerate() {
            let mut mapping = ArgumentMapping {
                argument_index: argument_index as u64,
                status: ArgumentMappingStatus::Unresolved,
                unresolved: None,
                parameter: None,
            };
            if is_spread_element(*argument) {
                mapping.unresolved = Some(ArgumentMappingReason::SpreadArgument);
                result.push(mapping);
                continue;
            }
            let key = (signature.addr(), argument_index, object_shape);
            if let Some(cached) = self.caches.parameters.get(&key) {
                self.evidence.parameter(cached);
                mapping.status = ArgumentMappingStatus::Resolved;
                mapping.parameter = Some(cached.clone());
                result.push(mapping);
                continue;
            }

            let mut formal_index = argument_index;
            if formal_index >= parameters.len() {
                if !signature.has_rest_parameter() || parameters.is_empty() {
                    mapping.unresolved = Some(ArgumentMappingReason::ParameterUnavailable);
                    result.push(mapping);
                    continue;
                }
                formal_index = parameters.len() - 1;
            }
            let mut parameter = Some(parameters[formal_index]);
            if let Some(current) = current_parameters.get(formal_index) {
                if let Some(symbol) = current.symbol() {
                    parameter = Some(symbol);
                } else if let Some(name) = current.name()
                    && let Some(symbol) = self.env.checker.get_symbol_at_location_exported(name)
                {
                    parameter = Some(symbol);
                }
            }
            let parameter_type = self
                .env
                .checker
                .get_type_at_position_exported(signature, argument_index);
            let rest = signature.has_rest_parameter() && formal_index == parameters.len() - 1;
            let mut fact = ParameterFact {
                index: formal_index as u64,
                symbol: "".into(),
                declaration: None,
                rest,
                optional: !rest && formal_index >= signature.min_argument_count(),
                callability: callability_of_type(self.env.checker, parameter_type),
                type_descriptor: None,
                object_shape: None,
            };
            if let Some(parameter) = parameter {
                fact.optional =
                    fact.optional || (!rest && parameter.flags().intersects(SymbolFlags::Optional));
                fact.symbol = self.env.id_for(parameter);
                fact.declaration = declarations_for_symbol(parameter).into_iter().next();
            }
            fact.type_descriptor =
                Some((*self.memo.descriptor(self.env.checker, parameter_type)).clone());
            if object_shape {
                let mut shape_type = parameter_type;
                // Construction precedes generic inference. Read the selected
                // declaration signature when it exists so a bottom-typed probe
                // argument does not collapse every type parameter to never.
                // The completed candidate call is still validated against the
                // inferred instantiated signature by ordinary resolvedCall
                // validity.
                if let Some(declaration) = signature_declaration {
                    let declared = self
                        .env
                        .checker
                        .get_signature_from_declaration_exported(declaration);
                    shape_type = self
                        .env
                        .checker
                        .get_type_at_position_exported(declared, argument_index);
                }
                fact.object_shape = self.object_construction_shape(shape_type);
            }
            self.caches.parameters.insert(key, fact.clone());
            self.evidence.parameter(&fact);
            mapping.status = ArgumentMappingStatus::Resolved;
            mapping.parameter = Some(fact);
            result.push(mapping);
        }
        result.into()
    }

    /// Go: `objectConstructionShape`: bounded, side-effect-free candidates
    /// derived from the selected declaration parameter and its compiler
    /// constraints. It never renders or parses type text. The candidates become
    /// proof only when the consumer validates the completed synthetic call with
    /// resolvedCall; unknown witnesses remain explicit so the consumer must
    /// enumerate or stop.
    fn object_construction_shape(&mut self, value: P<Type>) -> Option<ObjectConstructionShape> {
        if has_flag(value, OPEN_CONSTRUCTION_SHAPE | TypeFlags::TypeParameter) {
            return None;
        }
        let descriptor = self.memo.descriptor(self.env.checker, value);
        self.evidence.descriptor(Some(&descriptor));
        let properties = self.env.checker.get_properties_of_type_exported(value);
        let mut required: Vec<(String, ConstructionWitness)> = Vec::with_capacity(properties.len());
        for property in properties {
            if property.flags().intersects(SymbolFlags::Optional) {
                continue;
            }
            let name = property.name();
            let property_type = self
                .env
                .checker
                .get_type_of_property_of_type_exported(value, name)?;
            let descriptor = self.memo.descriptor(self.env.checker, property_type);
            self.evidence.descriptor(Some(&descriptor));
            let witness = self.construction_witness(property_type);
            required.push((name.to_owned(), witness));
        }
        // Go sorts the raw names, where an internal name starts with the byte
        // 0xFE and so follows every ordinary name; tsrs's marker (U+007F) would
        // sort before non-ASCII names. Sort by (internal, name) to match.
        required.sort_by(|left, right| {
            let key = |name: &str| match name.strip_prefix(tsrs_ast::InternalSymbolNamePrefix) {
                Some(rest) => (true, rest.to_owned()),
                None => (false, name.to_owned()),
            };
            key(&left.0).cmp(&key(&right.0))
        });
        Some(ObjectConstructionShape {
            required_properties: required
                .into_iter()
                .map(|(name, witness)| ObjectConstructionProperty {
                    name: wire_symbol_name(&name).into(),
                    witness,
                })
                .collect(),
        })
    }

    /// Go: `constructionWitness`.
    fn construction_witness(&mut self, value: P<Type>) -> ConstructionWitness {
        if has_flag(value, OPEN_CONSTRUCTION_SHAPE) {
            return ConstructionWitness::Unknown;
        }
        if array_shape_of_type(self.env.checker, value) == ArrayShape::Array {
            for constituent in value.distributed() {
                if constituent.is_tuple_type() {
                    // Go: a nil `TargetTupleType()` is unknown; tsrs's cannot be.
                    let target = constituent.target_tuple_type();
                    if target
                        .element_flags()
                        .iter()
                        .any(|flags| !flags.intersects(ElementFlags::NonRequired))
                    {
                        return ConstructionWitness::Unknown;
                    }
                }
            }
            return ConstructionWitness::EmptyArray;
        }
        if self.empty_object_inhabits(value, &mut HashSet::new()) {
            return ConstructionWitness::EmptyObject;
        }
        ConstructionWitness::Unknown
    }

    /// Go: `emptyObjectInhabits`.
    fn empty_object_inhabits(&mut self, value: P<Type>, seen: &mut HashSet<usize>) -> bool {
        if has_flag(value, OPEN_CONSTRUCTION_SHAPE) {
            return false;
        }
        if !seen.insert(value.addr()) {
            return false;
        }
        let answer = self.empty_object_inhabits_walk(value, seen);
        seen.remove(&value.addr());
        answer
    }

    fn empty_object_inhabits_walk(&mut self, value: P<Type>, seen: &mut HashSet<usize>) -> bool {
        if has_flag(value, TypeFlags::TypeParameter) {
            return match self.env.checker.get_base_constraint_of_type_exported(value) {
                Some(constraint) => self.empty_object_inhabits(constraint, seen),
                None => false,
            };
        }
        if has_flag(value, TypeFlags::Intersection) {
            let constituents = value.types();
            if constituents.is_empty() {
                return false;
            }
            return constituents
                .iter()
                .all(|constituent| self.empty_object_inhabits(*constituent, seen));
        }
        if has_flag(value, TypeFlags::Union) {
            return value
                .types()
                .iter()
                .any(|constituent| self.empty_object_inhabits(*constituent, seen));
        }
        if !has_flag(value, TypeFlags::Object) {
            return match self.env.checker.get_base_constraint_of_type_exported(value) {
                Some(constraint) if constraint != value => {
                    self.empty_object_inhabits(constraint, seen)
                }
                _ => false,
            };
        }
        let constituents = value.distributed();
        if constituents.is_empty() {
            return false;
        }
        for constituent in constituents {
            let checker = &mut *self.env.checker;
            if !checker
                .get_signatures_of_type_exported(constituent, SignatureKind::Call)
                .is_empty()
                || !checker
                    .get_signatures_of_type_exported(constituent, SignatureKind::Construct)
                    .is_empty()
            {
                return false;
            }
            if checker
                .get_properties_of_type_exported(constituent)
                .iter()
                .any(|property| !property.flags().intersects(SymbolFlags::Optional))
            {
                return false;
            }
        }
        true
    }

    /// Go: `callResultDomainAtLocked`: applies the same call-resolution
    /// recovery guard as resolvedCall before classifying the call
    /// expression's result type. A recovery signature can still expose the
    /// declared return type, which is not safe evidence for a result-domain
    /// query.
    pub fn call_result_domain(
        &mut self,
        node: P<Node>,
    ) -> Result<RuntimeValueDomain, BackendError> {
        let signature = self.env.checker.get_resolved_signature_exported(node);
        let callee = node.expression().expect("a call-like node has a callee");
        let target = self
            .env
            .checker
            .get_symbol_at_location_exported(callee)
            .map(|symbol| self.env.canonical_symbol(symbol));
        let (validity, _) = self.validity_and_callee_type(node, signature, target)?;
        if validity != ResolvedCallValidity::Valid {
            return Ok(unknown_runtime_value_domain());
        }
        let value = self.env.checker.get_type_at_location(node);
        Ok(runtime_value_domain_of_type(self.env.checker, value))
    }
}

/// Go: `openConstructionShapeFlags`.
const OPEN_CONSTRUCTION_SHAPE: TypeFlags = TypeFlags::Any
    .union(TypeFlags::Unknown)
    .union(TypeFlags::Never)
    .union(TypeFlags::IncludesError);

/// Go: `fillUnresolvedArgumentMappings`.
fn unresolved_mappings(count: usize, reason: ArgumentMappingReason) -> Arc<[ArgumentMapping]> {
    (0..count)
        .map(|index| ArgumentMapping {
            argument_index: index as u64,
            status: ArgumentMappingStatus::Unresolved,
            unresolved: Some(reason),
            parameter: None,
        })
        .collect()
}

/// Go: `singleOptionalCallConstituent`. Optional chaining excludes nullish
/// alternatives before invocation. A single remaining callable therefore
/// retains the compiler-selected signature and its argument mapping. This is
/// not a runtime target or execution proof: the call may be skipped, and a
/// structural signature stays structural. Multiple callable alternatives and
/// open constituents keep the composite-call path.
fn single_optional_call_constituent(node: P<Node>, callee_type: P<Type>) -> bool {
    if !is_optional_chain(node) {
        return false;
    }
    let open = TypeFlags::Any
        | TypeFlags::Unknown
        | TypeFlags::Never
        | TypeFlags::Instantiable
        | TypeFlags::Union
        | TypeFlags::Intersection
        | TypeFlags::IncludesError;
    let mut count = 0;
    for constituent in callee_type.types() {
        if has_flag(*constituent, TypeFlags::Null | TypeFlags::Undefined) {
            continue;
        }
        if has_flag(*constituent, open) {
            return false;
        }
        count += 1;
    }
    count == 1
}

/// Go: `isCallResolutionDiagnostic`.
fn is_call_resolution_diagnostic(code: i32) -> bool {
    matches!(
        code,
        2344 | 2345
            | 2348
            | 2349
            | 2350
            | 2379
            | 2554
            | 2555
            | 2558
            | 2635
            | 2677
            | 2721
            | 2722
            | 2723
            | 2757
            | 2769
            | 2794
            | 2810
            | 6234
    )
}

/// The library-write census walk of `defaultLibraryBinding`: every property or
/// element write or `delete` in non-declaration source, by name.
fn collect_property_writes(node: P<Node>, writes: &mut HashSet<String>) -> bool {
    let member = is_property_access_expression(node) || is_element_access_expression(node);
    if member
        && let Some(parent) = node.parent()
        && (get_assignment_target(node).is_some() || is_delete_expression(parent))
    {
        if is_property_access_expression(node) {
            if let Some(name) = node.name() {
                writes.insert(name.text().to_owned());
            }
        } else {
            let key =
                transparent_call_callee(node.as_element_access_expression().argument_expression);
            if is_string_literal(key) || is_no_substitution_template_literal(key) {
                writes.insert(key.text().to_owned());
            }
        }
    }
    node.for_each_child(&mut |child| collect_property_writes(child, writes));
    false
}
