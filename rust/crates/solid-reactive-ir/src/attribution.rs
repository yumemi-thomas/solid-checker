//! Which project functions can reach an unresolved proof obligation.
//!
//! Contract emission has to say *which exports* an open claim belongs to.
//! An obligation that sits inside an exported function answers that question
//! lexically, and emission resolves that case itself from the AST. An
//! obligation inside a private helper does not: the only thing that can say
//! whether a consumer of export `A` — and not of export `B` — can trigger it
//! is the call graph.
//!
//! The call graph lives here, beside the interprocedural analysis that owns
//! it, and emission consumes only the answer ([`Program::obligation_reach`]).
//! The alternative — handing the raw graph across the process boundary and
//! walking it in the emitter — would put two independently drifting notions of
//! "calls" in the codebase, and the emitter's copy would be the one nobody
//! tests against real reactive code.
//!
//! The answer is deliberately shaped as *fail closed or exact*: either
//! [`ObligationReach::complete`] is true and `reaching` enumerates every
//! project function that can enter the obligation's own function, or it is
//! false and emission must fall back to marking every export. There is no
//! third "probably these" state, because a partial enumeration read as a
//! complete one silently certifies an export that can reach the obligation.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};

use serde::{Deserialize, Serialize};
use solid_facts::{FileFacts, ProjectFacts, core::Span};
use typefacts::Location;

use crate::identity::SymbolId;
use crate::indexes::{EntitySymbols, SemanticLookup};
use crate::{StaticDefect, location};

/// The exports-reaching answer for one unresolved proof obligation.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObligationReach {
    /// The obligation's defect location, as emission looks it up by.
    pub location: Location,
    /// The **body** span of every project function that can transitively enter
    /// the obligation's own function, that function included.
    ///
    /// Body spans rather than declaration spans: the consumer joins a function
    /// to an export name by finding the function whose body contains a span,
    /// and a declaration span starts before the body.
    pub reaching: Vec<Location>,
    /// Whether `reaching` is a complete enumeration.
    ///
    /// False when a function on the path is entered by something this analysis
    /// cannot enumerate — a module-level call, or a function value escaping
    /// into a callee the graph could not resolve. Emission must then mark every
    /// export rather than trust the partial set.
    pub complete: bool,
}

/// The reach answer for every distinct unresolved obligation in `defects`.
///
/// Returns empty when the project has no unresolved obligation, which is the
/// common case and the reason this is not built unconditionally: the escape
/// analysis below walks every reference of every function on a path, and a
/// certified project should not pay for a question nobody asks.
pub(crate) fn obligation_reach(
    facts: &ProjectFacts,
    lookup: &SemanticLookup<'_>,
    entities: &EntitySymbols,
    aliases: &HashMap<SymbolId, SymbolId>,
    symbols_by_root: &HashMap<SymbolId, Vec<SymbolId>>,
    defects: &[StaticDefect],
) -> Vec<ObligationReach> {
    let mut seen = HashSet::new();
    let locations = defects
        .iter()
        .filter(|defect| defect.kind.is_unresolved_obligation())
        .map(|defect| &defect.location)
        .filter(|location| {
            seen.insert((
                location.path.clone(),
                location.start_byte,
                location.end_byte,
            ))
        })
        .collect::<Vec<_>>();
    if locations.is_empty() {
        return Vec::new();
    }
    let graph = CallGraph {
        files_by_path: facts
            .files
            .iter()
            .map(|file| (file.path.as_str(), file))
            .collect(),
        lookup,
        entities,
        aliases,
        symbols_by_root,
        entered_only_through_calls: RefCell::new(HashMap::new()),
    };
    locations
        .into_iter()
        .filter_map(|location| graph.reach(location))
        .collect()
}

