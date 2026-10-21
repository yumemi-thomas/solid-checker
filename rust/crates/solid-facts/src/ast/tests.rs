/// ADR 0195: a straight-line call is one its function runs on every
/// execution that reaches it. Every construct that can skip or repeat a
/// call, or that moves it into another function, keeps it out.
#[test]
fn straight_line_calls_exclude_every_conditional_construct() {
    let source = concat!(
        "declare function s(): number; declare const o: { f(x: number): void } | undefined;\n",
        "declare const c: boolean;\n",
        "async function g() {\n",
        "  s();\n",
        "  const v = s();\n",
        "  if (c) s();\n",
        "  c ? s() : 0;\n",
        "  c && s();\n",
        "  for (const x of [1]) s();\n",
        "  try { s(); } catch {}\n",
        "  switch (v) { case 1: s(); }\n",
        "  o?.f(s());\n",
        "  const nested = () => s();\n",
        "  await 0;\n",
        "  return nested;\n",
        "}\n",
    );
    let facts = super::extract("straight.ts", source).unwrap();
    let line_of = |span: crate::core::Span| source[..span.start as usize].matches('\n').count() + 1;
    let lines = facts
        .straight_line_calls
        .iter()
        .filter(|span| source[span.start as usize..span.end as usize] == *"s()")
        .map(|span| line_of(*span))
        .collect::<Vec<_>>();
    // Line 13 is straight-line in the nested arrow's own flow: the fact is
    // recorded for the innermost function, as `unconditional_awaits` is,
    // and a consumer attributes each span to it.
    assert_eq!(lines, [4, 5, 13], "{lines:?}");
}

/// Facts schema 65: an await dominates the rest of its innermost block when
/// nothing between that block's start and the await can skip it. A `try`
/// block and a branch's block are blocks; `catch`, `finally`, the code after
/// a `try` statement, and an await under a conditional expression are not.
#[test]
fn dominating_awaits_cover_the_rest_of_their_block() {
    let source = concat!(
        "declare const p: Promise<number>; declare const c: boolean;\n",
        "declare function w(n: number): void;\n",
        "async function f() {\n",
        "  try {\n",
        "    const r = await p;\n",
        "    w(r);\n",
        "  } catch {\n",
        "    w(0);\n",
        "  } finally {\n",
        "    w(1);\n",
        "  }\n",
        "  w(2);\n",
        "  if (c) {\n",
        "    await p;\n",
        "    w(3);\n",
        "  }\n",
        "  w(4);\n",
        "  const x = c ? await p : 0;\n",
        "  w(x);\n",
        "  await p;\n",
        "  w(5);\n",
        "}\n",
    );
    let facts = super::extract("dominating.ts", source).unwrap();
    let line_of = |offset: u32| source[..offset as usize].matches('\n').count() + 1;
    let dominated = |line: usize| {
        let start = source
            .match_indices('\n')
            .nth(line - 2)
            .map_or(0, |(index, _)| index + 1);
        let call = start + source[start..].find("w(").unwrap();
        let call = u32::try_from(call).unwrap();
        facts
            .dominating_awaits
            .iter()
            .any(|(_, region)| region.start <= call && call < region.end)
    };
    let awaits = facts
        .dominating_awaits
        .iter()
        .map(|(awaited, _)| line_of(awaited.start))
        .collect::<Vec<_>>();
    assert_eq!(awaits, [5, 14, 20], "{awaits:?}");
    assert!(dominated(6) && dominated(15) && dominated(21));
    assert!(!dominated(8) && !dominated(10) && !dominated(12) && !dominated(17) && !dominated(19));
}

