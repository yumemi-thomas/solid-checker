//! ADR 1087, step 0 (census only): whether a tracked async computation can run the async function a read-after-await site is written in.
//!
//! The fact the pass's header names under "No census reason": no computation runs the function, so no flight is on its async stack when a
//! continuation reads. It is asked as a complete census of the function's references, never as the absence of a recognized start: every
//! reference is enumerated ([`crate::local_access::function_reference_hand_offs`], [`crate::local_access::foreign_reference_hand_offs`] and the
//! call graph's resolved calls), and each must be a direct call, or a standard carrier that runs the function inline (`forEach`, `f.call`,
//! `Reflect.apply`), whose calling function has the same property, or a call written at module level. Anything else (a value handed to a
//! project or unknown call, a copy, a fresh-stack scheduler, a host listener, an export unseen code may import, a component, a method, a
//! function literal that is not called where it is written, the depth bound) is the first blocker, and the fact fails.
//!
//! Step 0 records the answer in the blocker census (`SOLID_CHECKER_BLOCKER_CENSUS=1`, ADR 1061) and nothing else: no row, verdict or text
//! moves. The cause string is `flight|proved|<functions>|fn=<start>` or `flight|blocked|<kind>|<detail>|depth=<n>|fn=<start>` (`<start>` the byte offset of the
//! function the site is written in, so a reader counts functions).

use solid_facts::FileFacts;
use solid_facts::ast::FunctionFact;
use solid_facts::core::Span;

use crate::indexes::SemanticLookup;
use crate::local_access::HandOff;
use crate::{HashMap, HashSet};

/// How deep the census follows callers before it stops (a blocker of its own).
const DEPTH: usize = 12;

type Key = (String, u32, u32);

/// Per pass: the answer of each function the census asked, so a function shared by many sites is asked once.
#[derive(Default)]
pub(super) struct FlightCensus {
    answers: HashMap<Key, Result<usize, String>>,
}

impl FlightCensus {
    /// The census cause for the site at `at` of `file`.
    pub(super) fn cause(
        &mut self,
        file: &FileFacts,
        at: Span,
        lookup: &SemanticLookup<'_>,
    ) -> String {
        let Some(function) = crate::owners::owning_function(&file.ast, at) else {
            return "flight|blocked|module-scope|-|depth=0".to_owned();
        };
        let mut visiting = HashSet::default();
        let id = function.span.start;
        match self.function(file, function, lookup, 0, &mut visiting) {
            Ok(functions) => format!("flight|proved|{functions}|fn={id}"),
            Err(blocked) => format!("flight|blocked|{blocked}|fn={id}"),
        }
    }

    /// `Ok(n)`: no tracked computation runs `function` (`n` functions asked); `Err(kind|detail|depth=d)`: the first reference that fails.
    fn function(
        &mut self,
        file: &FileFacts,
        function: &FunctionFact,
        lookup: &SemanticLookup<'_>,
        depth: usize,
        visiting: &mut HashSet<Key>,
    ) -> Result<usize, String> {
        let key = (
            file.path.to_string(),
            function.span.start,
            function.span.end,
        );
        if let Some(answer) = self.answers.get(&key) {
            return answer.clone();
        }
        // A function entered only from itself, or from functions the census is asking now, adds no entry (a flight must come in through a reference
        // outside the cycle, which the census asks where it stands).
        if !visiting.insert(key.clone()) {
            return Ok(0);
        }
        let answer = self.function_uncached(file, function, lookup, depth, visiting);
        visiting.remove(&key);
        self.answers.insert(key, answer.clone());
        answer
    }

