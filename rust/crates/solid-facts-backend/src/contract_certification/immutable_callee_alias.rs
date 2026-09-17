//! Binding an immutable callee alias does not change what its call resolves
//! to. It supplies a separate, source-bound path to the callable copied there.

use super::{CensusRun, CensusSourceFacts};
use solid_facts::{ast::IdentifierRole, core::Span};

fn span(location: &typefacts::Location, path: &str) -> Result<Span, String> {
    if location.path.as_ref() != path {
        return Err("immutable callee alias crosses runtime source files".into());
    }
    Ok(Span::new(
        u32::try_from(location.start_byte).map_err(|_| "alias span overflows")?,
        u32::try_from(location.end_byte).map_err(|_| "alias span overflows")?,
    ))
}

fn validate(
    source: &CensusSourceFacts,
    call: &typefacts::ImplementationCall,
    alias: &typefacts::ImmutableCalleeAlias,
) -> Result<(), String> {
    let refuse = || {
        "creates census cannot bind an immutable callee alias to its exact source chain".to_owned()
    };
    if call.kind != typefacts::CallKind::Call
        || alias.bindings.is_empty()
        || alias.bindings.len() > 8
    {
        return Err(refuse());
    }
    let first = &alias.bindings[0].declaration;
    if call.declaration.as_ref() != Some(first)
        || first.symbol.is_empty()
        || call.target != first.symbol
    {
        return Err(refuse());
    }
    let path = call.location.path.as_ref();
    let facts = &source.facts;
    let call_span = span(&call.location, path)?;
    let actual = facts
        .calls
        .iter()
        .find(|node| node.span == call_span && node.direct_callee)
        .ok_or_else(refuse)?;
    let mut reference = facts.peel_ts_sugar_span(actual.callee);
    let mut seen = std::collections::BTreeSet::new();
    for hop in &alias.bindings {
        let declared = span(&hop.declaration.location, path)?;
        if hop.declaration.source_file.as_ref() != path
            || hop.declaration.standard_library
            || !seen.insert(declared)
            || facts.reference_declaration(reference) != Some(declared)
        {
            return Err(refuse());
        }
        let binding = super::census_plain_binding_named_at(facts, (declared.start, declared.end))
            .ok_or_else(refuse)?;
        let initializer = binding
            .initializer
            .map(|value| facts.peel_ts_sugar_span(value))
            .ok_or_else(refuse)?;
        if !binding.immutable
            || binding.declaration.end > reference.start
            || initializer != span(&hop.initializer, path)?
        {
            return Err(refuse());
        }
        let name = source.text_at(declared).ok_or_else(refuse)?;
        if name != hop.declaration.name.as_ref() {
            return Err(refuse());
        }
        for (use_span, target) in &facts.reference_declarations {
            if *target == declared
                && facts
                    .assignments
                    .iter()
                    .map(|write| write.target)
                    .chain(facts.iteration_targets.iter().copied())
                    .any(|write| write.start <= use_span.start && use_span.end <= write.end)
            {
                return Err("creates census refuses a written immutable callee alias".into());
            }
        }
        if facts
            .bindings
            .iter()
            .flat_map(|binding| binding.names.iter().map(|named| named.span))
            .chain(
                facts
                    .function_declarations
                    .iter()
                    .map(|function| function.name.span),
            )
            .chain(
                facts
                    .classes
                    .iter()
                    .filter_map(|class| class.name.as_ref().map(|named| named.span)),
            )
            .any(|other| other != declared && source.text_at(other) == Some(name))
        {
            return Err("creates census refuses a redeclared immutable callee alias".into());
        }
        reference = initializer;
    }
    if reference != span(&alias.expression, path)? || alias.declaration.symbol.is_empty() {
        return Err(refuse());
    }
    if !alias.declaration.standard_library {
        if alias.receiver.is_some()
            || !alias.default_library_invoker.is_empty()
            || !alias.invoked_arguments.is_empty()
            || facts.reference_declaration(reference)
                != Some(span(&alias.declaration.location, path)?)
        {
            return Err(refuse());
        }
        return Ok(());
    }
    // The producer resolves both identities to default-library declarations.
    // The independent parse only binds that fact to the terminal expression;
    // an absent local reference is not itself default-library authority.
    let root = if let Some(receiver) = &alias.receiver {
        if !receiver.standard_library || receiver.symbol.is_empty() {
            return Err(refuse());
        }
        let member = facts
            .members
            .iter()
            .find(|member| {
                member.span == reference && !facts.computed_members.contains(&member.span)
            })
            .ok_or_else(refuse)?;
        if source.text_at(member.property) != Some(alias.declaration.name.as_ref())
            || source.text_at(member.object) != Some(receiver.name.as_ref())
        {
            return Err(refuse());
        }
        member.object
    } else {
        if source.text_at(reference) != Some(alias.declaration.name.as_ref()) {
            return Err(refuse());
        }
        reference
    };
    if facts.reference_declaration(root).is_some()
        || !facts.identifiers.iter().any(|identifier| {
            identifier.span == root && identifier.role == IdentifierRole::Reference
        })
    {
        return Err(refuse());
    }
    let root_name = source.text_at(root).ok_or_else(refuse)?;
    for identifier in &facts.identifiers {
        if identifier.role != IdentifierRole::Reference
            || source.text_at(identifier.span) != Some(root_name)
            || facts.reference_declaration(identifier.span).is_some()
        {
            continue;
        }
        if facts
            .assignments
            .iter()
            .map(|write| write.target)
            .chain(facts.iteration_targets.iter().copied())
            .any(|write| write.start <= identifier.span.start && identifier.span.end <= write.end)
        {
            return Err("creates census refuses a written default-library alias target".into());
        }
        if alias.receiver.is_some()
            && !facts.members.iter().any(|member| {
                member.object == identifier.span && !facts.computed_members.contains(&member.span)
            })
        {
            return Err("creates census refuses an escaped default-library alias receiver".into());
        }
    }
    Ok(())
}

pub(super) fn rebind(
    run: &mut CensusRun<'_>,
    call: &typefacts::ImplementationCall,
) -> Result<Option<typefacts::ImplementationCall>, String> {
    let Some(alias) = &call.immutable_callee_alias else {
        return Ok(None);
    };
    let first = alias
        .bindings
        .first()
        .ok_or("creates census received an empty immutable callee alias")?;
    let (_, relative) = super::census_local_declaration_identity(run, &first.declaration)
        .ok_or("immutable callee alias is outside this artifact's runtime source")?;
    validate(super::census_source(run, &relative)?, call, alias)?;
    run.sites.push(format!(
        "immutable-callee-alias:{}",
        serde_json::to_string(alias).expect("native alias fact encoding")
    ));
    let mut rebound = call.clone();
    rebound.immutable_callee_alias = None;
    rebound.declaration = Some(alias.declaration.clone());
    rebound.target = alias.declaration.symbol.clone();
    rebound.target_name = alias.declaration.name.clone();
    rebound.target_module = alias.declaration.origin_module.clone();
    rebound.default_library_invoker = alias.default_library_invoker.clone();
    rebound.invoked_arguments = alias.invoked_arguments.clone();
    Ok(Some(rebound))
}