/// Facts schema 66: a `try` whose first statement awaits records the await,
/// its operand and its `catch`/`finally` blocks; a `try` that starts with
/// anything else records nothing.
#[test]
fn try_head_awaits_record_their_handlers() {
    let source = concat!(
        "declare function f(): Promise<number>; declare function g(): void;\n",
        "async function h() {\n",
        "  try { const v = await f(); g(); } catch { g(); } finally { g(); }\n",
        "  try { g(); await f(); } catch { g(); }\n",
        "  try { return await f(); } finally { g(); }\n",
        "  try { await k(1, 'a', null, true, 2n, NaN, 1 + 2, n); } catch { g(); }\n",
        "}\n",
        "declare function k(...values: unknown[]): Promise<void>; declare const n: number;\n",
    );
    let facts = super::extract("try-head.ts", source).unwrap();
    let operands = facts
        .try_head_awaits
        .iter()
        .map(|head| {
            (
                &source[head.operand.start as usize..head.operand.end as usize],
                head.regions.len(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(operands.len(), 3, "{operands:?}");
    assert_eq!(operands[..2], [("f()", 2), ("f()", 1)], "{operands:?}");
    // Literal nodes only: not `NaN` (an identifier), `1 + 2` or `n`.
    let literals = facts.try_head_awaits[2]
        .literal_arguments
        .iter()
        .map(|span| &source[span.start as usize..span.end as usize])
        .collect::<Vec<_>>();
    assert_eq!(literals, ["1", "'a'", "null", "true", "2n"], "{literals:?}");
}

/// ADR 0354 (facts schema 73): an `if` records its `else` arm, a loop its
/// repeated part and a destructuring pattern the value it is evaluated after.
/// A `for` initializer and a `for…of` iterable run once, before the first
/// iteration, so neither is repeated; the target of a `for…of` is.
#[test]
fn else_arms_loop_repeats_and_pattern_values_are_recorded() {
    let source = concat!(
        "declare const c: boolean; declare const xs: number[];\n",
        "declare const q: Promise<{ a?: number }>;\n",
        "declare function s(): number;\n",
        "async function f() {\n",
        "  if (c) { await s(); } else { s(); }\n",
        "  for (let i = s(); i < s(); i += s()) { s(); }\n",
        "  for (const x of xs) { s(); }\n",
        "  while (c) { s(); }\n",
        "  const { a = s() } = await q;\n",
        "  ({ a = s() } = await q);\n",
        "}\n",
    );
    let facts = super::extract("regions.ts", source).unwrap();
    let text = |span: crate::core::Span| &source[span.start as usize..span.end as usize];
    let else_arms = facts
        .if_regions
        .iter()
        .filter_map(|region| region.alternate)
        .map(text)
        .collect::<Vec<_>>();
    assert_eq!(else_arms, ["{ s(); }"], "{else_arms:?}");
    let repeats = facts
        .loop_repeats
        .iter()
        .map(|(looped, repeated)| (text(*looped), text(*repeated)))
        .collect::<Vec<_>>();
    assert_eq!(
        repeats,
        [
            (
                "for (let i = s(); i < s(); i += s()) { s(); }",
                "; i < s(); i += s()) { s(); }"
            ),
            ("for (const x of xs) { s(); }", "const x of "),
            ("for (const x of xs) { s(); }", ") { s(); }"),
            ("while (c) { s(); }", "while (c) { s(); }"),
        ],
        "{repeats:?}"
    );
    let patterns = facts
        .pattern_values
        .iter()
        .map(|(pattern, value)| (text(*pattern), text(*value)))
        .collect::<Vec<_>>();
    assert_eq!(
        patterns,
        [("{ a = s() }", "await q"), ("{ a = s() }", "await q"),],
        "{patterns:?}"
    );
}

/// ADR 0320 (facts schema 64): an await is unconditional only when no
/// construct that can skip it lies between the function entry and it:
/// logical assignments, optional chains, destructuring defaults and
/// labelled statements (whose body a `break label` leaves) join the
/// conditionals, loops, switches and `try` already counted.
#[test]
fn unconditional_awaits_exclude_every_skippable_construct() {
    let source = concat!(
        "declare const p: Promise<number>; declare const c: boolean;\n",
        "declare const g: ((n: number) => void) | undefined;\n",
        "declare const t: Record<number, number> | undefined;\n",
        "async function f(o: { a?: number }) {\n",
        "  let x: number | undefined;\n",
        "  x ??= await p;\n",
        "  x ||= await p;\n",
        "  x &&= await p;\n",
        "  g?.(await p);\n",
        "  void t?.[await p];\n",
        "  skip: { if (c) break skip; await p; }\n",
        "  outer: { { if (c) break outer; } await p; }\n",
        "  const { a = await p } = o;\n",
        "  ({ a: x = await p } = o);\n",
        "  void a;\n",
        "  await p;\n",
        "  const y = await p;\n",
        "  return y;\n",
        "}\n",
    );
    let facts = super::extract("awaits.ts", source).unwrap();
    let line_of = |span: crate::core::Span| source[..span.start as usize].matches('\n').count() + 1;
    let lines = facts
        .unconditional_awaits
        .iter()
        .map(|span| line_of(*span))
        .collect::<Vec<_>>();
    assert_eq!(lines, [16, 17], "{lines:?}");
    let calls = super::extract(
            "calls.ts",
            "declare function s(): number;\nlet x: number | undefined;\nx ??= s();\nl: { s(); }\nconst { a = s() } = {} as { a?: number };\ns();\n",
        )
        .unwrap();
    assert_eq!(
        calls.straight_line_calls.len(),
        1,
        "{:?}",
        calls.straight_line_calls
    );
}

/// ADR 0276: a completion site is straight-line in its own function and
/// no earlier `return`, `break` or `continue` of that function may skip
/// it. A jump in a nested function does not count.
/// A `try` block runs to its end under the normal-completion premise; its
/// `catch` and `finally` stay conditional, and a `try` inside a branch is
/// still conditional.
#[test]
fn a_try_block_is_on_every_completion() {
    let source = concat!(
        "declare function s(): number; declare const c: boolean;\n",
        "async function g() {\n",
        "  try {\n",
        "    await Promise.resolve();\n",
        "    s();\n",
        "  } catch {\n",
        "    s();\n",
        "  } finally {\n",
        "    s();\n",
        "  }\n",
        "  if (c) { try { s(); } catch {} }\n",
        "  s();\n",
        "}\n",
    );
    let facts = super::extract("try.ts", source).unwrap();
    let lines = facts
        .completion_sites
        .iter()
        .filter(|span| &source[span.start as usize..span.end as usize] == "s()")
        .map(|span| source[..span.start as usize].matches('\n').count() + 1)
        .collect::<Vec<_>>();
    assert_eq!(lines, [5, 12], "{lines:?}");
}

#[test]
fn completion_sites_stop_at_a_possible_exit() {
    let source = concat!(
        "declare function s(): number; declare const c: boolean;\n",
        "declare const o: { p: number };\n",
        "function g() {\n",
        "  s();\n",
        "  o.p;\n",
        "  const inner = () => { if (c) return; s(); };\n",
        "  s();\n",
        "  if (c) return;\n",
        "  s();\n",
        "  o.p;\n",
        "}\n",
        "function h() {\n",
        "  for (;;) { if (c) break; }\n",
        "  s();\n",
        "  return s();\n",
        "}\n",
        "function t(o: { v?: number }) {\n",
        "  o.v ??= s();\n",
        "  if (!c) throw new Error();\n",
        "  s();\n",
        "}\n",
        "declare const m: { f(): { g(x: number): number } | undefined };\n",
        "function k() {\n",
        "  m.f()?.g(s());\n",
        "}\n",
    );
    let facts = super::extract("completion.ts", source).unwrap();
    let line_of = |span: crate::core::Span| source[..span.start as usize].matches('\n').count() + 1;
    let lines = |text: &str| {
        facts
            .completion_sites
            .iter()
            .filter(|span| source[span.start as usize..span.end as usize] == *text)
            .map(|span| line_of(*span))
            .collect::<Vec<_>>()
    };
    // Line 6's arrow returns only after its own guard, so its `s()` is
    // not a site; line 7 follows the arrow and its jump is not `g`'s.
    // A `break` is read as a possible exit too, which only proves less.
    assert_eq!(lines("s()"), [4, 7], "{:?}", lines("s()"));
    assert_eq!(lines("o.p"), [5], "{:?}", lines("o.p"));
    // Before a chain's first `?.` everything runs; after it, nothing is
    // a site, the argument included.
    // A logical assignment's right side and a site after a thrown guard
    // are not sites (lines 18 and 20).
    assert_eq!(lines("m.f()"), [24], "{:?}", lines("m.f()"));
    assert!(lines("s()").iter().all(|line| ![18, 20, 24].contains(line)));
    assert!(lines("m.f()?.g(s())").is_empty());
}

#[test]
fn structural_binding_slots_require_direct_identifiers() {
    let facts = super::extract("slots.ts", "const [first, {nested}, fallback = 0, ...rest] = make(); const {value: direct, nested: {inner}, optional = 0, [key]: computed} = other(); delete (other as any).value;").unwrap();
    let array = &facts.bindings[0];
    assert!(array.array_slots[0].is_some());
    assert!(array.array_slots[1..].iter().all(Option::is_none));
    let object = facts
        .bindings
        .iter()
        .find(|binding| binding.shape == super::BindingShape::Object)
        .unwrap();
    assert_eq!(object.object_slots.len(), 1);
    assert_eq!(object.object_slots[0].property, "value");
    assert_eq!(facts.deleted_targets.len(), 1);
}
use super::*;

/// `result_discarded` proves the result reaches nothing, and proves it in
/// one direction only.
///
/// The consumer this exists for reads `!result_discarded` as "the result
/// may be used", so every form the classifier does not recognise has to
/// answer `false`. The negative half of this test is therefore the
/// load-bearing half: `await`, `void`, a nested call and a concise arrow
/// body all keep the obligation rather than shedding it.
#[test]
fn a_discarded_call_result_is_stated_only_where_the_call_is_the_whole_statement() {
    let source = concat!(
        "declare function f(x?: unknown): unknown;\n",
        "async function cases(flag: boolean) {\n",
        "  f();\n",               // 0 discarded: the call is the statement
        "  const bound = f();\n", // 1 used: bound to a name
        "  await f();\n",         // 2 used: the await consumes it
        "  void f();\n",          // 3 used: the unary consumes it
        "  f(f());\n",            // 4 discarded (outer), 5 used (inner)
        "  if (flag) { f(); }\n", // 6 discarded inside a block
        "  return bound;\n",
        "}\n",
        "const concise = () => f();\n", // 7 used: a concise body returns it
        "new Date();\n",                // 8 discarded: `new` as a statement
    );
    let facts = extract("/project/discarded.ts", source).unwrap();
    let discarded: Vec<bool> = facts
        .calls
        .iter()
        .map(|call| call.result_discarded)
        .collect();
    assert_eq!(
        discarded,
        vec![true, false, false, false, true, false, true, false, true],
        "calls: {:?}",
        facts
            .calls
            .iter()
            .map(|call| &source[call.span.start as usize..call.span.end as usize])
            .collect::<Vec<_>>()
    );
}

#[test]
fn parameter_initializers_survive_function_and_arrow_normalization() {
    let source = "function direct(value: number) { return value; } function defaulted(value = 1) { return value; } const arrow = (value = 2) => value;";
    let facts = extract("/project/parameters.ts", source).unwrap();
    assert_eq!(facts.functions.len(), 3);
    let initializers = facts
        .functions
        .iter()
        .map(|function| {
            function.parameters[0]
                .initializer
                .map(|span| &source[span.start as usize..span.end as usize])
        })
        .collect::<Vec<_>>();
    assert_eq!(initializers, vec![None, Some("1"), Some("2")]);
}

/// String directives: the module prologue lands in `module_directives`,
/// each function's prologue lands on its `FunctionFact`, and a string
/// literal after the first real statement is not a directive.
#[test]
fn extracts_module_and_function_directive_prologues() {
    let source = r#""use server";
"use strict";
export async function addTodo(title: string) {
  "use server";
  return title;
}
export const wrapped = async () => {
  "use server";
  return 1;
};
const notAPrologue = 1;
"not a directive";
export const short = async () => 2;
"#;
    let facts = extract("/project/api.ts", source).unwrap();
    assert_eq!(
        facts
            .module_directives
            .iter()
            .map(|directive| directive.value.as_str())
            .collect::<Vec<_>>(),
        ["use server", "use strict"]
    );
    let declaration = facts
        .functions
        .iter()
        .find(|function| function.kind == FunctionKind::Declaration)
        .unwrap();
    assert!(declaration.has_directive("use server"));
    assert!(!declaration.has_directive("use strict"));
    let arrows: Vec<_> = facts
        .functions
        .iter()
        .filter(|function| function.kind == FunctionKind::Arrow)
        .collect();
    assert_eq!(arrows.len(), 2);
    assert!(arrows.iter().any(|arrow| arrow.has_directive("use server")));
    // The expression-bodied arrow has no prologue at all.
    assert!(
        arrows
            .iter()
            .any(|arrow| arrow.expression_body && arrow.directives.is_empty())
    );
    // The trailing string literal sits after a statement and is not a
    // module directive.
    assert_eq!(facts.module_directives.len(), 2);
}

#[test]
fn type_names_are_erased_but_import_aliases_are_not() {
    let source = "import * as X from \"m\"; type P = X.Props<string>; \
            class C implements X.Shape {} interface I extends X.Base {} \
            import A = X.Value; const v: X.Props<number> = X.make();";
    let facts = extract("/project/types.ts", source).unwrap();
    let texts: Vec<&str> = facts
        .type_names
        .iter()
        .map(|name| &source[name.start as usize..name.end as usize])
        .collect();
    assert_eq!(texts, ["X.Props", "X.Shape", "X.Base", "X.Props"]);
    let runtime = source.find("X.make").unwrap() as u32;
    assert!(
        facts
            .type_names
            .iter()
            .all(|name| !name.contains(Span::new(runtime, runtime + 1)))
    );
}

#[test]
fn type_queries_are_erased_but_runtime_typeof_is_not() {
    let source = "function f() {} type T = ReturnType<typeof f>; const value = typeof f;";
    let facts = extract("/project/types.ts", source).unwrap();
    assert_eq!(facts.type_queries.len(), 1);
    let query = facts.type_queries[0];
    assert_eq!(
        &source[query.start as usize..query.end as usize],
        "typeof f"
    );
    let runtime = source.rfind("typeof f").unwrap();
    let runtime = Span::new(runtime as u32, (runtime + "typeof f".len()) as u32);
    assert!(!query.contains(runtime));
}

#[test]
fn a_load_records_its_runtime_literal_not_a_type_argument() {
    let source = r#"const ns = require<typeof import("x")>("x"); const m = import("./m");"#;
    let facts = extract("/project/loads.ts", source).unwrap();
    let [required, imported] = facts.module_loads.as_slice() else {
        panic!("two loads: {:?}", facts.module_loads);
    };
    let literal = required.specifier_span.expect("require literal");
    assert_eq!(literal.start as usize, source.rfind(r#""x""#).unwrap());
    assert_eq!(
        &source[literal.start as usize..literal.end as usize],
        r#""x""#
    );
    let literal = imported.specifier_span.expect("import literal");
    assert_eq!(
        &source[literal.start as usize..literal.end as usize],
        r#""./m""#
    );
}

#[test]
fn tagged_template_tags_are_recorded() {
    let source = "const o = { t() { return 1; } }; o.t`x${1}`; `plain${2}`;";
    let facts = extract("/project/tags.ts", source).unwrap();
    let [tag] = facts.tagged_template_tags.as_slice() else {
        panic!("one tag: {:?}", facts.tagged_template_tags);
    };
    assert_eq!(&source[tag.start as usize..tag.end as usize], "o.t");
}

fn text(source: &str, span: Span) -> &str {
    &source[span.start as usize..span.end as usize]
}

#[test]
fn tagged_templates_record_their_substitutions_as_tag_arguments() {
    let source = "declare function tag(s: TemplateStringsArray, ...v: unknown[]): void;\n\
                      declare const value: () => string;\n\
                      tag`a${value}b${1 + 1}`; tag`none`; `plain${value}`;";
    let facts = extract("/project/tags.ts", source).unwrap();
    let [first, second] = facts.tagged_templates.as_slice() else {
        panic!("two tagged templates: {:?}", facts.tagged_templates);
    };
    assert_eq!(text(source, first.tag), "tag");
    assert_eq!(
        first
            .substitutions
            .iter()
            .map(|substitution| text(source, *substitution))
            .collect::<Vec<_>>(),
        ["value", "1 + 1"]
    );
    assert!(second.substitutions.is_empty());
    // The untagged literal stays in its own table only.
    assert_eq!(facts.template_literals.len(), 1);
}

#[test]
fn array_literals_record_values_spreads_and_holes_in_order() {
    let source = "declare const value: () => string; declare const xs: unknown[];\n\
                      const a = [value, , ...xs, value as unknown];";
    let facts = extract("/project/arrays.ts", source).unwrap();
    let [array] = facts.array_literal_elements.as_slice() else {
        panic!("one array: {:?}", facts.array_literal_elements);
    };
    assert_eq!(
        array
            .elements
            .iter()
            .map(|element| (element.kind, text(source, element.span)))
            .collect::<Vec<_>>(),
        [
            (ArrayElementKind::Value, "value"),
            (ArrayElementKind::Hole, ","),
            (ArrayElementKind::Spread, "xs"),
            (ArrayElementKind::Value, "value as unknown"),
        ]
    );
    assert_eq!(facts.array_literals, [array.span]);
}

#[test]
fn pattern_elements_record_slots_targets_defaults_and_rests() {
    let source = "declare const value: () => string; declare const o: any; declare const k: string;\n\
                      function helper(v: () => string, { text = v(), 'q': [first = value, ...tail], [k]: c, 0: z, ...others } = {}) {}\n\
                      let a, b, r; ({ a = value, b: [r = 1] } = o); [a, ...b] = o;";
    let facts = extract("/project/patterns.ts", source).unwrap();
    let rows = facts
        .pattern_elements
        .iter()
        .map(|element| {
            let slot = match &element.slot {
                PatternSlot::Index(index) => format!("#{index}"),
                PatternSlot::Key(key) => format!(".{key}"),
                PatternSlot::ComputedKey => "[*]".into(),
                PatternSlot::Rest => "...".into(),
            };
            format!(
                "{}{slot}->{}{}{}",
                if element.assignment { "=" } else { "" },
                text(source, element.target),
                element
                    .default
                    .map(|default| format!("={}", text(source, default)))
                    .unwrap_or_default(),
                element
                    .key
                    .map(|key| format!(" key:{}", text(source, key)))
                    .unwrap_or_default(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        rows,
        [
            ".text->text=v() key:text",
            ".q->[first = value, ...tail] key:'q'",
            "[*]->c key:k",
            ".0->z key:0",
            "...->others",
            "#0->first=value",
            "...->tail",
            "=.a->a=value key:a",
            "=.b->[r = 1] key:b",
            "=#0->r=1",
            "=#0->a",
            "=...->b",
        ]
    );
    // The parameter's own default stays its binding's initializer.
    let helper = &facts.functions[0];
    assert_eq!(
        helper.parameters[1]
            .initializer
            .map(|span| text(source, span)),
        Some("{}")
    );
}

#[test]
fn await_operands_and_jsx_child_expressions_are_recorded() {
    let source = "declare const value: () => string;\n\
                      async function f() { await value; await (value as unknown); }\n\
                      const view = <p>{value}{/* note */}{value()}<>{ value }</></p>;";
    let facts = extract("/project/view.tsx", source).unwrap();
    assert_eq!(
        facts
            .await_operands
            .iter()
            .map(|operand| text(source, *operand))
            .collect::<Vec<_>>(),
        ["value", "value as unknown"]
    );
    assert_eq!(
        facts
            .jsx_child_expressions
            .iter()
            .map(|child| text(source, *child))
            .collect::<Vec<_>>(),
        ["value", "value()", "value"]
    );
}

#[test]
fn disposal_initializers_are_recorded() {
    let source = "declare const value: Disposable;\n\
                      async function f() { using held = value; await using later = (value as any); const plain = value; }";
    let facts = extract("/project/dispose.ts", source).unwrap();
    assert_eq!(
        facts
            .disposal_initializers
            .iter()
            .map(|initializer| text(source, *initializer))
            .collect::<Vec<_>>(),
        ["value", "value as any"]
    );
}

#[test]
fn shadow_event_tables_default_without_attesting_completeness() {
    let source = "async function f({ x = 1, ...rest } = {}) { using held = rest; await x; return tag`a${[x, ...rest]}`; } const view = <><p>{f()}{/* comment */}</p></>;";
    let facts = extract("/project/control.tsx", source).unwrap();
    let fields = [
        "taggedTemplates",
        "arrayLiteralElements",
        "patternElements",
        "awaitOperands",
        "jsxChildExpressions",
        "disposalInitializers",
    ];
    let mut old = serde_json::to_value(&facts).unwrap();
    for field in fields {
        assert!(old.as_object_mut().unwrap().remove(field).is_some());
    }
    let legacy: AstFacts = serde_json::from_value(old.clone()).unwrap();
    assert!(legacy.tagged_templates.is_empty());
    assert!(legacy.array_literal_elements.is_empty());
    assert!(legacy.pattern_elements.is_empty());
    assert!(legacy.await_operands.is_empty());
    assert!(legacy.jsx_child_expressions.is_empty());
    assert!(legacy.disposal_initializers.is_empty());
    assert_eq!(serde_json::to_value(&legacy).unwrap(), old);
    let roundtrip: AstFacts =
        serde_json::from_value(serde_json::to_value(&facts).unwrap()).unwrap();
    assert_eq!(roundtrip, facts);
    let changed = extract(
        "/project/control.tsx",
        &source.replace("await x", "await rest"),
    )
    .unwrap();
    assert_ne!(changed.await_operands, facts.await_operands);
}

#[test]
fn object_get_shapes_distinguish_getters_from_function_values() {
    let facts = extract(
        "/project/gets.ts",
        "const o = { get count() { return n(); }, fn: () => n(), method() { return n(); } }; ",
    )
    .unwrap();
    let shape = &facts.object_get_shapes[0];
    assert!(shape.closed);
    assert_eq!(shape.properties.len(), 3);
    assert!(shape.properties[0].getter.is_some());
    assert!(shape.properties[0].entry_call.is_some());
    assert!(shape.properties[1].getter.is_none());
    assert!(shape.properties[2].getter.is_none());
    assert!(!facts.object_properties[0].method);
    assert!(!facts.object_properties[1].method);
    assert!(facts.object_properties[2].method);
    assert!(!facts.object_properties[2].data);
    for source in [
        "const o = { get x() { before(); return n(); } };",
        "const o = { get x() { const y = unknown; return n(); } };",
        "const o = { get x() { return n(before()); } };",
        "const o = { get x() { if (ready) return n(); return 0; } };",
        "const o = { get x() { return () => n(); } };",
    ] {
        // `n()` is never the getter's guaranteed entry: something else
        // is evaluated first, or it is conditional or deferred. An
        // earlier no-argument statement call (`before()`) may itself be
        // the entry call, which proves nothing about `n()`.
        let entry = extract("/project/gets.ts", source)
            .unwrap()
            .object_get_shapes[0]
            .properties[0]
            .entry_call;
        assert!(
            entry.is_none_or(|span| &source[span.start as usize..span.end as usize] != "n()"),
            "{source}"
        );
    }
    for source in [
        "const o = { ...other, get count() { return n(); } };",
        "const o = { [key]: 1 };",
        "const o = { __proto__: other };",
        "const o = { get x() { return n(); }, x: 1 };",
        "const o = { set x(v) {} };",
        "const o = { '0': 1, get x() { return n(); } };",
    ] {
        assert!(
            !extract("/project/gets.ts", source)
                .unwrap()
                .object_get_shapes[0]
                .closed,
            "{source}"
        );
    }
}

#[test]
fn runtime_let_facts_do_not_turn_ambient_bindings_into_undefined() {
    let source = "let local; declare let ambient: object; declare const opaque: object; var legacy; declare namespace External { let nested: object; } namespace Runtime { export let member: object; }";
    let facts = extract("ambient.ts", source).unwrap();
    for binding in &facts.bindings {
        let name = binding.names[0].span;
        assert_eq!(
            binding.runtime_let,
            &source[name.start as usize..name.end as usize] == "local"
        );
    }
    let definition = extract(
        "ambient.d.ts",
        "let implicit: object; declare let explicit: object;",
    )
    .unwrap();
    assert!(
        definition
            .bindings
            .iter()
            .all(|binding| !binding.runtime_let)
    );
}

#[test]
fn switch_case_facts_mark_only_declarators_written_directly_in_a_clause() {
    let source = "switch (m) { /* } */ case 0: const direct = 1, second = 2; break; case 1: { const blocked = 3; } default: let tail = 4; } const outside = 5; const f = () => { switch (m) { case 0: const nested = 6; } const inner = 7; };";
    let facts = extract("switch.ts", source).unwrap();
    let marked = facts
        .bindings
        .iter()
        .filter(|binding| binding.in_switch_case)
        .map(|binding| {
            let name = binding.names[0].span;
            &source[name.start as usize..name.end as usize]
        })
        .collect::<Vec<_>>();
    assert_eq!(marked, ["direct", "second", "tail", "nested"]);
}

#[test]
fn parameter_lists_are_inert_only_when_no_default_can_run_user_code() {
    let source = "const a = (x = 0, y = {}, z = [], w = () => 1, u = undefined, s = `t`) => 1; const b = (x = f()) => 1; const c = ({ k = g() } = {}) => 1; const d = ([k = 1], { m } = {}) => 1; const e = ({ [h()]: v }) => 1; const f2 = (x = [h()]) => 1; const g2 = (x = {p: 1}) => 1; const k = (x = `${h()}`) => 1; const l = (x = new Later()) => 1;";
    let facts = extract("params.ts", source).unwrap();
    // Source order: `a` then its nested `w` arrow, then `b` .. `l`.
    let mut functions = facts.functions.iter().collect::<Vec<_>>();
    functions.sort_by_key(|function| function.span.start);
    let by_start = functions
        .into_iter()
        .map(|function| function.parameters_inert)
        .collect::<Vec<_>>();
    assert_eq!(
        by_start,
        [
            true, true, false, false, true, false, false, false, false, false
        ]
    );
}

#[test]
fn require_wrappers_are_loads_and_other_mentions_are_unmodeled() {
    for source in [
        "require('./a')",
        "(require as any)('./a')",
        "require!('./a')",
        "(require satisfies Function)('./a')",
        "(<any>require)('./a')",
    ] {
        let facts = extract("direct.ts", source).unwrap();
        assert_eq!(facts.module_loads.len(), 1, "{source}");
        assert!(facts.unmodeled_require_references.is_empty(), "{source}");
    }
    for source in [
        "(0, require)('./a')",
        "const r = require; r('./a')",
        "typeof require",
        "require.resolve('./a')",
        "new require('./a')",
    ] {
        let facts = extract("other.ts", source).unwrap();
        assert_eq!(facts.unmodeled_require_references.len(), 1, "{source}");
    }
}

#[test]
fn import_globs_are_in_place_unless_provably_lazy() {
    let lazy = [
        "import.meta.glob('./a')",
        "import.meta.glob('./a', { eager: false })",
        "import.meta.glob(['./a'], { import: 'x', 'query': '?raw' })",
        "(import.meta).glob('./a', { eager: false })",
    ];
    for source in lazy {
        let facts = extract("lazy.ts", source).unwrap();
        assert!(facts.in_place_import_globs.is_empty(), "{source}");
    }
    let in_place = [
        "import.meta.glob('./a', { eager: true })",
        "import.meta.glob('./a', { eager })",
        "import.meta.glob('./a', { ...options })",
        "import.meta.glob('./a', { ['eager']: false })",
        "import.meta.glob('./a', options)",
        "import.meta.glob(...args)",
        "import.meta.globEager('./a')",
        "const g = import.meta.glob; g('./a')",
        "import.meta['glob']('./a')",
        "import.meta?.glob('./a')",
    ];
    for source in in_place {
        let facts = extract("eager.ts", source).unwrap();
        assert_eq!(facts.in_place_import_globs.len(), 1, "{source}");
    }
    let env = extract("env.ts", "import.meta.env.MODE; import.meta['env']").unwrap();
    assert!(env.in_place_import_globs.is_empty());
}

#[test]
fn literal_keys_peel_wrappers_without_closing_computed_object_shapes() {
    for key in [
        "'state' as const",
        "'state' satisfies string",
        "'state'!",
        "(('state'))",
        "<string>'state'",
        "`st\\u0061te`",
        "(`state` as const)!",
    ] {
        let source = format!("other[{key}] = read; const o = {{ [{key}]: read }};");
        let facts = extract("keys.ts", &source).unwrap();
        let [member] = facts.literal_computed_members.as_slice() else {
            panic!("one literal member key: {key}");
        };
        assert_eq!(member.key, "state", "{key}");
        assert_eq!(member.span, facts.members[0].span, "{key}");
        let [shape] = facts.object_get_shapes.as_slice() else {
            panic!("one object shape: {key}");
        };
        assert!(!shape.closed, "computed shapes must stay open: {key}");
        assert_eq!(shape.properties.len(), 1, "{key}");
        assert_eq!(shape.properties[0].key, "state", "{key}");
    }
    for key in ["key as string", "`st${key}te`", "'\\ud800'", "`\\ud800`"] {
        let source = format!("other[{key}] = read; const o = {{ [{key}]: read }};");
        let facts = extract("keys.ts", &source).unwrap();
        assert!(facts.literal_computed_members.is_empty(), "{key}");
        assert!(facts.object_get_shapes[0].properties.is_empty(), "{key}");
        assert!(!facts.object_get_shapes[0].closed, "{key}");
    }
}

#[test]
fn extracts_tsx_structure_without_text_patterns() {
    let source = r#"
import { createSignal as signal, createEffect } from "solid-js";
export const [count, setCount] = signal(0);
export async function App(props: { title: string }) {
  await ready();
  createEffect(() => count(), value => console.log(value));
  return <button onClick={() => setCount(count() + 1)}>{props.title}</button>;
}
"#;
    let facts = extract("/project/App.tsx", source).unwrap();
    assert_eq!(facts.schema, AST_FACTS_SCHEMA);
    assert_eq!(facts.imports[0].module, "solid-js");
    assert!(
        facts
            .calls
            .iter()
            .any(|call| call.static_callee(source) == Some("signal"))
    );
    assert!(
        facts
            .calls
            .iter()
            .any(|call| call.static_callee(source) == Some("createEffect"))
    );
    assert!(facts.bindings.iter().any(|binding| {
        binding.shape == BindingShape::Array
            && binding
                .names
                .iter()
                .filter_map(|name| source.get(name.span.start as usize..name.span.end as usize))
                .collect::<Vec<_>>()
                == ["count", "setCount"]
    }));
    assert!(facts.functions.iter().any(|function| {
        function.name.as_ref().is_some_and(|name| {
            source.get(name.span.start as usize..name.span.end as usize) == Some("App")
        }) && function.r#async
    }));
    assert_eq!(facts.awaits.len(), 1);
    assert_eq!(facts.returns.len(), 1);
    assert_eq!(facts.jsx_elements.len(), 1);
    assert_eq!(
        source.get(
            facts.jsx_elements[0].name.span.start as usize
                ..facts.jsx_elements[0].name.span.end as usize
        ),
        Some("button")
    );
    assert!(facts.members.iter().any(|member| {
        source.get(member.property.start as usize..member.property.end as usize) == Some("title")
    }));
}

#[test]
fn unwraps_typescript_assertions_around_call_initializers() {
    let source =
        "const [state, setState] = createSignal(0) as unknown as [() => number, Function];";
    let facts = extract("state.ts", source).unwrap();
    assert_eq!(facts.bindings.len(), 1);
    assert!(facts.bindings[0].call_initializer.is_some());
}

#[test]
fn nullish_tests_observe_exact_operands_without_value_escape() {
    let source = r#"
declare const value: object;
value === undefined; null !== (value as object); value == null;
value ?? (() => { throw new Error("missing"); })();
const alias = value ?? {};
function shadow(undefined: object) { return value === undefined; }
value === {}; value > null;
"#;
    let facts = extract("tests.ts", source).unwrap();
    let operands = facts
        .nullish_test_operands
        .iter()
        .map(|span| &source[span.start as usize..span.end as usize])
        .collect::<Vec<_>>();
    assert_eq!(operands, ["value", "value", "value", "value"]);
    let escaped = source.find("const alias = value").unwrap() + "const alias = ".len();
    assert!(
        !facts
            .nullish_test_operands
            .iter()
            .any(|span| span.start as usize == escaped)
    );
}

#[test]
fn peels_only_transparent_typescript_wrappers_and_names_methods() {
    let source = r#"
declare const fn: () => number;
const wrapped = (((fn) as (() => number)) satisfies (() => number))!;
const object = { helper() { return fn(); } };
class Box { helper() { return fn(); } }
"#;
    let facts = extract("wrappers.ts", source).unwrap();
    let initializer = facts.bindings.iter().find_map(|binding| {
        (source.get(binding.pattern.start as usize..binding.pattern.end as usize)
            == Some("wrapped"))
        .then_some(binding.initializer)
        .flatten()
    });
    let initializer = initializer.expect("wrapped initializer");
    let peeled = facts.peel_ts_sugar_span(initializer);
    assert_eq!(&source[peeled.start as usize..peeled.end as usize], "fn");
    assert!(facts.transparent_wrappers.len() >= 3);
    let methods = facts
        .functions
        .iter()
        .filter_map(|function| function.method_name.as_ref())
        .map(|name| &source[name.span.start as usize..name.span.end as usize])
        .collect::<Vec<_>>();
    assert_eq!(methods, ["helper", "helper"]);
}

#[test]
fn object_binding_keys_distinguish_plain_identifiers_from_literal_spellings() {
    let facts = extract(
        "bindings.ts",
        r#"const { read: plain, "read": quoted, r\u0065ad: escaped, ["read"]: computed } = value;"#,
    )
    .unwrap();
    let slots = &facts.bindings[0].object_slots;
    assert_eq!(slots.len(), 3, "computed keys remain absent");
    assert!(slots[0].plain_identifier_key);
    assert!(!slots[1].plain_identifier_key);
    assert!(!slots[2].plain_identifier_key);
}

#[test]
fn certifies_static_callee_spans_without_retaining_their_text() {
    let source = "solid.createEffect(); (factory())();";
    let facts = extract("calls.ts", source).unwrap();
    let callees = facts
        .calls
        .iter()
        .map(|call| call.static_callee(source))
        .collect::<Vec<_>>();

    assert!(callees.contains(&Some("solid.createEffect")));
    assert!(callees.contains(&Some("factory")));
    assert!(callees.contains(&None));
}

/// `declares_class_at` is the span-addressing rule a package export's `kind`
/// decision consults before it reads any type fact: at these spans the
/// compiler answers about the class's *instance*, not about the constructor
/// the export publishes. Every span that must answer, and the spans that
/// must not.
#[test]
fn addresses_class_declaration_names_and_anonymous_class_nodes() {
    let source = r#"
class Declared {}
const Named = class Inner {};
export default class {}
"#;
    let facts = extract("classes.ts", source).unwrap();
    let span_of = |text: &str| {
        let start = u32::try_from(source.find(text).expect("text occurs in source")).unwrap();
        Span::new(start, start + u32::try_from(text.len()).unwrap())
    };

    // A class declaration's name, and a named class expression's name.
    assert!(facts.declares_class_at(span_of("Declared")));
    assert!(facts.declares_class_at(span_of("Inner")));
    // An anonymous class has no name span, so an `export default class {}`
    // records the class node itself — the span the decision is asked at.
    assert!(facts.declares_class_at(span_of("class {}")));
    // The declarator name is the binding, not the class: its type there is
    // the constructor, so the facts answer that span correctly and this
    // rule must not claim it.
    assert!(!facts.declares_class_at(span_of("Named")));
    // Nor does anything merely contained in a class.
    assert!(!facts.declares_class_at(span_of("{}")));
}

/// A `namespace` body binds members of the namespace object, so nothing
/// inside one is part of this module's export surface — neither the nested
/// `export` statements nor their declarators.
#[test]
fn namespace_members_are_not_module_level_exports() {
    let source = r#"
export namespace Config { export const inner = 1; }
namespace Unexported { export const hidden = 2; }
declare global { const ambient: number; }
export const real = 3;
"#;
    let facts = extract("namespaces.ts", source).unwrap();
    let text = |span: Span| &source[span.start as usize..span.end as usize];
    let span_of = |name: &str| {
        let start = u32::try_from(source.find(name).expect("name occurs in source")).unwrap();
        Span::new(start, start + u32::try_from(name.len()).unwrap())
    };

    // Three blocks: the two namespaces and `declare global`.
    assert_eq!(facts.module_blocks.len(), 3);
    assert_eq!(facts.namespace_declarations.len(), 2);
    assert!(facts.declares_namespace_at(span_of("Config")));
    assert!(facts.declares_namespace_at(span_of("Unexported")));
    assert!(!facts.declares_namespace_at(span_of("ambient")));
    assert!(!facts.declares_namespace_at(span_of("real")));
    let module_level = facts
        .module_level_exports()
        .flat_map(|export| {
            export
                .declarations
                .iter()
                .chain(&export.specifiers)
                .map(|specifier| specifier.exported.to_string())
                .chain(
                    facts
                        .exported_bindings(export)
                        .flat_map(|binding| &binding.names)
                        .map(|name| text(name.span).to_owned()),
                )
        })
        .collect::<Vec<_>>();

    assert!(module_level.contains(&"Config".to_owned()));
    assert!(module_level.contains(&"real".to_owned()));
    for member in ["inner", "hidden", "ambient"] {
        assert!(
            !module_level.contains(&member.to_owned()),
            "{member} is a namespace member, not a module export: {module_level:?}"
        );
    }
    // The complete syntactic table still carries the nested statements; it
    // is the enumeration of the module's surface that filters them.
    assert!(
        facts.exports.len() > facts.module_level_exports().count(),
        "nested exports must remain in the syntactic table"
    );
}

#[test]
fn namespace_declaration_identity_excludes_every_ambient_context() {
    let source = r#"
export {};
namespace RuntimeOuter { namespace RuntimeInner {} }
module RuntimeModule {}
declare namespace Ambient { namespace HiddenAmbient {} }
declare module "pkg" { namespace HiddenStringModule {} }
declare global { namespace HiddenGlobal {} }
"#;
    let facts = extract("namespace-identity.ts", source).unwrap();
    let span_of = |name: &str| {
        let start = u32::try_from(source.find(name).expect("name occurs in source")).unwrap();
        Span::new(start, start + u32::try_from(name.len()).unwrap())
    };

    for runtime in ["RuntimeOuter", "RuntimeInner", "RuntimeModule"] {
        assert!(
            facts.declares_namespace_at(span_of(runtime)),
            "{runtime} is runtime namespace syntax"
        );
    }
    for ambient in [
        "Ambient",
        "HiddenAmbient",
        "HiddenStringModule",
        "HiddenGlobal",
    ] {
        assert!(
            !facts.declares_namespace_at(span_of(ambient)),
            "{ambient} is inside an ambient declaration"
        );
    }
    assert_eq!(facts.namespace_declarations.len(), 3);
}

#[test]
fn exported_namespace_surface_is_exact_and_includes_explicit_declare() {
    let source = r#"
export namespace RuntimeSurface {}
export declare namespace DeclaredSurface {}
export const value = function () { namespace NestedInsideValue {} return 1; };
export declare module "quoted" {}
"#;
    let facts = extract("namespace-surface.ts", source).unwrap();
    let surfaces = facts
        .module_level_exports()
        .flat_map(|export| &export.declaration_surface_only)
        .map(|declaration| declaration.exported.as_str())
        .collect::<Vec<_>>();

    assert_eq!(surfaces, ["RuntimeSurface", "DeclaredSurface"]);
    assert!(facts.module_level_exports().any(|export| {
        export
            .declarations
            .iter()
            .any(|declaration| declaration.exported == "value")
    }));
}

/// A class **static block** is neither a function body nor a module
/// block, so `exported_bindings` used to admit a declarator inside one
/// onto the export surface the same way a namespace member's used to. The
/// exclusion tests strict containment in a `ClassFact::span`, and the two
/// shapes here sit on opposite sides of that containment: `hiddenInside`'s
/// declarator is inside the class body, so the class span contains it, but
/// `boxed`'s own declarator span is the whole `boxed = class { … }`
/// initializer, which *contains* the class expression instead — the
/// reverse relationship — so `boxed` must survive the same exclusion that
/// removes `hiddenInside`.
#[test]
fn class_static_blocks_are_not_module_level_bindings_either() {
    let source = r#"
export class Holder {
  static {
    const insideStaticBlock = 1;
  }
}
export const boxed = class {
  static {
    const hiddenInside = 2;
  }
};
"#;
    let facts = extract("static-blocks.ts", source).unwrap();
    let text = |span: Span| &source[span.start as usize..span.end as usize];
    let module_level = facts
        .module_level_exports()
        .flat_map(|export| {
            export
                .declarations
                .iter()
                .chain(&export.specifiers)
                .map(|specifier| specifier.exported.to_string())
                .chain(
                    facts
                        .exported_bindings(export)
                        .flat_map(|binding| &binding.names)
                        .map(|name| text(name.span).to_owned()),
                )
        })
        .collect::<Vec<_>>();

    // `Holder` is a class declaration, not a `BindingFact`, so it reaches
    // the surface through `export.declarations`, not `exported_bindings`.
    assert!(module_level.contains(&"Holder".to_owned()));
    // `boxed`'s own declarator survives: its span *contains* the class
    // expression it initializes, the reverse of strict containment.
    assert!(module_level.contains(&"boxed".to_owned()));
    for member in ["insideStaticBlock", "hiddenInside"] {
        assert!(
            !module_level.contains(&member.to_owned()),
            "{member} is declared inside a class body, not the module: {module_level:?}"
        );
    }
}

#[test]
fn classifies_cleanup_return_shapes_from_ast_nodes() {
    let source = r#"
const cleanup = () => {};
const valid = () => {
  if (ready) return undefined;
  return cleanup;
};
const invalid = async () => 42;
const mixed = () => {
  if (ready) return () => {};
  return { invalid: true };
};
"#;
    let facts = extract("cleanup.ts", source).unwrap();
    let cleanup = facts
        .bindings
        .iter()
        .find(|binding| {
            source.get(binding.names[0].span.start as usize..binding.names[0].span.end as usize)
                == Some("cleanup")
        })
        .unwrap();
    assert!(cleanup.initializer_function);
    let values = facts
        .returns
        .iter()
        .map(|returned| returned.value)
        .collect::<Vec<_>>();
    assert_eq!(
        values,
        [
            ReturnValueKind::Undefined,
            ReturnValueKind::Identifier,
            ReturnValueKind::Function,
            ReturnValueKind::Other,
        ]
    );
    let returned_identifier = facts
        .returns
        .iter()
        .find(|returned| returned.value == ReturnValueKind::Identifier)
        .unwrap();
    assert_eq!(
        source.get(returned_identifier.span.start as usize..returned_identifier.span.end as usize),
        Some("cleanup")
    );
    assert!(facts.functions.iter().any(|function| {
        function.r#async
            && function
                .expression_return
                .as_ref()
                .is_some_and(|returned| returned.value == ReturnValueKind::Other)
    }));
}

#[test]
fn classifies_argument_shapes_and_boolean_options() {
    let source =
        "createMemo(async () => 1, { sync: true, ownedWrite: false }); runWithOwner(null, f);";
    let facts = extract("options.ts", source).unwrap();
    let call = &facts.calls[0];
    assert_eq!(call.arguments[0].value, ArgumentValueKind::AsyncFunction);
    assert_eq!(
        call.arguments[1]
            .boolean_properties
            .iter()
            .map(|property| (
                source.get(property.name.start as usize..property.name.end as usize),
                property.value,
            ))
            .collect::<Vec<_>>(),
        [(Some("sync"), true), (Some("ownedWrite"), false),]
    );
    assert_eq!(facts.calls[1].arguments[0].value, ArgumentValueKind::Null);
}

#[test]
fn argument_literals_keep_exact_values_and_refuse_dynamic_structure() {
    let facts = extract(
        "arguments.ts",
        r#"use(true as const); use(["Control", "K"]); use([, ,]);
use([...keys]); use({ resize() {}, resize: undefined });
use({ resize: () => {}, ...other }); use({ get resize() { return fn; } });
use({ [key]: () => {} }); use({ nested: { enabled: false } });"#,
    )
    .unwrap();
    let values = facts
        .calls
        .iter()
        .map(|call| &call.arguments[0].literal_value)
        .collect::<Vec<_>>();
    assert_eq!(values[0], &ArgumentLiteralFact::Boolean(true));
    assert_eq!(values[1], &ArgumentLiteralFact::ArrayLength(2));
    assert_eq!(values[2], &ArgumentLiteralFact::ArrayLength(2));
    for index in [3, 5, 6, 7] {
        assert_eq!(values[index], &ArgumentLiteralFact::Unknown);
    }
    let ArgumentLiteralFact::Object(properties) = values[4] else {
        panic!("expected final own properties");
    };
    assert_eq!(properties.len(), 1);
    assert_eq!(properties[0].name, "resize");
    assert_eq!(properties[0].value, ArgumentLiteralFact::Unknown);
    assert!(matches!(values[8], ArgumentLiteralFact::Object(_)));
}

#[test]
fn object_completeness_survives_transparent_typescript_wrappers() {
    let source = "effect(compute, {} as unknown as Apply); effect(compute, ({ effect: apply }) satisfies Bundle); effect(compute, [] as unknown as Apply); effect(compute, null!); effect(compute, <Apply><unknown>5);";
    let facts = extract("effect.ts", source).unwrap();
    let empty = &facts.calls[0].arguments[1];
    assert_eq!(empty.runtime_value_kind, RuntimeValueKind::Object);
    assert!(empty.runtime_value_kind.is_data_literal());
    assert!(empty.exact_object_literal);
    assert!(empty.closed_object_literal);
    assert!(empty.property_names.is_empty());
    assert!(empty.runtime_type_escape);

    let bundle = &facts.calls[1].arguments[1];
    assert_eq!(bundle.runtime_value_kind, RuntimeValueKind::Object);
    assert!(bundle.exact_object_literal);
    assert!(bundle.closed_object_literal);
    assert!(!bundle.runtime_type_escape);
    assert_eq!(
        bundle
            .property_names
            .iter()
            .filter_map(|span| source.get(span.start as usize..span.end as usize))
            .collect::<Vec<_>>(),
        ["effect"]
    );

    let array = &facts.calls[2].arguments[1];
    assert_eq!(array.runtime_value_kind, RuntimeValueKind::Array);
    assert!(array.runtime_value_kind.is_data_literal());
    assert!(array.runtime_type_escape);

    let nullish = &facts.calls[3].arguments[1];
    assert_eq!(nullish.runtime_value_kind, RuntimeValueKind::Nullish);
    assert!(nullish.runtime_type_escape);

    let angle_asserted = &facts.calls[4].arguments[1];
    assert_eq!(
        angle_asserted.runtime_value_kind,
        RuntimeValueKind::Primitive
    );
    assert!(angle_asserted.runtime_type_escape);
}

#[test]
fn closed_object_literals_allow_accessors_but_reject_hidden_keys() {
    let facts = extract(
        "objects.ts",
        "merge({ get value() { return 1; } }); merge({ ...other }); merge({ [key]: 1 });",
    )
    .unwrap();
    let getter = &facts.calls[0].arguments[0];
    assert!(getter.closed_object_literal);
    assert!(!getter.exact_object_literal);
    assert!(!facts.calls[1].arguments[0].closed_object_literal);
    assert!(!facts.calls[2].arguments[0].closed_object_literal);
}

#[test]
fn global_undefined_is_distinct_from_a_shadowed_binding() {
    let facts = extract(
        "effect.ts",
        r#"effect(undefined!);
function run(undefined: () => void) {
    effect(undefined!);
    return undefined;
}"#,
    )
    .unwrap();

    assert_eq!(
        facts.calls[0].arguments[0].runtime_value_kind,
        RuntimeValueKind::Nullish
    );
    assert_eq!(
        facts.calls[1].arguments[0].runtime_value_kind,
        RuntimeValueKind::Unknown
    );
    assert_eq!(facts.returns[0].value, ReturnValueKind::Identifier);
}

#[test]
fn retains_named_callbacks_in_object_arguments() {
    let facts = extract(
        "effect.ts",
        "createEffect(compute, { effect: apply, error: handle });",
    )
    .unwrap();
    assert_eq!(
        facts.calls[0].arguments[1].identifier_properties,
        [
            NamedSpan {
                span: Span::new(32, 37),
            },
            NamedSpan {
                span: Span::new(46, 52),
            },
        ]
    );
}

#[test]
fn retains_conditional_returns_and_jsx_boolean_properties() {
    let source = "const View = (props) => props.ready ? <For keyed={false} /> : <Show keyed />;";
    let facts = extract("control.tsx", source).unwrap();
    assert!(
        facts.functions[0]
            .expression_return
            .as_ref()
            .is_some_and(|returned| returned.conditional)
    );
    assert_eq!(
        facts
            .jsx_elements
            .iter()
            .map(|element| {
                element
                    .boolean_properties
                    .iter()
                    .map(|property| {
                        (
                            source.get(property.name.start as usize..property.name.end as usize),
                            property.value,
                        )
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>(),
        [vec![(Some("keyed"), false)], vec![(Some("keyed"), true)],]
    );
    assert!(facts.jsx_elements.iter().all(|element| {
        element.properties.iter().all(|property| {
            source.get(property.start as usize..property.end as usize) == Some("keyed")
        })
    }));
}

#[test]
fn relates_returns_to_the_if_tests_that_control_them() {
    let source = "function View(props) { if (props.debug) return null; const label = props.ready ? 'yes' : 'no'; return <div>{label}</div>; }";
    let facts = extract("control.tsx", source).unwrap();
    let debug_start = u32::try_from(source.find("props.debug").unwrap()).unwrap();
    let ready_start = u32::try_from(source.find("props.ready").unwrap()).unwrap();
    let guarded = facts
        .returns
        .iter()
        .find(|returned| !returned.control_tests.is_empty())
        .expect("guarded return");
    assert_eq!(guarded.control_tests[0].start, debug_start);
    assert!(
        guarded
            .control_tests
            .iter()
            .all(|test| test.start != ready_start),
        "an unrelated conditional must not control the early return"
    );
    assert!(
        facts
            .returns
            .iter()
            .any(|returned| returned.control_tests.is_empty()),
        "the final return is not controlled by the earlier if"
    );
}

#[test]
fn retains_type_only_import_specifiers() {
    let facts = extract(
        "types.ts",
        r#"import value, { type Shape, runtime as renamed, OnlyType } from "./dependency";
type Alias = OnlyType;
void value;
renamed();"#,
    )
    .unwrap();

    let bindings = &facts.imports[0].bindings;
    assert!(
        bindings
            .iter()
            .find(|binding| binding.imported.as_deref() == Some("Shape"))
            .is_some_and(|binding| binding.type_only)
    );
    assert!(
        bindings
            .iter()
            .filter(|binding| binding.imported.as_deref() != Some("Shape"))
            .all(|binding| !binding.type_only)
    );
    assert!(
        !bindings
            .iter()
            .find(|binding| binding.imported.as_deref() == Some("Shape"))
            .unwrap()
            .runtime_referenced
    );
    assert!(
        !bindings
            .iter()
            .find(|binding| binding.imported.as_deref() == Some("OnlyType"))
            .unwrap()
            .runtime_referenced
    );
    assert!(
        bindings
            .iter()
            .find(|binding| binding.imported.as_deref() == Some("runtime"))
            .unwrap()
            .runtime_referenced
    );
}

#[test]
fn represents_side_effect_imports_without_an_empty_name_sentinel() {
    let source = r#"import "./setup";"#;
    let facts = extract("setup.ts", source).unwrap();
    let binding = &facts.imports[0].bindings[0];

    assert_eq!(binding.kind, ImportKind::SideEffect);
    assert_eq!(
        source.get(binding.local.span.start as usize..binding.local.span.end as usize),
        Some(r#""./setup""#)
    );
    assert!(facts.structural_seed_spans().is_empty());
}

#[test]
fn module_loading_and_opaque_hazards_are_scope_exact_and_explicit() {
    let facts = extract(
            "fixture.ts",
            r#"
                import("./literal.js");
                import(`./template-literal.js`);
                import(dynamicName);
                require("./required.js");
                require(`./template-required.js`);
                eval(source);
                WebAssembly.instantiate(bytes);
                leaked = 1;
                advanced++;
                ({ destructured } = source);
                for (iterated of values) {}
                function local(require: (name: string) => unknown, eval: (text: string) => unknown) {
                    require("not-a-module.js");
                    eval("not-eval");
                    let WebAssembly = { instantiate() {} };
                    WebAssembly.instantiate(bytes);
                    let leaked = 0;
                    leaked = 2;
                    let advanced = 0;
                    advanced++;
                    let destructured = 0;
                    ({ destructured } = source);
                    let iterated = 0;
                    for (iterated of values) {}
                }
            "#,
        )
        .unwrap();

    assert_eq!(facts.module_loads.len(), 5);
    assert_eq!(
        facts
            .module_loads
            .iter()
            .filter_map(|load| load.specifier.as_deref())
            .collect::<Vec<_>>(),
        vec![
            "./literal.js",
            "./template-literal.js",
            "./required.js",
            "./template-required.js"
        ]
    );
    assert_eq!(facts.module_hazards.len(), 7);
}

/// `import S = require("m")` and a type-position `import("m")` are module
/// references no import declaration or call records (facts schema 46).
#[test]
fn import_equals_and_type_imports_are_recorded() {
    let source = r#"import S = require("solid-js");
import type T = require("./types");
export import E = require("@solidjs/web");
import Alias = S.createSignal;
type A = import("solid-js").Accessor<number>;
type B = typeof import("solid-js").createErrorBoundary;
type C = typeof import("solid-js");
type D = import("./deep").Outer.Inner;
S.createErrorBoundary;
"#;
    let facts = extract("/p/App.ts", source).unwrap();
    let rows = facts
        .import_equals
        .iter()
        .map(|fact| {
            (
                fact.module.as_str(),
                &source[fact.local.span.start as usize..fact.local.span.end as usize],
                fact.type_only,
                fact.exported,
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        rows,
        [
            ("solid-js", "S", false, false),
            ("./types", "T", true, false),
            ("@solidjs/web", "E", false, true),
        ]
    );
    // Every declaration contains its specifier literal, which is how an
    // attested resolution row joins it.
    for fact in &facts.import_equals {
        let text = &source[fact.span.start as usize..fact.span.end as usize];
        assert!(text.contains(&format!("\"{}\"", fact.module)), "{text}");
    }
    // The binder resolves a member read's object to the import-equals
    // binding.
    let member = facts
        .members
        .iter()
        .find(|member| {
            &source[member.property.start as usize..member.property.end as usize]
                == "createErrorBoundary"
        })
        .expect("the member read is recorded");
    assert_eq!(
        facts.reference_declaration(member.object),
        Some(facts.import_equals[0].local.span)
    );
    let types = facts
        .type_imports
        .iter()
        .map(|fact| (fact.module.as_str(), fact.member.as_deref()))
        .collect::<Vec<_>>();
    assert_eq!(
        types,
        [
            ("solid-js", Some("Accessor")),
            ("solid-js", Some("createErrorBoundary")),
            ("solid-js", None),
            ("./deep", Some("Outer")),
        ]
    );
}

/// The span of the `index`-th occurrence of `raw` in `source`. Tests pick
/// the occurrence they mean; the extractor never searches text.
fn occurrence(source: &str, raw: &str, index: usize) -> Span {
    let (start, _) = source
        .match_indices(raw)
        .nth(index)
        .unwrap_or_else(|| panic!("{raw:?} occurs fewer than {} times", index + 1));
    Span::new(start as u32, (start + raw.len()) as u32)
}

fn sliced(source: &str, span: Option<Span>) -> &str {
    let span = span.expect("a freshly extracted specifier records its literal");
    &source[span.start as usize..span.end as usize]
}

/// Facts schema 63: each import and module-bearing export records the
/// owning string literal's own span, quotes included. The declaration can
/// hold other strings with the module's text -- an imported string name,
/// a comment, an attribute value -- and none of them is the literal.
#[test]
fn imports_and_exports_record_their_specifier_literal() {
    let source = concat!(
        "import \"side\";\n",
        "import d from 'single';\n",
        "import { \"m\" as named } from /* \"m\" */ \"m\";\n",
        "import type { T } from \"types\";\n",
        "import * as ns from \"ns\";\n",
        "import j from \"./j.json\" with { type: \"json\" };\n",
        "export { named as again } from \"m\";\n",
        "export * as all from \"star-ns\";\n",
        "export * from \"star\";\n",
        "export { d };\n",
        "export default 1;\n",
    );
    let facts = extract("/p/specifiers.ts", source).unwrap();
    assert_eq!(facts.schema, AST_FACTS_SCHEMA);
    let imports = facts
        .imports
        .iter()
        .map(|fact| (fact.module.as_str(), fact.specifier_span))
        .collect::<Vec<_>>();
    assert_eq!(
        imports,
        [
            ("side", Some(occurrence(source, "\"side\"", 0))),
            ("single", Some(occurrence(source, "'single'", 0))),
            // The third `"m"` in the source: after the imported string
            // name and the comment.
            ("m", Some(occurrence(source, "\"m\"", 2))),
            ("types", Some(occurrence(source, "\"types\"", 0))),
            ("ns", Some(occurrence(source, "\"ns\"", 0))),
            ("./j.json", Some(occurrence(source, "\"./j.json\"", 0))),
        ]
    );
    // Declarations, counts, flags and bindings are what they were.
    let declarations = facts
        .imports
        .iter()
        .map(|fact| {
            (
                &source[fact.span.start as usize..fact.span.end as usize],
                fact.type_only,
                fact.bindings.len(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        declarations,
        [
            ("import \"side\";", false, 1),
            ("import d from 'single';", false, 1),
            (
                "import { \"m\" as named } from /* \"m\" */ \"m\";",
                false,
                1
            ),
            ("import type { T } from \"types\";", true, 1),
            ("import * as ns from \"ns\";", false, 1),
            (
                "import j from \"./j.json\" with { type: \"json\" };",
                false,
                1
            ),
        ]
    );
    let exports = facts
        .exports
        .iter()
        .map(|fact| (fact.kind, fact.module.as_deref(), fact.specifier_span))
        .collect::<Vec<_>>();
    assert_eq!(
        exports,
        [
            (
                ExportKind::Named,
                Some("m"),
                Some(occurrence(source, "\"m\"", 3))
            ),
            (
                ExportKind::All,
                Some("star-ns"),
                Some(occurrence(source, "\"star-ns\"", 0))
            ),
            (
                ExportKind::All,
                Some("star"),
                Some(occurrence(source, "\"star\"", 0))
            ),
            (ExportKind::Named, None, None),
            (ExportKind::Default, None, None),
        ]
    );
}

/// Import-equals and import types record the literal too. An exported
/// import-equals keeps its module-free outer export, and the entity-name
/// form loads no module and stays unrecorded. Nested import types with
/// the same text keep distinct literals, neither one the qualifier or a
/// type argument.
#[test]
fn import_equals_and_type_imports_record_their_specifier_literal() {
    let source = r#"import S = require("solid-js");
import type T = require('./types');
export import E = require("@solidjs/web");
import Alias = S.createSignal;
type A = import("a").X<import("a").Y>;
type B = typeof import("b").c.d;
type C = typeof import("a");
"#;
    let facts = extract("/p/App.ts", source).unwrap();
    let equals = facts
        .import_equals
        .iter()
        .map(|fact| (fact.module.as_str(), fact.specifier_span, fact.type_only))
        .collect::<Vec<_>>();
    assert_eq!(
        equals,
        [
            (
                "solid-js",
                Some(occurrence(source, "\"solid-js\"", 0)),
                false
            ),
            ("./types", Some(occurrence(source, "'./types'", 0)), true),
            (
                "@solidjs/web",
                Some(occurrence(source, "\"@solidjs/web\"", 0)),
                false
            ),
        ]
    );
    let outer = facts
        .exports
        .iter()
        .map(|fact| (fact.kind, fact.module.as_deref(), fact.specifier_span))
        .collect::<Vec<_>>();
    assert_eq!(outer, [(ExportKind::Named, None, None)]);
    let types = facts
        .type_imports
        .iter()
        .map(|fact| {
            (
                fact.module.as_str(),
                fact.member.as_deref(),
                fact.specifier_span,
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        types,
        [
            ("a", Some("X"), Some(occurrence(source, "\"a\"", 0))),
            ("a", Some("Y"), Some(occurrence(source, "\"a\"", 1))),
            ("b", Some("c"), Some(occurrence(source, "\"b\"", 0))),
            ("a", None, Some(occurrence(source, "\"a\"", 2))),
        ]
    );
    // The outer import type contains the inner one's literal, and records
    // its own.
    let [first, second, ..] = facts.type_imports.as_slice() else {
        panic!("import types: {:?}", facts.type_imports);
    };
    assert!(first.span.contains(second.specifier_span.unwrap()));
    assert_ne!(first.specifier_span, second.specifier_span);
}

/// Byte offsets of the raw literal, whatever it spells: escapes stay raw
/// beside the cooked module, non-ASCII text and multibyte leading trivia
/// count in UTF-8 bytes, the empty literal is two quotes wide, and line or
/// control whitespace before the literal is never part of it.
#[test]
fn specifier_literals_are_raw_utf8_byte_spans() {
    let source = concat!(
        "/* ü — ✓ */ import a from \"p\\u006bg\";\n",
        "import b from \"ünï\";\n",
        "import c from \"\";\n",
        "import e from\n\t\u{000C} \"next-line\";\n",
    );
    let facts = extract("/p/raw.ts", source).unwrap();
    let rows = facts
        .imports
        .iter()
        .map(|fact| {
            (
                fact.module.as_str(),
                sliced(source, fact.specifier_span),
                fact.specifier_span,
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        rows,
        [
            (
                "pkg",
                "\"p\\u006bg\"",
                Some(occurrence(source, "\"p\\u006bg\"", 0))
            ),
            ("ünï", "\"ünï\"", Some(occurrence(source, "\"ünï\"", 0))),
            ("", "\"\"", Some(occurrence(source, "\"\"", 0))),
            (
                "next-line",
                "\"next-line\"",
                Some(occurrence(source, "\"next-line\"", 0))
            ),
        ]
    );
    let empty = facts.imports[2].specifier_span.unwrap();
    assert_eq!(empty.end - empty.start, 2);
}

/// A fact decoded without the field records no literal location: `None`
/// is unknown, never an absent import, and decoding does not upgrade it.
/// Freshly extracted facts round-trip with the field, and a module-free
/// export serializes no field at all.
#[test]
fn a_decoded_specifier_span_defaults_to_unknown() {
    let source = r#"import d from "m";
export * from "n";
export { d };
import S = require("o");
type T = import("p").Q;
"#;
    let facts = extract("/p/legacy.ts", source).unwrap();
    fn legacy<T: Serialize + serde::de::DeserializeOwned>(fact: &T) -> (T, serde_json::Value) {
        let mut value = serde_json::to_value(fact).unwrap();
        assert!(
            value
                .as_object_mut()
                .unwrap()
                .remove("specifierSpan")
                .is_some(),
            "{value}"
        );
        let decoded: T = serde_json::from_value(value.clone()).unwrap();
        (decoded, value)
    }
    fn fresh<T: Serialize + serde::de::DeserializeOwned + PartialEq + std::fmt::Debug>(fact: &T) {
        let decoded: T = serde_json::from_value(serde_json::to_value(fact).unwrap()).unwrap();
        assert_eq!(&decoded, fact);
    }
    let (import, stripped) = legacy(&facts.imports[0]);
    assert_eq!(import.specifier_span, None);
    assert_eq!(import.module, "m");
    assert_eq!(import.span, facts.imports[0].span);
    assert_eq!(serde_json::to_value(&import).unwrap(), stripped);
    fresh(&facts.imports[0]);
    let (export, stripped) = legacy(&facts.exports[0]);
    assert_eq!(export.specifier_span, None);
    assert_eq!(export.module.as_deref(), Some("n"));
    assert_eq!(serde_json::to_value(&export).unwrap(), stripped);
    fresh(&facts.exports[0]);
    let local = serde_json::to_value(&facts.exports[1]).unwrap();
    assert!(local.get("specifierSpan").is_none(), "{local}");
    fresh(&facts.exports[1]);
    let (equals, stripped) = legacy(&facts.import_equals[0]);
    assert_eq!(equals.specifier_span, None);
    assert_eq!(serde_json::to_value(&equals).unwrap(), stripped);
    fresh(&facts.import_equals[0]);
    let (type_import, stripped) = legacy(&facts.type_imports[0]);
    assert_eq!(type_import.specifier_span, None);
    assert_eq!(serde_json::to_value(&type_import).unwrap(), stripped);
    fresh(&facts.type_imports[0]);
    let encoded = serde_json::to_value(&facts.imports[0]).unwrap();
    assert_eq!(
        encoded["specifierSpan"],
        serde_json::json!({
            "start": facts.imports[0].specifier_span.unwrap().start,
            "end": facts.imports[0].specifier_span.unwrap().end,
        })
    );
}

/// Every way to install a property accessor at run time, which is the
/// condition the `reads` implementation census refuses on
/// (design note of 2026-09-10, no longer in the tree).
///
/// The set is not a style preference: each member is pinned by a
/// zero-form case in the producer's
/// `TestRuntimeInstalledAccessorReadsAreInvisibleToTheProducer`, which
/// measured that a read through it records **nothing** for the census to
/// refuse. A shape missing here is a hole in the refusal, so add its case
/// there and its line here together.
///
/// Detection is deliberately by *name*, not by receiver: `Object`,
/// `Reflect` and `globalThis.Object` all reach the same intrinsic, and an
/// alias (`const dp = Object.defineProperty`) reads the member before it
/// calls anything. Over-refusing here is free — the corpus measurement
/// (§ 8) found the broad set and the narrow one refuse the same packages.
#[test]
fn records_every_run_time_accessor_installation_as_a_module_hazard() {
    let installations = [
        "new Proxy(target, handler);",
        "Proxy.revocable(target, handler);",
        "const aliased = Proxy;",
        "Object.defineProperty(target, key, descriptor);",
        "Object.defineProperties(target, descriptors);",
        "Object.create(prototype, descriptors);",
        "Reflect.defineProperty(target, key, descriptor);",
        "target.__defineGetter__(key, read);",
        "target.__defineSetter__(key, write);",
        "Object.setPrototypeOf(target, prototype);",
        "target.__proto__ = prototype;",
        "const literal = { __proto__: prototype };",
        "const dp = Object.defineProperty;",
        "Object[name](target, key, descriptor);",
    ];
    for source in installations {
        let facts = extract("fixture.ts", source).unwrap();
        assert!(
            facts
                .module_hazards
                .iter()
                .any(|hazard| hazard.kind == ModuleHazardKind::RuntimeAccessorInstallation),
            "no accessor-installation hazard for {source}"
        );
    }
}

/// The other half of the claim: ordinary code must not be refused. A
/// census that refuses everything is sound and worthless.
#[test]
fn ordinary_object_use_is_not_an_accessor_installation() {
    let benign = [
        "Object.create(prototype);",
        "Object.keys(target);",
        "Object.assign(target, source);",
        "Object.entries(target);",
        "Object.freeze(target);",
        "const value = target.property;",
        "const proxied = { proxy: 1 };",
        "class Declared { get value() { return 1; } }",
        "const declared = { get value() { return 1; } };",
    ];
    for source in benign {
        let facts = extract("fixture.ts", source).unwrap();
        assert!(
            !facts
                .module_hazards
                .iter()
                .any(|hazard| hazard.kind == ModuleHazardKind::RuntimeAccessorInstallation),
            "{source} was refused as an accessor installation"
        );
    }
}

#[test]
fn distinguishes_namespace_reexports_from_export_stars() {
    let facts = extract(
        "fixture.ts",
        r#"export * as namespace from "./namespace"; export * from "./star";"#,
    )
    .unwrap();

    assert_eq!(facts.exports[0].namespace.as_deref(), Some("namespace"));
    let binding = facts.exports[0].namespace_binding.as_ref().unwrap().span;
    assert_eq!(binding.start, 12);
    assert_eq!(binding.end, 21);
    assert_eq!(facts.exports[1].namespace, None);
    assert_eq!(facts.exports[1].namespace_binding, None);
}

#[test]
fn retains_runtime_and_type_only_export_declarations() {
    let facts = extract(
            "exports.ts",
            "export class RuntimeClass {} export interface Shape {} export const value = 1; export default function Main() {}",
        )
        .unwrap();
    let declarations = facts
        .exports
        .iter()
        .flat_map(|export| &export.declarations)
        .map(|declaration| (declaration.exported.as_str(), declaration.type_only))
        .collect::<Vec<_>>();

    assert_eq!(
        declarations,
        [
            ("RuntimeClass", false),
            ("Shape", true),
            ("value", false),
            ("default", false)
        ]
    );
}

#[test]
fn resolves_local_export_specifiers_to_their_exact_declarations() {
    let facts = extract(
        "exports.ts",
        "export default function createX() {} export { createX };",
    )
    .unwrap();
    let default_declaration = facts.exports[0].declarations[0].local.span;
    let named_specifier = facts.exports[1].specifiers[0].local.span;

    assert_eq!(
        facts.reference_declaration(named_specifier),
        Some(default_declaration)
    );
}

#[test]
fn rejects_malformed_source() {
    assert!(matches!(
        extract("broken.tsx", "const = ;"),
        Err(AstFactsError::Parse(_))
    ));
}

#[test]
fn a_json_import_target_is_certified_inert_rather_than_a_fatal_error() {
    // A real dependency's ESM entrypoint reaching its own `package.json`
    // (e.g. `@solidjs/start@2.0.3`'s `dist/shared/dev-toolbar/index.jsx`)
    // used to make the whole build fail with an Oxc "unsupported source
    // path" error, because Oxc's `SourceType` has no JSON source kind.
    // `extract` must instead certify the JSON module as inert: every
    // fact table empty, proven rather than assumed, and no error.
    let facts = extract("package.json", r#"{"name": "demo", "version": "1.0.0"}"#).unwrap();
    assert_eq!(facts.calls, Vec::new());
    assert_eq!(facts.bindings, Vec::new());
    assert_eq!(facts.functions, Vec::new());
    assert_eq!(facts.imports, Vec::new());
    assert_eq!(facts.exports, Vec::new());
    assert_eq!(facts.identifiers, Vec::new());
    assert_eq!(facts.jsx_elements, Vec::new());
    assert_eq!(facts.members, Vec::new());
}

#[test]
fn json_module_detection_is_by_extension_not_by_filename() {
    // The rule is the `.json` module kind, not the specific filename
    // `package.json`: any `.json` specifier takes the inert path, and a
    // non-JSON extension does not, even when it is not `package.json`.
    assert!(is_json_module_path("package.json"));
    assert!(is_json_module_path("/some/pkg/data.json"));
    assert!(is_json_module_path("/some/pkg/DATA.JSON"));
    assert!(!is_json_module_path("package.jsonc"));
    assert!(!is_json_module_path("package.js"));
}

#[test]
fn a_genuinely_unsupported_extension_still_fails_closed() {
    // The JSON fix must not become a blanket "skip anything we cannot
    // parse": a source reached through some other unsupported extension
    // is still a fatal AST facts error, exactly as before.
    assert!(matches!(
        extract("logo.svg", "<svg></svg>"),
        Err(AstFactsError::SourceType { .. })
    ));
}

#[test]
fn retains_computed_members_and_spreads_as_reactive_read_shapes() {
    let source = "const key = 'name'; const value = props[key]; const copy = { ...props };";
    let facts = extract("reads.ts", source).unwrap();

    assert_eq!(facts.members.len(), 1);
    assert_eq!(
        source
            .get(facts.members[0].property.start as usize..facts.members[0].property.end as usize),
        Some("key")
    );
    assert_eq!(
        &source[usize::try_from(facts.members[0].object.start).unwrap()
            ..usize::try_from(facts.members[0].object.end).unwrap()],
        "props"
    );
    assert_eq!(facts.spreads.len(), 1);
    assert_eq!(
        &source[usize::try_from(facts.spreads[0].argument.start).unwrap()
            ..usize::try_from(facts.spreads[0].argument.end).unwrap()],
        "props"
    );
}

/// Item B of ways-to-improve § 3.3: a computed member names one property
/// when its key is a string literal or a non-negative integer numeric
/// literal -- in the canonical decimal spelling the producer states --
/// and names none otherwise; a `new` is marked as one.
#[test]
fn records_literal_member_keys_and_construct_calls() {
    let source = "h[0](); h[\"run\"](); h[0x6](); h[k](); h[-1](); h[1.5](); \
                      h[`x`](); h[1e21](); new h[2]();";
    let facts = extract("members.ts", source).unwrap();
    let keys = facts
        .literal_computed_members
        .iter()
        .map(|fact| {
            (
                source
                    .get(fact.span.start as usize..fact.span.end as usize)
                    .unwrap(),
                fact.key.as_str(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        keys,
        [
            ("h[0]", "0"),
            ("h[\"run\"]", "run"),
            ("h[0x6]", "6"),
            ("h[`x`]", "x"),
            ("h[2]", "2")
        ]
    );
    let constructs = facts
        .calls
        .iter()
        .filter(|call| call.construct)
        .map(|call| {
            source
                .get(call.span.start as usize..call.span.end as usize)
                .unwrap()
        })
        .collect::<Vec<_>>();
    assert_eq!(constructs, ["new h[2]()"]);
}

/// Facts schema 44: a member link written `?.` says so, and a later link
/// of the same chain, or a plain member, does not.
#[test]
fn records_which_member_links_are_optional() {
    let source = "p?.a; p?.[0]; p.b; p?.c.d; (p?.e).f;";
    let facts = extract("members.ts", source).unwrap();
    let optional = facts
        .optional_members
        .iter()
        .map(|member| {
            source
                .get(member.start as usize..member.end as usize)
                .unwrap()
        })
        .collect::<Vec<_>>();
    assert_eq!(optional, ["p?.a", "p?.[0]", "p?.c", "p?.e"]);
}

/// The members of an array or object literal argument, keyed by the
/// property key the runtime gives them, exactly when the set is static;
/// every other argument names none.
#[test]
fn records_the_exact_members_of_a_literal_argument() {
    let source = "f([a, , b, ...c, d]); f({ x: a, \"y\": b, 0: c, x: d }); \
                      f({ ...o, x: a }); f({ [k]: a }); f({ get x() { return a; } }); \
                      f({ __proto__: a }); f(a); f([a] as const);";
    let facts = extract("literals.ts", source).unwrap();
    let members = facts
        .calls
        .iter()
        .map(|call| {
            call.arguments[0]
                .literal_members
                .iter()
                .map(|member| {
                    (
                        member.key.to_string(),
                        source
                            .get(member.value.start as usize..member.value.end as usize)
                            .unwrap()
                            .to_owned(),
                    )
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let pairs = |items: &[(&str, &str)]| {
        items
            .iter()
            .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        members,
        [
            // A hole counts its index; the spread ends what is known.
            pairs(&[("0", "a"), ("2", "b")]),
            // A later duplicate key wins, as at runtime.
            pairs(&[("y", "b"), ("0", "c"), ("x", "d")]),
            pairs(&[]),
            pairs(&[]),
            pairs(&[]),
            pairs(&[]),
            pairs(&[]),
            // Behind a transparent wrapper, the literal itself.
            pairs(&[("0", "a")]),
        ]
    );
}

/// The runtime coercion table mirrors the Type Facts producer's own
/// `coercingBinaryOperators`/`coercingUnaryOperators` and template
/// substitutions, and is a separate fact from `coercive_operands`:
/// binary arithmetic is in, `!` is out, and so are strict equality,
/// `in`, `instanceof` and a tagged template's substitutions.
#[test]
fn records_the_operands_every_coercing_operator_reaches() {
    let source = "a < b; c * d; e == f; g += h; +i; ~j; k++; --l; `${m}`; \
                      !n; o === p; q in r; s instanceof t; tag`${u}`; v && w; \
                      x != null; null == y; z == undefined;";
    let facts = extract("coercions.ts", source).unwrap();
    let operands = facts
        .coercing_operands
        .iter()
        .filter_map(|span| source.get(span.start as usize..span.end as usize))
        .collect::<Vec<_>>();
    assert_eq!(
        operands,
        [
            "a",
            "b",
            "c",
            "d",
            "e",
            "f",
            "g",
            "h",
            "i",
            "j",
            "k",
            "l",
            "m",
            "z",
            "undefined"
        ],
        "a loose comparison with the `null` keyword coerces nothing, as the producer \
             counts it; `undefined` is an identifier and still does"
    );
    assert!(
        facts.coercive_operands.iter().all(|operand| source
            .get(operand.span.start as usize..operand.span.end as usize)
            != Some("c")),
        "`coercive_operands` keeps its own meaning: binary arithmetic stays out"
    );
}

/// The operands the iteration protocol is driven over: a `for…of` operand,
/// an array or argument spread, `yield*`, and an array pattern's source.
/// An object spread is a property enumeration and is absent.
#[test]
fn records_every_value_the_iteration_protocol_is_driven_over() {
    let source = "for (const x of a) {} [...b]; f(...c); const [d0] = d; [e0] = e; \
                      function* g() { yield* h; } ({ ...i }); const { j0 } = j;";
    let facts = extract("iterations.ts", source).unwrap();
    let operands = facts
        .iterated_operands
        .iter()
        .filter_map(|span| source.get(span.start as usize..span.end as usize))
        .collect::<Vec<_>>();
    assert_eq!(operands, ["a", "b", "c", "d", "e", "h"]);
}

#[test]
fn retains_only_typescript_valid_operators_that_coerce_accessor_values() {
    let source = "const a = signal + 1; const b = -signal; const c = !signal; const d = signal === other; const e = typeof signal; const f = \"value: \" + signal;";
    let facts = extract("operators.ts", source).unwrap();
    let operands = facts
        .coercive_operands
        .iter()
        .filter_map(|operand| source.get(operand.span.start as usize..operand.span.end as usize))
        .collect::<Vec<_>>();

    // `signal + 1` is TS2365 and stays out; `-signal`, `!signal`, and the
    // string-literal concatenation are all TypeScript-clean and stay in.
    assert_eq!(operands, ["signal", "signal", "\"value: \"", "signal"]);
}

#[test]
fn retains_array_assignments_and_if_regions_for_runtime_proofs() {
    let source = r#"
let callbacks = null;
let resource;
if (!callbacks) callbacks = [];
if (Array.isArray(callbacks)) callbacks.push(fn);
[resource] = createResource(source, fetcher);
"#;
    let facts = extract("/project/runtime.js", source).unwrap();
    assert!(facts.assignments.iter().any(|assignment| {
        assignment.value == AssignmentValueKind::Array
            && source.get(assignment.target.start as usize..assignment.target.end as usize)
                == Some("callbacks")
    }));
    let resource = facts
        .assignments
        .iter()
        .find(|assignment| !assignment.array_slots.is_empty())
        .unwrap();
    assert_eq!(
        resource
            .array_slots
            .first()
            .and_then(|slot| *slot)
            .and_then(|span| source.get(span.start as usize..span.end as usize)),
        Some("resource")
    );
    assert_eq!(
        resource
            .call_initializer
            .and_then(|span| source.get(span.start as usize..span.end as usize)),
        Some("createResource(source, fetcher)")
    );
    assert!(facts.if_regions.iter().any(|region| {
        source.get(region.test.start as usize..region.test.end as usize)
            == Some("Array.isArray(callbacks)")
            && source
                .get(region.consequent.start as usize..region.consequent.end as usize)
                .is_some_and(|consequent| consequent.contains("callbacks.push(fn)"))
    }));
}

#[test]
fn assignment_facts_distinguish_plain_compound_and_update_reads() {
    let source = "let value = 0; value = 1; value += 2; value++;";
    let facts = extract("assignments.ts", source).unwrap();
    let reads = facts
        .assignments
        .iter()
        .map(|assignment| {
            (
                source
                    .get(assignment.target.start as usize..assignment.target.end as usize)
                    .unwrap(),
                assignment.reads_target,
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        reads,
        vec![("value", false), ("value", true), ("value", true)]
    );
}

#[test]
fn exact_write_targets_separate_destructuring_leaves_from_reads() {
    let source = "out[host.hydrating ? 1 : 0] = 1; ({ a: host.done, b: local = host.hydrating } = input); host.done ||= true; for (host.hydrating of flags) {} (host.done as boolean) = false;";
    let facts = extract("targets.ts", source).unwrap();
    let written = facts
        .write_targets
        .iter()
        .map(|target| &source[target.start as usize..target.end as usize])
        .collect::<Vec<_>>();
    assert_eq!(
        written,
        vec![
            "out[host.hydrating ? 1 : 0]",
            "host.done",
            "local",
            "host.done",
            "host.hydrating",
            "host.done",
        ]
    );
}

#[test]
fn only_the_written_target_itself_is_a_plain_assignment_target() {
    let source = "state.rows[props.index].done = true; state.count += 1; ({ a: local = fallback.name } = incoming);";
    let facts = extract("targets.ts", source).unwrap();
    let plain = |text: &str| {
        let start = u32::try_from(source.find(text).unwrap()).unwrap();
        facts.is_plain_assignment_target(Span::new(
            start,
            start + u32::try_from(text.len()).unwrap(),
        ))
    };
    // The written member is a write; the computed key that addresses it,
    // the compound target that evaluates its old value, and the default
    // that builds a destructured value are all genuine reads.
    assert!(plain("state.rows[props.index].done"));
    assert!(!plain("props.index"));
    assert!(!plain("state.count"));
    assert!(!plain("fallback.name"));
}

#[test]
fn retains_complete_jsx_rule_structure() {
    let source = r#"<Button use:focus style={{ fontSize: 12 }} {...props}>
  {ready && <Content />}
  {ready ? <Yes /> : <No />}
</Button>"#;
    let facts = extract("rules.tsx", source).unwrap();
    let button = &facts.jsx_elements[0];

    assert!(!button.self_closing);
    assert_eq!(button.attributes.len(), 2);
    assert_eq!(button.spreads.len(), 1);
    assert_eq!(button.children.len(), 5);
    let directive = &button.attributes[0];
    assert_eq!(
        directive
            .namespace
            .and_then(|span| source.get(span.start as usize..span.end as usize)),
        Some("use")
    );
    assert_eq!(
        source.get(directive.local_name.start as usize..directive.local_name.end as usize),
        Some("focus")
    );
    assert_eq!(facts.object_properties.len(), 1);
    assert_eq!(facts.logical_expressions.len(), 1);
    assert_eq!(facts.conditional_expressions.len(), 1);
}

#[test]
fn records_the_closing_tags_name_span_beside_the_opening_ones() {
    // A closing tag writes the element's name a second time, and
    // TypeScript reports that occurrence as another reference to the same
    // symbol. A consumer enumerating references to a component has to be
    // able to see it, so the span is recorded — as the same name, not as
    // another element.
    let source = "<Panel><dotted.Inner></dotted.Inner><Leaf /></Panel>";
    let facts = extract("closing.tsx", source).unwrap();
    let text = |span: Span| source.get(span.start as usize..span.end as usize);
    let named = facts
        .jsx_elements
        .iter()
        .map(|element| {
            (
                text(element.name.span),
                element.closing_name.and_then(text),
                element.self_closing,
            )
        })
        .collect::<Vec<_>>();

    assert_eq!(
        named,
        vec![
            (Some("Panel"), Some("Panel"), false),
            (Some("dotted.Inner"), Some("dotted.Inner"), false),
            (Some("Leaf"), None, true),
        ]
    );
    // The two spans of a paired tag are distinct positions, which is the
    // whole point: the second one is unaccounted for unless something
    // records it.
    let panel = &facts.jsx_elements[0];
    assert_ne!(Some(panel.name.span), panel.closing_name);
}

#[test]
fn jsx_attributes_distinguish_runtime_type_escapes_from_checked_wrappers() {
    let source = r#"<div
  onClick={handler as unknown as () => void}
  onInput={handler satisfies () => void}
  onFocus={(handler)}
  onBlur={handler!}
/>"#;
    let facts = extract("attributes.tsx", source).unwrap();
    let element = &facts.jsx_elements[0];
    let escaped = element
        .attributes
        .iter()
        .filter(|attribute| attribute.runtime_type_escape)
        .filter_map(|attribute| {
            source.get(attribute.name.start as usize..attribute.name.end as usize)
        })
        .collect::<Vec<_>>();
    assert_eq!(escaped, ["onClick", "onBlur"]);
    for attribute in &element.attributes {
        let expression = attribute.expression.expect("expression attribute");
        let runtime = facts.peel_ts_sugar_span(expression);
        assert_eq!(
            source.get(runtime.start as usize..runtime.end as usize),
            Some("handler")
        );
    }
}

#[test]
fn resolves_directive_names_through_value_scope_only() {
    let source = r#"
import { importedDirective } from "./directives";
import type { typeOnlyDirective } from "./types";
function hoistedDirective() {}
const moduleDirective = () => {};
const shadowedDirective = () => {};
interface interfaceDirective {}

function View(shadowedDirective: (element: HTMLDivElement) => void) {
  const parameterUse = <div use:shadowedDirective />;
  const visible = <div use:importedDirective use:hoistedDirective use:moduleDirective />;
  {
    const blockDirective = () => {};
    const blockUse = <div use:blockDirective />;
  }
  const outsideBlock = <div use:blockDirective />;
  const typeOnly = <div use:typeOnlyDirective use:interfaceDirective />;
  return <div class:moduleDirective use:missingDirective />;
}
"#;
    let facts = extract("directives.tsx", source).unwrap();
    let attributes = facts
        .jsx_elements
        .iter()
        .flat_map(|element| &element.attributes)
        .map(|attribute| {
            let namespace = attribute
                .namespace
                .and_then(|span| source.get(span.start as usize..span.end as usize));
            let name = source
                .get(attribute.local_name.start as usize..attribute.local_name.end as usize)
                .unwrap();
            let binding = attribute.directive_binding.and_then(|span| {
                source
                    .get(span.start as usize..span.end as usize)
                    .map(|text| (text, span.start))
            });
            (namespace, name, binding)
        })
        .collect::<Vec<_>>();

    for name in [
        "importedDirective",
        "hoistedDirective",
        "moduleDirective",
        "blockDirective",
    ] {
        assert!(
            attributes.iter().any(|&(namespace, local, binding)| {
                namespace == Some("use")
                    && local == name
                    && binding.is_some_and(|(declaration, _)| declaration == name)
            }),
            "missing lexical value resolution for {name}: {attributes:#?}"
        );
    }

    let shadowed = attributes
        .iter()
        .find(|&&(namespace, name, _)| namespace == Some("use") && name == "shadowedDirective")
        .and_then(|&(_, _, binding)| binding)
        .expect("shadowed parameter binding");
    assert_eq!(
        shadowed.1 as usize,
        source
            .find("shadowedDirective: (element")
            .expect("parameter declaration")
    );

    let unresolved = attributes
        .iter()
        .filter(|&&(namespace, _, binding)| namespace == Some("use") && binding.is_none())
        .map(|&(_, name, _)| name)
        .collect::<Vec<_>>();
    assert_eq!(
        unresolved,
        [
            "blockDirective",
            "typeOnlyDirective",
            "interfaceDirective",
            "missingDirective"
        ]
    );
    assert!(attributes.iter().any(|&(namespace, name, binding)| {
        namespace == Some("class") && name == "moduleDirective" && binding.is_none()
    }));
}

#[test]
fn resolves_shorthand_property_values_through_block_scope() {
    let source = r#"
import { importedValue } from "./values";

const moduleValue = () => "module";

function make() {
  {
    const scoped = () => "block";
    const fromBlock = { scoped };
  }
  const scoped = () => "function";
  const fromFunction = { scoped };
  const fromModule = { moduleValue };
  const fromImport = { importedValue };
  const fromGlobal = { structuredClone };
  const written = { scoped: moduleValue };
  const computed = { [moduleValue]: 1 };
}
"#;
    let facts = extract("shorthand.ts", source).unwrap();
    let text = |span: Span| source.get(span.start as usize..span.end as usize).unwrap();
    let resolved = facts
        .object_properties
        .iter()
        .map(|property| {
            (
                text(property.span),
                property
                    .shorthand_binding
                    .map(|binding| (text(binding), binding.start as usize)),
            )
        })
        .collect::<Vec<_>>();

    // Two `scoped` declarations in sibling scopes. Each shorthand takes
    // the one its own scope chain reaches -- the distinction the spelling
    // alone cannot make. Everything below is keyed off the declaration
    // offset, so a same-spelled sibling cannot satisfy the assertion.
    assert_eq!(
        resolved,
        [
            (
                "scoped",
                Some(("scoped", source.find("scoped = () => \"block\"").unwrap()))
            ),
            (
                "scoped",
                Some((
                    "scoped",
                    source.find("scoped = () => \"function\"").unwrap()
                ))
            ),
            (
                "moduleValue",
                Some((
                    "moduleValue",
                    source.find("moduleValue = () => \"module\"").unwrap()
                ))
            ),
            (
                "importedValue",
                Some((
                    "importedValue",
                    source.find("importedValue } from").unwrap()
                ))
            ),
            // A global resolves to no declaration in this file's scope
            // tree, and a written or computed key is not a shorthand at
            // all. All three are the absence of a fact, never proof that
            // no binding exists.
            ("structuredClone", None),
            ("scoped: moduleValue", None),
            ("[moduleValue]: 1", None),
        ]
    );
}

#[test]
fn retains_untagged_template_interpolations() {
    let facts = extract(
        "App.tsx",
        "const label = `count is ${count} of ${total()}`;\n",
    )
    .unwrap();
    assert_eq!(facts.template_literals.len(), 1);
    let literal = &facts.template_literals[0];
    assert_eq!(literal.expressions.len(), 2);
    assert_eq!(
        literal
            .expressions
            .iter()
            .map(|slot| &"const label = `count is ${count} of ${total()}`;\n"
                [slot.start as usize..slot.end as usize])
            .collect::<Vec<_>>(),
        ["count", "total()"]
    );
}

/// A tagged template owns a template literal, but a tag receives the
/// values while an untagged literal stringifies them -- so a consumer
/// asking "is this interpolated into a string" must not be answered by a
/// tagged one.
#[test]
fn a_tagged_template_is_not_also_an_untagged_one() {
    let facts = extract("App.tsx", "const styled = css`color: ${theme}`;\n").unwrap();
    assert_eq!(
        facts.template_literals.len(),
        0,
        "a tag's quasi is filtered at collection; the table's untagged-only invariant holds"
    );
    // An untagged template nested inside a tagged interpolation is still
    // an untagged template — the manual visit must reach it.
    let facts = extract("App.tsx", "const styled = css`color: ${`x${theme}`}`;\n").unwrap();
    assert_eq!(facts.template_literals.len(), 1);
}

/// A fragment has no element fact — no name, no attributes — so it gets
/// its own span table, and a consumer asking "is this span inside JSX"
/// must consult both. Only the element table answered that question
/// before, which made fragment children invisible.
#[test]
fn fragments_are_recorded_beside_elements() {
    let facts = extract(
        "App.tsx",
        "const view = <>{outer()}<div>{inner()}</div></>;\n",
    )
    .unwrap();
    assert_eq!(facts.jsx_fragments.len(), 1);
    assert_eq!(facts.jsx_elements.len(), 1);
    let fragment = facts.jsx_fragments[0];
    let element = facts.jsx_elements[0].span;
    assert!(
        fragment.contains(element),
        "the fragment wraps the element: {fragment:?} vs {element:?}"
    );
}

#[test]
fn structured_returns_retain_tuple_slots_and_object_properties() {
    let source = r#"
function tuple() { return [store, accessor] as const; }
function object() { return { active: () => state(), pending: createMemo(() => state()) }; }
"#;
    let facts = extract("App.ts", source).unwrap();
    let tuple = facts
        .returns
        .iter()
        .find(|returned| !returned.elements().is_empty())
        .unwrap();
    assert_eq!(
        tuple
            .elements()
            .iter()
            .flatten()
            .map(|span| &source[span.start as usize..span.end as usize])
            .collect::<Vec<_>>(),
        ["store", "accessor"]
    );
    let object = facts
        .returns
        .iter()
        .find(|returned| !returned.properties().is_empty())
        .unwrap();
    assert_eq!(
        object
            .properties()
            .iter()
            .map(|property| property.name.as_str())
            .collect::<Vec<_>>(),
        ["active", "pending"]
    );
    let pending = object
        .properties()
        .iter()
        .find(|property| property.name == "pending")
        .unwrap();
    assert_eq!(
        source.get(pending.value.start as usize..pending.value.end as usize),
        Some("createMemo(() => state())")
    );
    assert!(facts.calls.iter().any(|call| call.span == pending.value));
}

/// Facts schema 45: every loop, and every suspension an `async` body can
/// take without an `await` expression, is recorded where it is written.
#[test]
fn records_loops_and_suspensions_without_an_await_expression() {
    let source = r#"
async function* body(items: AsyncIterable<number>, inner: () => AsyncGenerator<number>) {
  for (let i = 0; i < 1; i++) {}
  for (const key in {}) {}
  for (const item of [1]) {}
  while (false) {}
  do {} while (false);
  for await (const item of items) {}
  yield* inner();
  yield 1;
  await using resource = { async [Symbol.asyncDispose]() {} };
  await 0;
}
"#;
    let facts = extract("/project/body.ts", source).unwrap();
    let text = |span: &Span| &source[span.start as usize..span.end as usize];
    assert_eq!(
        facts
            .loop_statements
            .iter()
            .map(|span| text(span).split_whitespace().next().unwrap())
            .collect::<Vec<_>>(),
        ["for", "for", "for", "while", "do", "for"]
    );
    assert_eq!(
        facts
            .implicit_suspensions
            .iter()
            .map(text)
            .collect::<Vec<_>>(),
        [
            "for await (const item of items",
            "yield* inner()",
            "resource = { async [Symbol.asyncDispose]() {} }"
        ]
    );
    // A plain `yield` resumes inside its caller's step and is not one; an
    // `await` expression is its own fact.
    assert_eq!(
        facts.awaits.iter().map(text).collect::<Vec<_>>(),
        ["await 0"]
    );
}