struct CallGraph<'a, 'b> {
    files_by_path: HashMap<&'a str, &'a FileFacts>,
    lookup: &'a SemanticLookup<'b>,
    entities: &'a EntitySymbols,
    aliases: &'a HashMap<SymbolId, SymbolId>,
    symbols_by_root: &'a HashMap<SymbolId, Vec<SymbolId>>,
    /// One escape verdict per function. Obligations cluster in the same few
    /// functions, and the verdict walks every reference of a symbol; without
    /// this the same walk runs once per obligation that reaches the function.
    entered_only_through_calls: RefCell<HashMap<(&'a str, Span), bool>>,
}

impl<'a> CallGraph<'a, '_> {
    fn reach(&self, obligation: &Location) -> Option<ObligationReach> {
        let file = self.file(obligation.path.as_ref())?;
        let span = Span::new(
            u32::try_from(obligation.start_byte).unwrap_or(u32::MAX),
            u32::try_from(obligation.end_byte).unwrap_or(u32::MAX),
        );
        // The outermost enclosing function, not the innermost: a nested arrow
        // is entered whenever its enclosing declaration is, and only the
        // outermost declaration is a call-graph node other functions name.
        let start = outermost_function(file, span).or_else(|| {
            // Not every obligation sits *inside* a body. The exported-helper
            // obligations are filed at the helper's own declaration span,
            // which no body contains; without this the graph declined to
            // answer for them and emission fell back to marking every export
            // of the entrypoint — including exports that provably cannot call
            // the helper.
            file.ast
                .functions
                .iter()
                .find(|function| function.span == span)
        })?;
        let mut queue = VecDeque::from([(file.path.as_str(), start.span, start.body)]);
        let mut visited = HashSet::from([(file.path.as_str(), start.span)]);
        let mut reaching = Vec::new();
        let mut complete = true;
        while let Some((path, function, body)) = queue.pop_front() {
            reaching.push(location(path, body));
            if !self.entered_only_through_calls(path, function) {
                complete = false;
            }
            for (caller, callee) in self.lookup.function_call_sites(path, function) {
                let Some(owner) = outermost_function(caller, callee) else {
                    // A call at module scope runs when the module is imported,
                    // so every consumer of the entrypoint reaches it.
                    complete = false;
                    continue;
                };
                if visited.insert((caller.path.as_str(), owner.span)) {
                    queue.push_back((caller.path.as_str(), owner.span, owner.body));
                }
            }
        }
        reaching.sort_by(crate::location_order);
        Some(ObligationReach {
            location: obligation.clone(),
            reaching,
            complete,
        })
    }

    /// Whether every way of entering this function is one of the call sites
    /// the graph enumerated.
    ///
    /// A function referenced anywhere other than its own declaration, one of
    /// those call sites, or the module surface that declares it, has escaped as
    /// a value: something the graph did not model can invoke it, so the caller
    /// set is not the whole entry set. Callers treat that as fail-closed.
    fn entered_only_through_calls(&self, path: &'a str, function: Span) -> bool {
        if let Some(cached) = self
            .entered_only_through_calls
            .borrow()
            .get(&(path, function))
        {
            return *cached;
        }
        let verdict = self.compute_entered_only_through_calls(path, function);
        self.entered_only_through_calls
            .borrow_mut()
            .insert((path, function), verdict);
        verdict
    }

    fn compute_entered_only_through_calls(&self, path: &str, function: Span) -> bool {
        let Some(file) = self.file(path) else {
            return false;
        };
        let Some(declaration) = file
            .ast
            .functions
            .iter()
            .find(|candidate| candidate.span == function)
            .and_then(|function| crate::owners::function_binding_name(file, function))
        else {
            // No binding name: nothing can name it, so the only entry is the
            // expression it was written in. That expression is inside the
            // enclosing function the walk already visited, or at module scope,
            // and neither is enumerable from here.
            return false;
        };
        let Some(symbol) = self.entities.at(path, declaration.span) else {
            return false;
        };
        let root = self.aliases.get(symbol).unwrap_or(symbol);
        // References, not sites: this test asks whether every reference to the
        // function is accounted for, and one render can write the component's
        // name twice (`<Panel></Panel>`). The call graph still holds one edge
        // per invocation.
        let known_call_sites = self
            .lookup
            .function_call_site_references(path, function)
            .into_iter()
            .map(|(caller, callee)| (caller.path.to_string(), callee.start, callee.end))
            .collect::<HashSet<_>>();
        let mut aliased = self
            .symbols_by_root
            .get(root)
            .cloned()
            .unwrap_or_else(|| vec![symbol.clone()]);
        if !aliased.iter().any(|candidate| candidate == symbol) {
            aliased.push(symbol.clone());
        }
        aliased.iter().all(|candidate| {
            self.lookup
                .symbol_references(candidate.as_str())
                .iter()
                .all(|reference| self.reference_is_accounted_for(reference, &known_call_sites))
        })
    }