    fn function_uncached(
        &mut self,
        file: &FileFacts,
        function: &FunctionFact,
        lookup: &SemanticLookup<'_>,
        depth: usize,
        visiting: &mut HashSet<Key>,
    ) -> Result<usize, String> {
        let blocked = |kind: &str, detail: &str| Err(format!("{kind}|{detail}|depth={depth}"));
        if depth >= DEPTH {
            return blocked("bound", "-");
        }
        if file
            .ast
            .module_hazards
            .iter()
            .any(|hazard| hazard.kind == solid_facts::ast::ModuleHazardKind::Eval)
        {
            return blocked("eval", "-");
        }
        if function.method_name.is_some() {
            return blocked(
                "method",
                &short(file, function.method_name.as_ref().map(|name| name.span)),
            );
        }
        let named = function.name.is_some()
            || crate::owners::function_binding_name(file, function).is_some();
        if !named {
            // A literal called where it is written runs on its caller's stack; any other literal is a value (an argument, a property, a return).
            if let Some(call) = file
                .ast
                .calls_with_peeled_callee(function.span)
                .find(|call| !call.construct)
            {
                return self
                    .caller(file, call.span, lookup, depth, visiting)
                    .map(|n| n + 1);
            }
            if let Some((call, _)) =
                file.ast
                    .arguments_containing(function.span)
                    .find(|(call, index)| {
                        file.ast.peel_ts_sugar_span(call.arguments[*index].span) == function.span
                    })
            {
                return blocked("literal-argument", &short(file, Some(call.callee)));
            }
            return blocked("literal-value", "-");
        }
        if !lookup.function_value_is_current(file, function) {
            return blocked("rebound", &name_of(file, function));
        }
        if crate::execution_role::entered_by_unseen_code(file, function, lookup) {
            return blocked("unseen-entry", &name_of(file, function));
        }
        if lookup.function_may_be_component(file, function) {
            return blocked("component", &name_of(file, function));
        }
        let Some(own) = crate::local_access::function_reference_hand_offs(file, function, lookup)
        else {
            return blocked("no-binding", &name_of(file, function));
        };
        let mut functions = 1;
        for (at, hand_off) in own {
            functions += self.reference(file, at, &hand_off, lookup, depth, visiting)?;
        }
        // Calls and renders the call graph resolves in other files, and every other foreign reference with its hand-off.
        for (caller, callee) in lookup.function_call_sites(file.path.as_str(), function.span) {
            if caller.path == file.path {
                continue;
            }
            let Some(call) = caller.ast.calls.iter().find(|call| {
                caller.ast.peel_ts_sugar_span(call.callee) == callee || call.callee == callee
            }) else {
                return blocked("foreign-render", &short(caller, Some(callee)));
            };
            functions += self.caller(caller, call.span, lookup, depth, visiting)?;
        }
        for (holder, at, hand_off) in
            crate::local_access::foreign_reference_hand_offs(file, function, lookup)
        {
            functions += self.reference(holder, at, &hand_off, lookup, depth, visiting)?;
        }
        Ok(functions)
    }

    /// One reference of the function at `at` of `file`, handed on as `hand_off`.
    fn reference(
        &mut self,
        file: &FileFacts,
        at: Span,
        hand_off: &HandOff,
        lookup: &SemanticLookup<'_>,
        depth: usize,
        visiting: &mut HashSet<Key>,
    ) -> Result<usize, String> {
        let blocked = |kind: &str| Err(format!("{kind}|{}|depth={depth}", short(file, Some(at))));
        if file.ast.type_queries.iter().any(|query| query.contains(at)) {
            return Ok(0);
        }
        match hand_off {
            HandOff::Call => match file
                .ast
                .calls
                .iter()
                .find(|call| file.ast.peel_ts_sugar_span(call.callee) == at)
            {
                Some(call) => self.caller(file, call.span, lookup, depth, visiting),
                None => blocked("call-unplaced"),
            },
            HandOff::Inline(carrier) => self.caller(file, *carrier, lookup, depth, visiting),
            HandOff::Value => Ok(0),
            HandOff::FreshStack => blocked("fresh-stack"),
            HandOff::Retained => blocked("host-retained"),
            HandOff::Unfollowed => blocked(&format!("unfollowed:{}", context(file, at))),
        }
    }

    /// A call at `span` of `file`: written at module level, or in a function no tracked computation runs.
    fn caller(
        &mut self,
        file: &FileFacts,
        span: Span,
        lookup: &SemanticLookup<'_>,
        depth: usize,
        visiting: &mut HashSet<Key>,
    ) -> Result<usize, String> {
        match crate::owners::owning_function(&file.ast, span) {
            None => Ok(0),
            Some(caller) => self.function(file, caller, lookup, depth + 1, visiting),
        }
    }
}

/// The first 50 characters of the source at `span`, on one line.
fn short(file: &FileFacts, span: Option<Span>) -> String {
    span.and_then(|span| file.source_text(span)).map_or_else(
        || "-".to_owned(),
        |text| {
            text.split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
                .chars()
                .take(50)
                .collect()
        },
    )
}

fn name_of(file: &FileFacts, function: &FunctionFact) -> String {
    short(
        file,
        function
            .name
            .as_ref()
            .map(|name| name.span)
            .or_else(|| crate::owners::function_binding_name(file, function).map(|name| name.span)),
    )
}

/// Where an unfollowed reference stands: the callee of the call it is an argument of, or `value`.
fn context(file: &FileFacts, at: Span) -> String {
    file.ast
        .arguments_containing(at)
        .min_by_key(|(call, _)| call.span.end - call.span.start)
        .map_or_else(
            || "value".to_owned(),
            |(call, _)| format!("arg-of={}", short(file, Some(call.callee))),
        )
}
