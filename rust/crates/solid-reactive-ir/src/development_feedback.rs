//! Optional, non-authoritative models for executed development feedback.
//! Exact primitive identity comes from the normal semantic lookup. The host
//! may instrument these spans, but cannot manufacture models from API names.
use serde::{Deserialize, Serialize};
use solid_dialect::{Dialect, Execution};
use solid_facts::{
    FileFacts, ProjectFacts,
    ast::{FunctionFact, ReturnFact},
    core::Span,
};
use std::collections::HashMap;

use crate::indexes::SemanticLookup;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DevelopmentFile {
    pub path: String,
    pub source_sha256: String,
    pub functions: Vec<DevelopmentFunction>,
    pub operations: Vec<DevelopmentOperation>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DevelopmentFunction {
    pub span: Span,
    pub body: Span,
    pub parent: Option<Span>,
    pub asynchronous: bool,
    pub generator: bool,
    /// Present only for a callback directly supplied to an exactly resolved,
    /// tracked primitive whose accessor yields only its computation's value.
    pub derived_origin: Option<Span>,
    /// Candidate relation of this function allocation to its parent's result.
    /// A callback invoked later cannot inherit relevance from lexical nesting.
    pub allocation_relevance: ResultRelevance,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DevelopmentOperation {
    pub span: Span,
    pub function: Span,
    /// Bounded candidate flow, never Promise adoption or reactive intent.
    pub result_relevance: ResultRelevance,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ResultRelevance {
    Open,
    ReturnExpression,
    LocalInitializer,
    DistinctBranch,
}

fn relevance(file: &FileFacts, call: Span, returns: &[&ReturnFact]) -> ResultRelevance {
    // Multiple exits can replace a return (notably finally). Keep them open
    // unless this read controls two explicitly distinct numeric field results.
    if returns.len() > 1 {
        let numeric = returns
            .iter()
            .map(|value| {
                let [property] = value.properties() else {
                    return None;
                };
                let number = file.source_text(property.value)?.parse::<f64>().ok()?;
                number
                    .is_finite()
                    .then_some((property.name.as_str(), number))
            })
            .collect::<Option<Vec<_>>>();
        if numeric.is_some_and(|values| {
            values.iter().all(|value| value.0 == values[0].0)
                && values.iter().any(|value| value.1 != values[0].1)
        }) && returns
            .iter()
            .any(|value| value.control_tests.iter().any(|test| test.contains(call)))
        {
            return ResultRelevance::DistinctBranch;
        }
        return ResultRelevance::Open;
    }
    let Some(value) = returns.first().and_then(|value| value.argument) else {
        return ResultRelevance::Open;
    };
    if value.contains(call) {
        return ResultRelevance::ReturnExpression;
    }
    // Const initialization -> exact lexical references in the returned value.
    // This is a candidate through containers/closures, not a settlement proof.
    if file.ast.bindings.iter().any(|binding| {
        binding.immutable
            && binding.names.len() == 1
            && binding
                .initializer
                .is_some_and(|initializer| initializer.contains(call))
            && file
                .ast
                .reference_declarations
                .iter()
                .any(|(reference, declaration)| {
                    *declaration == binding.names[0].span && value.contains(*reference)
                })
    }) {
        ResultRelevance::LocalInitializer
    } else {
        ResultRelevance::Open
    }
}

fn enclosing(file: &FileFacts, span: Span) -> Option<&FunctionFact> {
    file.ast
        .functions
        .iter()
        .filter(|f| f.body.contains(span))
        .min_by_key(|f| f.body.end - f.body.start)
}

pub(crate) fn models(
    facts: &ProjectFacts,
    lookup: &SemanticLookup<'_>,
    dialect: &dyn Dialect,
) -> Vec<DevelopmentFile> {
    facts
        .files
        .iter()
        .map(|file| {
            let origins = file
                .ast
                .calls
                .iter()
                .filter_map(|call| {
                    let [argument] = call.arguments.as_slice() else {
                        return None;
                    };
                    let primitive = lookup.primitive_at_call(file, call.span)?;
                    (dialect.accessor_yields_only_its_compute(primitive)
                        && dialect.callback_execution_at(primitive, 0, 1)
                            == Some(Execution::Tracked))
                    .then_some((file.ast.peel_ts_sugar_span(argument.span), call.span))
                })
                .collect::<HashMap<_, _>>();
            let mut returns = HashMap::<Span, Vec<&ReturnFact>>::new();
            for function in &file.ast.functions {
                returns
                    .entry(function.span)
                    .or_default()
                    .extend(function.expression_return.iter());
            }
            for value in &file.ast.returns {
                if let Some(function) = enclosing(file, value.span) {
                    returns.entry(function.span).or_default().push(value);
                }
            }
            let functions = file
                .ast
                .functions
                .iter()
                .map(|function| {
                    let parent = file
                        .ast
                        .functions
                        .iter()
                        .filter(|other| {
                            other.span != function.span && other.body.contains(function.span)
                        })
                        .min_by_key(|other| other.body.end - other.body.start)
                        .map(|other| other.span);
                    let derived_origin = origins
                        .get(&function.span)
                        .copied()
                        .filter(|_| function.parameters.is_empty() && !function.rest_parameter);
                    DevelopmentFunction {
                        span: function.span,
                        body: function.body,
                        parent,
                        asynchronous: function.r#async,
                        generator: function.generator,
                        derived_origin,
                        allocation_relevance: parent.map_or(ResultRelevance::Open, |parent| {
                            relevance(
                                file,
                                function.span,
                                returns.get(&parent).map_or(&[], Vec::as_slice),
                            )
                        }),
                    }
                })
                .collect();
            let operations = file
                .ast
                .calls
                .iter()
                .filter(|call| !call.construct)
                .filter_map(|call| {
                    let function = enclosing(file, call.span)?;
                    let result_relevance = relevance(
                        file,
                        call.span,
                        returns.get(&function.span).map_or(&[], Vec::as_slice),
                    );
                    Some(DevelopmentOperation {
                        span: call.span,
                        function: function.span,
                        result_relevance,
                    })
                })
                .collect();
            DevelopmentFile {
                path: file.path.as_str().to_owned(),
                source_sha256: file.source_hash.to_string(),
                functions,
                operations,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use solid_facts::{
        compiler::{COMPILER_FACTS_PROTOCOL, ExecutionMap},
        core::{Generation, SourceHash, SourcePath},
    };
    use std::sync::Arc;

    fn classify(body: &str) -> ResultRelevance {
        let source = format!("function example(read){{{body}}}");
        let hash = SourceHash::of(&source);
        let file = FileFacts {
            generation: Generation::new(1).unwrap(),
            path: SourcePath::new("/example.ts").unwrap(),
            source_hash: hash.clone(),
            source: Arc::from(source.as_str()),
            ast: Arc::new(solid_facts::ast::extract("/example.ts", &source).unwrap()),
            compiler: Arc::new(ExecutionMap {
                compiler_facts_protocol: COMPILER_FACTS_PROTOCOL,
                source_hash: hash,
                semantic_model: Default::default(),
                tracked_regions: vec![],
                untracked_regions: vec![],
                discarded_regions: vec![],
                ownership_regions: vec![],
                callback_roles: vec![],
                jsx_operations: vec![],
            }),
        };
        let call = file
            .ast
            .calls
            .iter()
            .find(|call| file.source_text(call.callee) == Some("read"))
            .unwrap();
        let function = enclosing(&file, call.span).unwrap();
        let returns = file
            .ast
            .returns
            .iter()
            .filter(|value| {
                enclosing(&file, value.span).is_some_and(|owner| owner.span == function.span)
            })
            .collect::<Vec<_>>();
        relevance(&file, call.span, &returns)
    }

    #[test]
    fn returns_and_exact_const_references_supply_bounded_candidates() {
        assert_eq!(
            classify("return {value:read()};"),
            ResultRelevance::ReturnExpression
        );
        assert_eq!(
            classify("const value=read();return {value};"),
            ResultRelevance::LocalInitializer
        );
        assert_eq!(
            classify("const value=read();return {then(resolve){resolve({value});}};"),
            ResultRelevance::LocalInitializer
        );
        assert_eq!(
            classify("const value=read();return {then(resolve){const value=9;resolve({value});}};"),
            ResultRelevance::Open
        );
        assert_eq!(classify("read();return {value:9};"), ResultRelevance::Open);
    }

    #[test]
    fn control_branches_need_distinct_results_and_competing_exits_stay_open() {
        assert_eq!(
            classify("if(read()===1)return {value:1};return {value:2};"),
            ResultRelevance::DistinctBranch
        );
        assert_eq!(
            classify("if(read()===1)return {value:9};return {value:9};"),
            ResultRelevance::Open
        );
        assert_eq!(
            classify("try{return {value:read()};}finally{return {value:9};}"),
            ResultRelevance::Open
        );
        assert_eq!(
            classify("if(flag)return {value:read()};return {value:2};"),
            ResultRelevance::Open
        );
    }
}