    fn reference_is_accounted_for(
        &self,
        reference: &Location,
        known_call_sites: &HashSet<(String, u32, u32)>,
    ) -> bool {
        let start = u32::try_from(reference.start_byte).unwrap_or(u32::MAX);
        let end = u32::try_from(reference.end_byte).unwrap_or(u32::MAX);
        if known_call_sites.contains(&(reference.path.to_string(), start, end)) {
            return true;
        }
        let Some(file) = self.file(reference.path.as_ref()) else {
            // A reference in a file outside the analyzed project — a bundled
            // declaration, say — is not a runtime entry this project owns.
            return true;
        };
        let span = Span::new(start, end);
        // The declaration that introduces the function, and the import/export
        // surface that forwards it, are not runtime entries: emission resolves
        // the export surface itself, by name.
        if file
            .ast
            .functions
            .iter()
            .any(|function| function.name.as_ref().is_some_and(|name| name.span == span))
        {
            return true;
        }
        if file
            .ast
            .bindings
            .iter()
            .any(|binding| binding.names.iter().any(|name| name.span == span))
        {
            return true;
        }
        // Only the export *specifier* — `export { Panel }`, or the name a
        // declaration export introduces. An `ExportNamedDeclaration`'s own span
        // covers the whole declaration, body included (solid-facts
        // `visit_export_named_declaration`), so testing containment accepted
        // every reference written inside an exported function: `apply(Panel)`
        // and `return Panel` read as "export surface", and the escape they
        // prove was lost. That is the unsound direction — the caller reads a
        // true verdict as "the enumerated callers are all of them" and
        // certifies exports that a value-escaped function can reach.
        //
        // `<Panel/>` is not in that list, and is not accepted here either: a
        // rendered tag is a call site, so `known_call_sites` above already
        // holds its span — both of its spans, for `<Panel></Panel>`, since the
        // closing tag rides on the same edge. The acceptance is the call-graph
        // edge, not a syntactic exception for tags, so a tag that resolves to
        // nothing (an unresolved import) reaches none of these branches, which
        // is the honest answer: something the graph cannot name renders it.
        //
        // A dotted tag is the case where the edge exists and the reference is
        // still unaccounted for. `<ns.Panel/>` *does* resolve: TypeScript
        // reports the symbol at the whole `ns.Panel` name span, so the graph
        // emits an edge whose callee is that whole span. But the reference this
        // test walks is the `Panel` property *inside* the name, and this set is
        // a byte-exact span membership test, so the property does not match the
        // whole name and the enumeration reports itself incomplete. That is the
        // conservative direction; closing it means making the edge's callee and
        // the walked reference name the same span, which is a resolution
        // question rather than a widening one.
        if file.ast.exports.iter().any(|export| {
            export
                .specifiers
                .iter()
                .chain(export.declarations.iter())
                .any(|specifier| specifier.local.span == span)
        }) {
            return true;
        }
        file.ast
            .imports
            .iter()
            .any(|import| import.span.contains(span))
    }

    fn file(&self, path: &str) -> Option<&'a FileFacts> {
        self.files_by_path.get(path).copied()
    }
}

/// The largest function body containing `span`, i.e. the top-level declaration
/// the span lexically belongs to.
fn outermost_function(file: &FileFacts, span: Span) -> Option<&solid_facts::ast::FunctionFact> {
    file.ast
        .functions_body_containing(span)
        .max_by_key(|function| function.body.end - function.body.start)
}
