//! The AST fact schema: every fact struct and enum, `AstFacts` and its accessors, and `AstFactsError`.

use super::*;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AstFacts {
    pub schema: u32,
    pub source: SourceIdentity,
    pub calls: Vec<CallFact>,
    /// Host-sensitive execution, facts schema 57. Absent old tables confer no
    /// browser authority. Runtime constant values belong to the host consumer.
    #[serde(default)]
    pub host_execution: Vec<HostExecutionFact>,
    #[serde(default)]
    pub host_defaults: Vec<HostDefaultInitializer>,
    #[serde(default)]
    pub host_call_sequences: Vec<HostCallSequence>,
    #[serde(default)]
    pub host_empty_arrays: Vec<Span>,
    #[serde(default)]
    pub host_zero_values: Vec<Span>,
    /// Exact `void 0` runtime values. In particular, an unresolved identifier
    /// spelled undefined is not positive input-value evidence.
    #[serde(default)]
    pub host_undefined_arguments: Vec<Span>,
    pub bindings: Vec<BindingFact>,
    pub functions: Vec<FunctionFact>,
    /// Direct named function declarations, including overload signatures whose
    /// body is absent. [`FunctionFact`] deliberately models executable
    /// functions and therefore cannot distinguish a TypeScript overload
    /// signature from some unrelated exported declaration with the same name.
    /// Counting runtime export entries needs that exact distinction.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub function_declarations: Vec<FunctionDeclarationFact>,
    /// Class declarations and expressions. See [`ClassFact`]: a class is
    /// `typeof === "function"` at runtime but carries no call signature, so
    /// this is the only fact domain that can tell a consumer the runtime kind
    /// of an exported class.
    #[serde(default)]
    pub classes: Vec<ClassFact>,
    /// Runtime `namespace`/`module` declarations by their exact binding-name
    /// span. This is positive declaration-kind evidence; a containing module
    /// block is not equivalent because it may be nested inside another
    /// declaration.
    #[serde(default)]
    pub namespace_declarations: Vec<NamedSpan>,
    pub imports: Vec<ImportFact>,
    pub exports: Vec<ExportFact>,
    /// Every dynamic import or unshadowed CommonJS `require` call. A missing
    /// specifier is an explicit nonliteral load, never a complete-empty
    /// module census.
    #[serde(default)]
    pub module_loads: Vec<ModuleLoadFact>,
    /// Every comma expression, with the operand that is its value (facts
    /// schema 72). `(0, () => …)()` calls the literal the tail names.
    #[serde(default)]
    pub sequences: Vec<SequenceFact>,
    /// Every TypeScript `import S = require("…")` (facts schema 46): the one
    /// module load that is neither an `ImportDeclaration` nor a call, so
    /// neither [`AstFacts::imports`] nor [`AstFacts::module_loads`] records
    /// it. `S` binds the module's namespace object. The entity-name form
    /// (`import S = Other.Name`) loads no module and is not recorded.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub import_equals: Vec<ImportEqualsFact>,
    /// Every `import("…")` written in a type position (facts schema 46):
    /// `import("m").Name`, `typeof import("m").name`, `typeof import("m")`.
    /// Erased at run time and loading nothing, but resolved through the same
    /// module declarations an import is, so a consumer that counts type-only
    /// imports counts these too.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub type_imports: Vec<TypeImportFact>,
    /// TypeScript type queries, erased at runtime (facts schema 49).
    /// Runtime `typeof value` expressions are deliberately not recorded.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub type_queries: Vec<Span>,
    /// Type names written in a type position, erased at runtime (facts
    /// schema 64): the name of every type reference (`X.Props<T>`), every
    /// class `implements` clause and every interface `extends` clause. An
    /// `import A = X.Y` alias is a runtime binding and is not recorded.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub type_names: Vec<Span>,
    /// The tag expression of each tagged template (facts schema 48). The
    /// template calls its tag, with the member's object as `this` when the
    /// tag is a member expression; no call fact records that invocation.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tagged_template_tags: Vec<Span>,
    /// Syntax whose module/runtime reachability cannot be made finite from a
    /// local specifier graph. These rows are scope-resolved by Oxc's binder.
    #[serde(default)]
    pub module_hazards: Vec<ModuleHazardFact>,
    /// The body block of every `namespace`, `module`, or `declare global`
    /// declaration in this file, sorted by span.
    ///
    /// A declaration inside one of these blocks binds a *member of the
    /// namespace object*, never a name in the enclosing module's scope. So an
    /// `export` inside such a block is not an export of this module, and a
    /// declarator inside one is not part of this module's export surface:
    /// `export namespace Config { export const inner = 1 }` publishes `Config`
    /// alone, and `import { inner } from "…"` does not resolve. Consumers that
    /// enumerate the module's exports must filter with
    /// [`AstFacts::module_level_exports`]; see [`AstFacts::exported_bindings`].
    #[serde(default)]
    pub module_blocks: Vec<Span>,
    pub identifiers: Vec<IdentifierFact>,
    /// The binder's own resolution for each identifier *reference* that names a
    /// declaration in this file: `(reference span, declaration name span)`,
    /// sorted by reference span.
    ///
    /// This is the scope-exact answer, and it is deliberately not the
    /// compiler's. TypeScript answers a symbol query at a shorthand property
    /// span (`{ fn }`) with the *property's* symbol, never the value binding's,
    /// and a compiler entity exists only at a span some demand asked for.
    /// Neither limitation applies to Oxc's scope tree, and a reference to a
    /// parameter is always resolvable inside the function that declares it —
    /// which is what a proof of "what does this export do with the value its
    /// caller passed" has to be able to ask.
    ///
    /// A reference that is absent resolves to no declaration in this file — a
    /// global, or an unresolved name. That is the absence of a fact, never
    /// proof that no declaration exists. It is a side table rather than a field
    /// on [`IdentifierFact`] because identifiers are the highest-cardinality
    /// fact in the file and that struct is kept to two words.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reference_declarations: Vec<(Span, Span)>,
    pub awaits: Vec<Span>,
    /// The subset of [`AstFacts::awaits`] proven to execute on every run of
    /// their innermost enclosing function (or of module evaluation): awaits
    /// with no conditional, logical, loop, switch, or try construct between
    /// the function entry and the expression. Code positioned after one of
    /// these is await-dominated in the straight-line sense — the conservative
    /// core the after-await member-read check builds on.
    #[serde(default)]
    pub unconditional_awaits: Vec<Span>,
    /// `(await, region)`: an await written at statement level of its
    /// innermost block (no conditional, logical, loop, switch or try construct
    /// between the block's start and the expression, not in a skippable
    /// chain), and the rest of that block. A block is entered only at its
    /// start, so code in the region runs only after that await suspended.
    /// Unlike [`Self::unconditional_awaits`] the block may be a `try` block or
    /// a branch's block; a `catch` or `finally` is never in the region, since
    /// a synchronous throw of the operand reaches it unsuspended.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dominating_awaits: Vec<(Span, Span)>,
    /// A `try` statement whose block's first statement is an await
    /// (`await e;`, `const x = await e;`, `return await e;`): the await, its
    /// operand, and the `catch` and `finally` blocks. If evaluating the
    /// operand cannot throw synchronously, nothing in the `try` block can run
    /// before the await suspends, so those blocks run only after it did.
    /// Whether the operand can throw is the consumer's question.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub try_head_awaits: Vec<TryHeadAwaitFact>,
    /// The `import.meta.glob(…)` and `import.meta.globEager(…)` calls that the
    /// bundler may turn into static imports run in place (facts schema 70):
    /// every call except `glob` with no options, or with an options object
    /// literal (no spread, no computed key) that has no `eager` key or has
    /// `eager: false`. The matched modules are unknown, so the module holding
    /// one may sit on an import cycle. A lazy glob is a dynamic import and
    /// is not recorded.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub in_place_import_globs: Vec<Span>,
    /// Every reference to the unresolved global `require` that is not the
    /// callee of a call, once parentheses and TypeScript wrappers (`as`,
    /// `satisfies`, `!`, `<T>`) are peeled (facts schema 69): `(0, require)(x)`,
    /// `const r = require`, `typeof require`, `require.resolve`. Such a
    /// reference may load a module in place with no recorded load, so the
    /// module holding one may sit on an import cycle.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unmodeled_require_references: Vec<Span>,
    /// The call expressions written in their innermost enclosing function's
    /// straight-line flow, as [`Self::unconditional_awaits`] are, and outside
    /// any optional chain (`a?.b(c())` evaluates `c()` only when `a` is not
    /// nullish). Such a call runs on every execution of that function that
    /// reaches its position.
    #[serde(default)]
    pub straight_line_calls: Vec<Span>,
    /// The `new` expressions written in straight-line flow under the same
    /// conditions as [`Self::straight_line_calls`] (facts schema 58). Kept
    /// apart so that table's existing consumers keep their meaning.
    #[serde(default)]
    pub straight_line_constructs: Vec<Span>,
    /// The call, `new` and member expressions written in their innermost
    /// enclosing function's straight-line flow, outside any logical
    /// assignment and after no optional-chain link, that no earlier `return`,
    /// `break`, `continue` or `throw` of that function may skip (facts schema
    /// 59, ADR 0276). Such a site runs on every normal
    /// completion of that function that reaches it without suspending; JSX,
    /// classes, binding patterns and parameter lists are left to the reader.
    #[serde(default)]
    pub completion_sites: Vec<Span>,
    pub returns: Vec<ReturnFact>,
    pub jsx_elements: Vec<JsxElementFact>,
    /// JSX fragment spans (`<>…</>`). A fragment's children are as tracked
    /// as an element's, but it has no name and no attributes, so it gets a
    /// bare span rather than a [`JsxElementFact`]; consumers asking "is this
    /// span inside JSX" must consult both tables.
    #[serde(default)]
    pub jsx_fragments: Vec<Span>,
    /// Transparent TypeScript expression wrappers and the expression they
    /// contain. Consumers use this table instead of reproducing parser-node
    /// knowledge at each span-equality gate.
    #[serde(default)]
    pub transparent_wrappers: Vec<TransparentWrapperFact>,
    pub members: Vec<MemberFact>,
    #[serde(default)]
    pub computed_members: Vec<Span>,
    /// Every array literal (`[a, ...b]`), sorted by span. An array literal
    /// evaluates to a fresh `Array`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub array_literals: Vec<Span>,
    /// The subset of [`AstFacts::computed_members`] whose key is a literal
    /// naming exactly one property, sorted by member span: a string literal
    /// or substitution-free template (its cooked value, lone surrogates
    /// refused), under transparent TypeScript wrappers, or a numeric literal whose
    /// value is a non-negative integer no larger than `i32::MAX` (its
    /// canonical decimal spelling, which is the property key ToPropertyKey
    /// gives it). The same key the Type Facts producer's
    /// `literalElementAccessSegment` roots a callee at, so the generator can
    /// name `handler[0]` as the member the census sees (item B of
    /// ways-to-improve § 3.3). Absence names nothing: a computed key the table
    /// does not hold is unknown, never "no member".
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub literal_computed_members: Vec<LiteralMemberKeyFact>,
    /// The spans of the member links written `?.`, sorted (facts schema 44):
    /// `p?.key` and `p?.[0]`, never a later link of the same chain. What the
    /// generator's `returns` walk reads to propose "the member, or undefined"
    /// for a returned optional read of a parameter (item B round 2 of
    /// ways-to-improve § 3.3); the census decides it from the producer's arms.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub optional_members: Vec<Span>,
    #[serde(default)]
    pub parameter_properties: Vec<Span>,
    pub spreads: Vec<SpreadFact>,
    pub conditional_tests: Vec<Span>,
    /// Exact operands of equality tests against `null` or the binder-proven
    /// global `undefined`, or the left operand of a discarded `??` expression.
    /// These observations neither coerce nor escape the operand.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub nullish_test_operands: Vec<Span>,
    #[serde(default)]
    pub conditional_expressions: Vec<ConditionalExpressionFact>,
    #[serde(default)]
    pub logical_expressions: Vec<LogicalExpressionFact>,
    #[serde(default)]
    pub object_properties: Vec<ObjectPropertyFact>,
    /// Descriptor-aware literal shapes for per-occurrence package Gets.
    #[serde(default)]
    pub object_get_shapes: Vec<ObjectGetShapeFact>,
    #[serde(default)]
    pub template_literals: Vec<TemplateLiteralFact>,
    /// Operands in TypeScript-valid coercions where a function object is
    /// provably observed as a value: string-literal concatenation, logical
    /// not, and the unary numeric coercions. *Binary* arithmetic and bitwise
    /// positions are deliberately absent because TypeScript rejects a function
    /// operand there (`f + 1` is TS2365, `f - 1`/`f * 2`/`f | 0` are TS2362,
    /// probed against solid-js@1.9.14 in both passes). The unary forms are
    /// not: `-f`, `+f`, and `~f` are all accepted, so a coerced accessor there
    /// is this checker's to report.
    #[serde(default)]
    pub coercive_operands: Vec<CoerciveOperandFact>,
    /// Every operand span of an operator that applies ToPrimitive to it, by
    /// span, sorted: the binary and compound-assignment operators the Type
    /// Facts producer's `coercingBinaryOperators` lists (arithmetic, the four
    /// relational comparisons, loose equality, shifts and bitwise operators,
    /// and each compound assignment of one), the prefix `+ - ~`, the prefix
    /// and postfix `++ --`, and each substitution of an untagged template
    /// literal.
    ///
    /// A different fact from [`AstFacts::coercive_operands`], deliberately:
    /// that table is about positions where TypeScript *accepts* a function
    /// operand, so it omits binary arithmetic, and it includes `!` (ToBoolean,
    /// which runs no user code). This one is about the runtime protocol --
    /// which operands can reach a value's `Symbol.toPrimitive`, `valueOf` or
    /// `toString` -- and is read to describe a coercion of a caller's
    /// argument, never by a rule.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub coercing_operands: Vec<Span>,
    /// Every operator expression whose value is a primitive whatever its
    /// operands are, by span, sorted (facts schema 64): arithmetic, shift,
    /// bitwise, relational, equality, `in` and `instanceof` binaries; the
    /// prefix `+ - ~ ! typeof delete`; and `++`/`--`. Not `void` (it yields
    /// `undefined`, which selects a parameter default) nor a logical or
    /// conditional operator (it yields one of its operands).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub primitive_results: Vec<Span>,
    /// Every value the iteration protocol is driven over, by span, sorted: the
    /// iterated operand of a `for…of` (and `for await…of`), of an array or
    /// argument spread (an object spread is a property enumeration, not an
    /// iteration, and is absent), of `yield*`, and the initializer of an array
    /// binding pattern or the right side of an array assignment pattern.
    ///
    /// Read only to decline describing a callback enumeration it could not
    /// describe whole: no summary derives an `iterate` item, so an iteration
    /// of a caller's value leaves the enumeration undescribable.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub iterated_operands: Vec<Span>,
    #[serde(default)]
    pub assignments: Vec<AssignmentFact>,
    /// Exact simple assignment targets, including destructuring leaves,
    /// update operands and iteration targets. Keys and defaults are reads.
    /// Transparent TypeScript wrappers are peeled (facts schema 53).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub write_targets: Vec<Span>,
    /// Exact operands of delete expressions, independently of assignments.
    #[serde(default)]
    pub deleted_targets: Vec<Span>,
    #[serde(default)]
    pub if_regions: Vec<IfRegionFact>,
    /// Every `break` and `continue` statement, by span, sorted.
    ///
    /// **No consumer reads this today.** It was added for the `creates`
    /// implementation census, which refused any declaration node containing a
    /// jump: the Type Facts producer *withheld* every call row inside the
    /// region a jump makes non-universal, and for a claim of the absence of
    /// behavior a withheld row is a silence indistinguishable from the absence
    /// of the call. Since handshake protocol 15 the producer states such a row
    /// with `reach: unknown` and classifies the enclosing construct, so the
    /// census reads the row instead — a producer fact in place of a syntactic
    /// approximation that could see the jump but never which rows it touched
    /// (Type Facts ADR 0026 amendment).
    ///
    /// Retained rather than removed because it is a faithful syntactic fact and
    /// its extraction is two pushes; it is a removal candidate. Any new consumer must state its own premise:
    /// a jump in a span says nothing about what any other fact domain withheld.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub jump_statements: Vec<Span>,
    /// The left side of every `for (target in …)` / `for (target of …)` whose
    /// target is an *assignment* to an existing binding rather than a
    /// declaration, by span, sorted. Each iteration writes the target, so it
    /// is a write of every binding it names, exactly as an
    /// [`AssignmentFact`] is; it lives apart from [`AstFacts::assignments`]
    /// because that table's consumers read a value provenance a loop head does
    /// not have.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub iteration_targets: Vec<Span>,
    /// The span of every iteration statement -- `for`, `for … in`,
    /// `for … of` (including `for await`), `while` and `do … while` -- sorted
    /// (facts schema 45). The only construct that can run code positioned
    /// earlier in a function body again after code positioned later: without
    /// one, a function body's own statements execute in source order. What
    /// the action-step proof (SC2006) reads to refuse a call a loop can reach
    /// again after a suspension.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub loop_statements: Vec<Span>,
    /// `(loop, repeated)`: the part of a loop statement that runs again after
    /// its body finished (facts schema 73). A `for` loop's initializer runs
    /// once, and a `for…of` or `for…in` iterable is evaluated once before the
    /// first iteration, so both are left out; the left-hand target of those
    /// loops is re-evaluated each iteration and is kept. An await anywhere in
    /// the loop may run before code in the repeated part on a later iteration.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub loop_repeats: Vec<(Span, Span)>,
    /// `(pattern, value)`: a destructuring declarator's or assignment's
    /// pattern and the value it destructures (facts schema 73). The pattern's
    /// defaults run after the value is evaluated, so a read there follows an
    /// await in the value even when it is written first.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pattern_values: Vec<(Span, Span)>,
    /// The points where an `async` function or generator suspends without an
    /// [`AstFacts::awaits`] expression, by span, sorted (facts schema 45):
    ///
    /// - a `for await (… of …)` statement, which awaits every iterator result
    ///   before its body runs;
    /// - a delegating `yield* operand` (in an async generator it awaits every
    ///   result of the delegate; in a sync generator it does not suspend the
    ///   caller's step, and a consumer must tell the two apart by the
    ///   function's own `async` flag);
    /// - an `await using` declaration, which awaits its disposal when the
    ///   enclosing block exits.
    ///
    /// Recorded for the function the construct is written in, never for a
    /// nested one; like `awaits`, a consumer attributes each span to its
    /// innermost enclosing function.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub implicit_suspensions: Vec<Span>,
    /// Module-level string directives (`"use server"`, `"use strict"`, …):
    /// the statements the parser classifies as the module's directive
    /// prologue, in source order, carrying the cooked directive text. A
    /// string literal after the first non-directive statement is an ordinary
    /// expression statement and is deliberately absent.
    #[serde(default)]
    pub module_directives: Vec<DirectiveFact>,
    /// Every statement that spells a string directive without being one
    /// (facts schema 75): a string-literal expression statement the parser
    /// did not classify as part of a directive prologue, and a
    /// substitution-free template literal statement, which is never a
    /// directive. In source order. A leading string statement of a program or
    /// function body is a [`DirectiveFact`] (module) or in
    /// [`FunctionFact::directives`], not here.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stray_directives: Vec<StrayDirectiveFact>,
    /// Comments that switch rules off or on (facts schema 76): the
    /// `eslint-disable` family and the checker's own `solid-checker-disable`
    /// family, each parsed with ESLint's directive grammar. In source order.
    /// Every other comment is absent.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub suppression_comments: Vec<SuppressionCommentFact>,
    /// Code no execution reaches, from the syntax alone (facts schema 77, ADR
    /// 0458): every statement after one that never completes normally in the
    /// same list (a `function` declaration excepted, it is hoisted), and every
    /// arm a literal test rules out (`if`, `?:`, `while`, `for`, the right of
    /// `&&`, `||`, `??`). Sorted; regions may nest. Everything written inside
    /// one, nested functions and classes included, never runs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unreachable_regions: Vec<Span>,
    /// Every tagged template, with its substitutions in order (facts schema
    /// 61, ADR 0309). The template calls its tag with the template object
    /// as argument 0 and substitution `i` as argument `i + 1`; no call fact
    /// records that invocation, and [`Self::template_literals`] deliberately
    /// excludes tagged quasis.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tagged_templates: Vec<TaggedTemplateFact>,
    /// Every array literal's elements in order, holes and spreads included
    /// (facts schema 62, ADR 0309). [`Self::array_literals`] keeps the bare
    /// spans its consumers read.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub array_literal_elements: Vec<ArrayLiteralFact>,
    /// Every element of every array or object destructuring pattern, in
    /// binding and assignment position alike (facts schema 62, ADR 0309):
    /// which slot it projects, what it binds, and its default initializer.
    /// A formal parameter's own default is its [`BindingFact::initializer`];
    /// a default nested inside a parameter's pattern is only here.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pattern_elements: Vec<PatternElementFact>,
    /// The operand of every `await` expression (facts schema 62, ADR 0309).
    /// Awaiting a value Gets its `then` property.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub await_operands: Vec<Span>,
    /// The expression of every JSX expression-container child, in elements
    /// and fragments alike (facts schema 62, ADR 0309). Empty containers
    /// (`{}` and comments) are absent. [`JsxElementFact::children`] holds the
    /// container spans, which include the braces.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub jsx_child_expressions: Vec<Span>,
    /// The initializer of every `using` and `await using` declarator (facts
    /// schema 62, ADR 0309). The declaration Gets the value's
    /// `Symbol.dispose` (or `Symbol.asyncDispose`) method and calls it when
    /// the enclosing block exits: a protocol use no reference spells.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub disposal_initializers: Vec<Span>,
    #[serde(skip, default)]
    pub span_index: LazySpanIndex,
}

/// A tagged template expression (facts schema 62).
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaggedTemplateFact {
    pub span: Span,
    pub tag: Span,
    /// The interpolated expressions, in order: arguments 1 onward of the
    /// tag's invocation.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub substitutions: Vec<Span>,
}

/// An array literal and its elements (facts schema 62).
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArrayLiteralFact {
    pub span: Span,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub elements: Vec<ArrayElementFact>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArrayElementFact {
    pub kind: ArrayElementKind,
    /// The element's value; a spread's argument; a hole's elision.
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ArrayElementKind {
    Value,
    Spread,
    Hole,
}

/// One element of a destructuring pattern (facts schema 62).
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatternElementFact {
    /// The array or object pattern (or assignment target) the element is in.
    pub pattern: Span,
    pub slot: PatternSlot,
    /// An object pattern property's key; `None` in an array pattern and for
    /// a rest element.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<Span>,
    /// What the slot binds or assigns: an identifier, a member target, or a
    /// nested pattern. A shorthand property's key is its target.
    pub target: Span,
    /// The default initializer, evaluated only when the projected value is
    /// `undefined`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<Span>,
    /// An assignment target pattern (`[a] = xs`) rather than a binding.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub assignment: bool,
}

/// Which value a pattern element projects.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "kebab-case")]
pub enum PatternSlot {
    /// An array pattern position before any rest element.
    Index(u32),
    /// An object pattern key the syntax states.
    Key(CompactString),
    /// An object pattern key computed at run time.
    ComputedKey,
    /// The rest element: the remaining iteration results, or the remaining
    /// own enumerable properties.
    Rest,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CallFact {
    pub span: Span,
    pub callee: Span,
    pub direct_callee: bool,
    pub type_arguments: bool,
    pub arguments: Vec<ArgumentFact>,
    pub static_callee: bool,
    /// Whether this call's own result provably reaches nothing: the call
    /// **is** the expression of an `ExpressionStatement`.
    ///
    /// One direction only. `true` proves the result is discarded; `false`
    /// proves nothing and is the default, so a consumer that needs "the
    /// result is used" reads `!result_discarded` and stays fail-closed on
    /// every form this does not classify. `await f()` and `void f()` are
    /// deliberately `false`: the statement's expression is the `await` or the
    /// unary, and the call's result reaches *it*.
    #[serde(default)]
    pub result_discarded: bool,
    /// Whether this is a `new` expression rather than a call. Both are
    /// recorded here, because both run the callee; a consumer describing a
    /// *call* of a member (item B of ways-to-improve § 3.3) asks this first.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub construct: bool,
}

impl CallFact {
    /// Returns the callee text only when extraction certified it as a static
    /// identifier or dotted identifier path.
    #[must_use]
    pub fn static_callee<'s>(&self, source: &'s str) -> Option<&'s str> {
        self.static_callee
            .then_some(self.callee)
            .and_then(|span| source.get(span.start as usize..span.end as usize))
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArgumentFact {
    pub span: Span,
    pub spread: bool,
    pub value: ArgumentValueKind,
    pub boolean_properties: Vec<BooleanPropertyFact>,
    pub identifier_properties: Vec<NamedSpan>,
    /// Statically-named properties of an object-literal argument whose value
    /// is a string literal, carrying the cooked string value.
    #[serde(default)]
    pub string_properties: Vec<StringPropertyFact>,
    /// The key spans of every statically-named data property when the
    /// argument is an object literal, regardless of value shape. Presence in
    /// this list proves `"name" in options` at runtime; absence proves
    /// nothing unless [`ArgumentFact::exact_object_literal`] also holds.
    #[serde(default)]
    pub property_names: Vec<Span>,
    /// True when the argument is an object literal whose complete property
    /// set is statically known: every property is a plain data property with
    /// a static identifier key — no spreads, no computed keys, no accessors.
    /// Only such a literal proves the *absence* or the *final value* of an
    /// option.
    #[serde(default)]
    pub exact_object_literal: bool,
    /// True when the argument is an object literal with a closed, statically
    /// named property set. Unlike `exact_object_literal`, accessors are
    /// allowed because they still cannot add a hidden spread/computed key;
    /// consumers may prove key absence but must not infer final property
    /// values from this flag.
    #[serde(default)]
    pub closed_object_literal: bool,
    /// Normalized runtime shape underneath transparent TypeScript wrappers.
    /// This is shared by arguments and object-property values so consumers do
    /// not rebuild a partial literal taxonomy from source text.
    #[serde(default)]
    pub runtime_value_kind: RuntimeValueKind,
    /// Declaration this argument's identifier refers to, as resolved by the
    /// binder for that exact reference.
    ///
    /// The same guarantee as [`ObjectPropertyFact::shorthand_binding`]: Oxc's
    /// scope tree resolved the reference, so the declaration it chose is
    /// recorded rather than left to be re-derived from the spelling. It exists
    /// so a *demand* can follow `save(payload)` to the literal `payload` was
    /// built from without either resolving names by text or sweeping every
    /// binding in the file. A consumer proving something still resolves the
    /// symbol semantically; this only decides what to ask the compiler about.
    ///
    /// `None` for a non-identifier argument, and for an identifier the binder
    /// resolves to no declaration in this file's scope tree — an import or a
    /// global. Consumers fail closed on `None`; it is the absence of a fact,
    /// never proof of a missing binding.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub binding_declaration: Option<Span>,
    /// The span of the runtime value expression when TypeScript sugar
    /// (parentheses, `as`, `satisfies`, `!`) wraps it; `None` when the
    /// argument span already is the runtime value.
    #[serde(default)]
    pub value_span: Option<Span>,
    /// True when a TypeScript assertion or non-null assertion can make the
    /// static call valid without changing the runtime value. Parentheses and
    /// `satisfies` are transparent too, but cannot launder an invalid value.
    #[serde(default)]
    pub runtime_type_escape: bool,
    /// The members of an array or object literal written as this argument
    /// (after transparent TypeScript wrappers), each keyed by the property key
    /// the runtime gives it, when that set is statically exact: an array
    /// literal's elements up to its first spread (a hole is skipped and still
    /// counts its index), and an object literal's data properties when every
    /// property has a static identifier, string or canonical integer key and
    /// the literal has no spread, accessor, computed key or `__proto__` key
    /// (a later duplicate key wins, as it does at runtime). Empty for any
    /// other argument, which names no member: absence is unknown, never "no
    /// such member". Read by the consumer to resolve a member-path
    /// `callbacks` row (`handler[0]` of `[readCount, data]`, item B of
    /// ways-to-improve § 3.3) to the value it invokes, and by nothing else.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub literal_members: Vec<ArgumentMemberFact>,
    /// Exact runtime literals written at this argument, underneath transparent
    /// TypeScript wrappers. Identifiers, getters, spreads and computed keys
    /// yield unknown facts, never a type-based claim about runtime values.
    #[serde(default)]
    pub literal_value: ArgumentLiteralFact,
}

/// A normalized literal value; no parser node crosses the fact boundary.
/// Objects contain only their final own data properties. Arrays state length
/// only without spreads (holes still contribute to length). Nested literals
/// stop at depth 32; an unknown child does not erase known sibling facts.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "kebab-case")]
pub enum ArgumentLiteralFact {
    #[default]
    Unknown,
    Null,
    Boolean(bool),
    Integer(u32),
    String(CompactString),
    Function,
    Object(Vec<ArgumentLiteralPropertyFact>),
    ArrayLength(u32),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArgumentLiteralPropertyFact {
    pub name: CompactString,
    pub value: ArgumentLiteralFact,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ArgumentValueKind {
    Undefined,
    Null,
    Identifier,
    Function,
    AsyncFunction,
    Other,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeValueKind {
    #[default]
    Unknown,
    Nullish,
    Primitive,
    Function,
    Object,
    Array,
}

impl RuntimeValueKind {
    #[must_use]
    pub const fn is_proven_noncallable(self) -> bool {
        matches!(
            self,
            Self::Nullish | Self::Primitive | Self::Object | Self::Array
        )
    }

    /// A literal carrying data rather than behavior: a primitive, object, or
    /// array literal. This is [`Self::is_proven_noncallable`] without the
    /// nullish arm, which callers keep separate because `null`/`undefined`
    /// answer a different question — absence, rather than a wrong value.
    #[must_use]
    pub const fn is_data_literal(self) -> bool {
        matches!(self, Self::Primitive | Self::Object | Self::Array)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BooleanPropertyFact {
    pub name: Span,
    pub value: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StringPropertyFact {
    pub name: Span,
    pub value: CompactString,
}

/// One string directive from a directive prologue (module or function body),
/// carrying its span and cooked text.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectiveFact {
    pub span: Span,
    pub value: CompactString,
}

/// A statement that reads like a string directive but is not one (facts
/// schema 75). The directive pass only reads a program's or function body's
/// prologue, so such a statement is an ordinary expression.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StrayDirectiveFact {
    /// The whole expression statement.
    pub span: Span,
    /// The string's cooked text.
    pub value: CompactString,
    /// A substitution-free template literal rather than a string literal.
    pub template: bool,
    /// Whether the statement is a direct child of the program or of a
    /// function body: the statement lists that have a directive prologue. A
    /// statement in a block, a `switch` case or a class static block is not.
    pub direct: bool,
}

/// Which comment family a [`SuppressionCommentFact`] belongs to.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SuppressionOrigin {
    /// `eslint-disable`, `eslint-enable`, `eslint-disable-line` and
    /// `eslint-disable-next-line`.
    Eslint,
    /// The same four, spelled `solid-checker-*`.
    SolidChecker,
}

/// What a suppression comment does, by its label.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SuppressionAction {
    Disable,
    Enable,
    DisableLine,
    DisableNextLine,
}

/// One comment that switches rules off or on (facts schema 76). Only the
/// syntax is recorded: which rule a name denotes, and which findings it
/// reaches, is the consumer's. A `//` comment is kept only for the two
/// line-scoped actions, as in ESLint, and the justification after ` -- ` is
/// dropped.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SuppressionCommentFact {
    /// The whole comment, delimiters included.
    pub span: Span,
    pub origin: SuppressionOrigin,
    pub action: SuppressionAction,
    /// A `/* */` comment rather than a `//` comment.
    pub block: bool,
    /// The comma-separated names as written, trimmed and with one pair of
    /// surrounding quotes removed; empty when the comment names none, which
    /// means every rule.
    pub names: Vec<CompactString>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BindingShape {
    Identifier,
    Array,
    Object,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BindingFact {
    pub declaration: Span,
    pub pattern: Span,
    pub shape: BindingShape,
    pub names: Vec<NamedSpan>,
    pub array_slots: Vec<Option<NamedSpan>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub object_slots: Vec<ObjectBindingFact>,
    /// Whether this binding is a `const` variable declarator. Parameters and
    /// mutable declarations are false.
    #[serde(default)]
    pub immutable: bool,
    /// A non-ambient lexical runtime `let`, outside namespace lowering.
    /// Missing ambient initializers never establish an undefined value.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub runtime_let: bool,
    /// Whether this declarator is written directly in a `switch` case clause
    /// (facts schema 67). All clauses share one lexical scope, so a jump to a
    /// later clause enters it without running the declarator, and a read
    /// there throws `ReferenceError`. A declaration in a block inside a
    /// clause (`case 1: { const x }`) is not this.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub in_switch_case: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initializer: Option<Span>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub call_initializer: Option<Span>,
    pub initializer_function: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initializer_identifier: Option<NamedSpan>,
    /// The normalized runtime shape of the initializer underneath transparent
    /// TypeScript wrappers, as [`ArgumentFact::runtime_value_kind`] gives it
    /// for an argument: `const items = [a, b]` is `Array`.
    #[serde(default)]
    pub initializer_value_kind: RuntimeValueKind,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectBindingFact {
    pub property: CompactString,
    pub local: NamedSpan,
    /// An unescaped static identifier key, rather than a quoted/computed key.
    /// Absent older facts confer no permission for plain-identifier transport.
    #[serde(default)]
    pub plain_identifier_key: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FunctionKind {
    Declaration,
    Expression,
    Arrow,
}

/// A class declaration or class expression, and the span of the name it binds.
///
/// This exists because *constructability is not callability*. The compiler's
/// `Callability` is derived from `GetSignaturesOfType(…, SignatureKindCall)`
/// alone, so `typeof C` for a class answers `nonCallable` — it has construct
/// signatures and no call signature. At runtime `typeof C === "function"` for
/// every class, which is exactly what a package export's `kind` describes
/// and exactly what a runtime probe measures. Nothing in the type facts can
/// close that gap, so the syntactic fact is the one that has to.
///
/// `name` is `None` for an anonymous class expression (`export default class
/// {}`), whose binding is the export itself.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassFact {
    pub span: Span,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<NamedSpan>,
    /// The `extends` expression, when the class has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heritage: Option<Span>,
    /// Every element of the class body, in source order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub elements: Vec<ClassElementFact>,
}

/// One element of a class body: what it defines, under which key, on the
/// instance or the constructor.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassElementFact {
    pub span: Span,
    pub kind: ClassElementKind,
    /// The key's span; `None` for a computed key, a private name, a static
    /// block or an index signature.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<Span>,
    #[serde(default)]
    pub computed: bool,
    #[serde(default)]
    pub r#static: bool,
    /// The method's function, or the field's initializer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<Span>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ClassElementKind {
    Constructor,
    Method,
    Getter,
    Setter,
    /// A field or auto-accessor: an own property of every instance.
    Field,
    StaticBlock,
    IndexSignature,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FunctionFact {
    pub span: Span,
    pub body: Span,
    pub kind: FunctionKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<NamedSpan>,
    /// The property name for an object-literal or class method. This is kept
    /// separate from `name`: a method is not a lexical binding, but its
    /// canonical property symbol is still the exact callee target for
    /// interprocedural summaries.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub method_name: Option<NamedSpan>,
    pub parameters: Vec<BindingFact>,
    /// Whether the declaration ends in a rest parameter (`...rest`). The rest
    /// binding is deliberately *not* one of `parameters`: it has no single
    /// argument index. It absorbs every argument from index
    /// `parameters.len()` onward, so a consumer reasoning about argument slots
    /// must treat that tail as observable rather than as unnamed.
    #[serde(default)]
    pub rest_parameter: bool,
    /// Whether evaluating the parameter list can run no user code and so
    /// cannot throw before the body does (facts schema 70): every default
    /// initializer, at any depth of a destructuring pattern, is a primitive
    /// literal, a function or arrow, an empty object or array literal or the
    /// global `undefined`, no computed key is written, and no parameter has a
    /// decorator. Absent (an older fact) is false: the list may throw.
    #[serde(default)]
    pub parameters_inert: bool,
    /// The binding identifiers a rest parameter declares.
    ///
    /// Kept out of `parameters` for the reason above — the tail fills no single
    /// argument slot — but a consumer proving what an exported function does
    /// with the values its caller supplied still has to resolve references to
    /// it. Empty when there is no rest parameter.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rest_parameter_names: Vec<NamedSpan>,
    pub r#async: bool,
    pub generator: bool,
    pub expression_body: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expression_return: Option<ReturnFact>,
    /// The string directives of this function body's directive prologue
    /// (`"use server"`, …), in source order. Always empty for
    /// expression-bodied arrows, which have no statement prologue.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub directives: Vec<CompactString>,
    /// The span of each entry of `directives`, in the same order (facts
    /// schema 75).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub directive_spans: Vec<Span>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FunctionDeclarationFact {
    pub name: NamedSpan,
    pub has_body: bool,
}

impl FunctionFact {
    /// Where the parameter list begins: the first parameter's start, `None`
    /// for a function with no parameter (its list holds no code). The list
    /// runs to the body; what lies between the last parameter and the body
    /// (`)`, a return type, `=>`) is no code. A site in the list runs only when
    /// a caller leaves the argument out (ADR 0454).
    pub fn parameter_list_start(&self) -> Option<u32> {
        self.parameters
            .iter()
            .map(|parameter| parameter.declaration.start)
            .chain(self.rest_parameter_names.iter().map(|name| name.span.start))
            .min()
    }

    /// Whether this function's own directive prologue contains `directive`.
    #[must_use]
    pub fn has_directive(&self, directive: &str) -> bool {
        self.directives.iter().any(|value| value == directive)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NamedSpan {
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransparentWrapperFact {
    pub span: Span,
    pub inner: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ImportKind {
    SideEffect,
    Named,
    Default,
    Namespace,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportBindingFact {
    pub kind: ImportKind,
    pub local: NamedSpan,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub imported: Option<CompactString>,
    pub type_only: bool,
    /// Whether Oxc resolved at least one runtime-value reference to this
    /// binding. A syntactic value import used only in type positions is
    /// normally erased by TypeScript and therefore does not load its module.
    #[serde(default)]
    pub runtime_referenced: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportFact {
    pub span: Span,
    pub module: CompactString,
    /// The module specifier literal's own span, quotes included (facts
    /// schema 63). `span` is the whole declaration, which can hold other
    /// strings with the same text (`import { "m" as x } from "m"`). `None`
    /// only for a fact decoded without the field: its literal's location is
    /// unknown, which says nothing about whether the import exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub specifier_span: Option<Span>,
    pub type_only: bool,
    pub bindings: Vec<ImportBindingFact>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ModuleLoadKind {
    DynamicImport,
    Require,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModuleLoadFact {
    pub span: Span,
    pub kind: ModuleLoadKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub specifier: Option<CompactString>,
    /// The runtime argument literal's own span when `specifier` is known
    /// (facts schema 50). A type argument such as
    /// `require<typeof import("x")>("x")` may hold another literal with the
    /// same text, so a resolution is joined to this span, not to `span`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub specifier_span: Option<Span>,
    /// The named reads the program makes of a dynamic import's result (facts
    /// schema 72). `None` when the result escapes unnamed, or is read in a
    /// shape this fact does not name. A `require` never has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub consumer: Option<ModuleLoadConsumer>,
}

/// The named reads of a dynamic import's result, as the program spells them
/// (facts schema 72). Each names the exports the program can reach through it;
/// whether they are Solid's reads is for the reactive IR to decide.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ModuleLoadConsumer {
    /// `const { a, b: c } = await import(x)` and `(await import(x)).a`: the
    /// export names read, and nothing else of the module's namespace.
    Members { names: Vec<CompactString> },
    /// `lazy(() => import(x), options)`: `call` is the `lazy` call, and
    /// `export` the one export its options name (`default` when they name
    /// none). Solid's `lazy` reads only that export.
    Lazy { call: Span, export: CompactString },
}

/// A comma expression and the operand whose value it is (facts schema 72).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SequenceFact {
    pub span: Span,
    /// The last operand: the value of the whole expression.
    pub tail: Span,
}

/// `import S = require("m")`, `import type S = require("m")`, or either
/// exported (`export import S = require("m")`).
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportEqualsFact {
    /// The whole declaration, which contains the specifier literal.
    pub span: Span,
    /// The specifier, cooked.
    pub module: CompactString,
    /// The specifier literal's own span, quotes included (facts schema 63);
    /// `None` only for a fact decoded without the field, whose literal's
    /// location is unknown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub specifier_span: Option<Span>,
    /// The binding `S`, whose references the binder resolves to this span.
    pub local: NamedSpan,
    pub type_only: bool,
    /// `export import S = require("m")`: the namespace object is also one of
    /// this module's exports.
    pub exported: bool,
}

/// `import("m")` in a type position, with the first name its qualifier
/// reads: `Name` in `import("m").Name.Inner` and in
/// `typeof import("m").Name`, `None` for a bare `typeof import("m")`, whose
/// type is the whole namespace.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeImportFact {
    /// The import type, which contains the specifier literal.
    pub span: Span,
    /// The specifier, cooked.
    pub module: CompactString,
    /// The specifier literal's own span, quotes included (facts schema 63).
    /// `span` also covers the qualifier and any type arguments, which may
    /// hold another import type with the same text. `None` only for a fact
    /// decoded without the field, whose literal's location is unknown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub specifier_span: Option<Span>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub member: Option<CompactString>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ModuleHazardKind {
    NonliteralDynamicLoading,
    Eval,
    OpaqueWasm,
    MutableUnboundGlobal,
    /// A property accessor installed at run time: a `Proxy` trap, a getter
    /// descriptor, or a prototype swapped for one that carries either.
    ///
    /// The point of the fact is what it makes *invisible*. A declared getter
    /// is seen — the producer records a `get-accessor` form for it — but an
    /// accessor installed at run time leaves the receiver's declared type
    /// saying data property, so a read through it records nothing at all and
    /// no census can refuse what it cannot see. This hazard is that premise,
    /// stated syntactically at the one place it is still visible.
    RuntimeAccessorInstallation,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModuleHazardFact {
    pub span: Span,
    pub kind: ModuleHazardKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExportKind {
    Named,
    Default,
    All,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportFact {
    pub span: Span,
    pub kind: ExportKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub module: Option<CompactString>,
    /// The module specifier literal's own span, quotes included (facts
    /// schema 63), extracted exactly when `module` is. `None` for an export
    /// with no module, and for a fact decoded without the field, whose
    /// literal's location is unknown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub specifier_span: Option<Span>,
    pub type_only: bool,
    /// `export * as name from "…"` publishes one namespace binding rather
    /// than contributing the target's names to an export-star set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub namespace: Option<CompactString>,
    /// Exact exported namespace binding, distinct from any member it exposes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub namespace_binding: Option<NamedSpan>,
    pub specifiers: Vec<ExportSpecifierFact>,
    pub declarations: Vec<ExportSpecifierFact>,
    /// Public declaration names that do not authenticate an exact value
    /// binding. Identifier-named TypeScript `namespace`/`module`
    /// declarations live here, including explicit `declare` forms; quoted
    /// ambient modules never do.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub declaration_surface_only: Vec<ExportSpecifierFact>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportSpecifierFact {
    pub local: NamedSpan,
    pub exported: CompactString,
    pub type_only: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum IdentifierRole {
    Binding,
    Reference,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TryHeadAwaitFact {
    pub awaited: Span,
    pub operand: Span,
    pub regions: Vec<Span>,
    /// The arguments of an operand call that are literal nodes (string,
    /// number, bigint, boolean, null): evaluating one cannot throw.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub literal_arguments: Vec<Span>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdentifierFact {
    pub span: Span,
    pub role: IdentifierRole,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReturnFact {
    pub span: Span,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub argument: Option<Span>,
    pub control_tests: Box<[Span]>,
    pub value: ReturnValueKind,
    pub conditional: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub callee: Option<Span>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structure: Option<Box<ReturnStructureFact>>,
    /// The returned value's normalized runtime shape, as
    /// [`ArgumentFact::runtime_value_kind`] gives it for an argument. A bare
    /// `return;` is `Nullish`.
    #[serde(default)]
    pub runtime_value_kind: RuntimeValueKind,
    /// The `return` statement sits in its function's straight-line flow, as
    /// [`AstFacts::unconditional_awaits`] are: no conditional, loop, switch,
    /// `try`, labelled body or logical construct between the function entry
    /// and it, so every run that does not leave earlier executes it (facts
    /// schema 64). Always false for an arrow's expression body, which has no
    /// `return` statement.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub straight_line: bool,
}

impl ReturnFact {
    #[must_use]
    pub fn elements(&self) -> &[Option<Span>] {
        self.structure
            .as_deref()
            .map_or(&[], |structure| structure.elements.as_slice())
    }

    #[must_use]
    pub fn properties(&self) -> &[ReturnPropertyFact] {
        self.structure
            .as_deref()
            .map_or(&[], |structure| structure.properties.as_slice())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReturnStructureFact {
    /// Whether the entire literal uses the initial fixed-construction grammar.
    /// This selects proposals only; the independent compiler census proves it.
    #[serde(default)]
    pub complete_literal: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub elements: Vec<Option<Span>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub properties: Vec<ReturnPropertyFact>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReturnPropertyFact {
    pub name: CompactString,
    pub value: Span,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JsxElementFact {
    pub span: Span,
    #[serde(default)]
    pub opening: Span,
    pub name: NamedSpan,
    /// The closing tag's name span, when the element has a closing tag.
    /// `<Panel></Panel>` writes the tag name twice, and TypeScript reports
    /// both occurrences as references to the same symbol, so a consumer that
    /// enumerates references to a component has to account for this one too.
    /// It is the *same* name as `name`, not a second element: nothing about a
    /// closing tag is an independent operation, and `self_closing` already
    /// says whether it exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub closing_name: Option<Span>,
    /// The object and final property of a dotted JSX name. Keeping these
    /// spans separate lets semantic consumers prove `<Context.Provider>`
    /// from the `Context` binding without parsing the name text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub member_object: Option<Span>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub member_property: Option<Span>,
    pub properties: Vec<Span>,
    pub boolean_properties: Vec<BooleanPropertyFact>,
    #[serde(default)]
    pub attributes: Vec<JsxAttributeFact>,
    #[serde(default)]
    pub spreads: Vec<JsxSpreadAttributeFact>,
    #[serde(default)]
    pub self_closing: bool,
    #[serde(default)]
    pub children: Vec<Span>,
    /// Children that are an expression container holding no expression
    /// (`{}`, `{/* c */}`, or one the parser reads as whitespace only, such
    /// as `{\u{200B}}`). No lowering builds a value from them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub empty_children: Vec<Span>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum JsxAttributeValueKind {
    Boolean,
    String,
    Expression,
    Element,
    Fragment,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JsxAttributeFact {
    pub span: Span,
    pub name: Span,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<Span>,
    pub local_name: Span,
    /// Declaration selected by ECMAScript/TypeScript lexical scope lookup for
    /// the local name of a `use:name` directive. `None` means that the binder
    /// found no declaration in this file's scope tree. Other JSX namespaces
    /// do not interpret their local name as a value binding and leave this
    /// field empty.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub directive_binding: Option<Span>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<Span>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expression: Option<Span>,
    /// True when an assertion or non-null assertion can make the attribute
    /// type-check without changing its runtime value. `satisfies` and
    /// parentheses are transparent but do not launder an invalid value.
    #[serde(default)]
    pub runtime_type_escape: bool,
    pub value_kind: JsxAttributeValueKind,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JsxSpreadAttributeFact {
    pub span: Span,
    pub argument: Span,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConditionalExpressionFact {
    pub span: Span,
    pub test: Span,
    pub consequent: Span,
    pub alternate: Span,
    /// ADR 0115: the elements of a branch that is an array literal, after
    /// parentheses and type wrappers, in order -- `None` for a spread or a
    /// hole, and `Some([])` for `[]`. Absent for every other branch. What the
    /// generator's `returns` walk reads to propose a fresh array of arguments;
    /// the census decides it from the producer's own arms.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub consequent_array: Option<Box<[Option<Span>]>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alternate_array: Option<Box<[Option<Span>]>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LogicalOperatorKind {
    And,
    Or,
    Coalesce,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogicalExpressionFact {
    pub span: Span,
    pub left: Span,
    pub right: Span,
    pub operator: LogicalOperatorKind,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectPropertyFact {
    pub span: Span,
    pub key: Span,
    pub value: Span,
    pub computed: bool,
    /// Runtime shape of the property value after transparent TypeScript
    /// wrappers are removed.
    #[serde(default)]
    pub value_kind: RuntimeValueKind,
    /// Whether a type or non-null assertion can make this property value
    /// statically acceptable without changing what reaches the runtime.
    #[serde(default)]
    pub runtime_type_escape: bool,
    /// Whether this is a plain **data** property: `kind: Init` and not a
    /// method. A getter or setter can run its body on access; a shorthand
    /// method stores a function with a receiver channel. Consumers that admit
    /// methods must consult the separate `method` fact and close that channel.
    ///
    /// This is the fact that lets a consumer prove a literal's property set is
    /// *closed against accessors*, and so conclude something about every value
    /// in it. Without it `{ get when() { return new Date(); } }` is
    /// indistinguishable from `{ when: "2026-01-01" }` — the first would read
    /// as JSON-safe when it is not. `ArgumentFact::exact_object_literal`
    /// carries the same guarantee for a literal written directly as an
    /// argument; this carries it per property, so a literal in any position
    /// can be judged.
    #[serde(default)]
    pub data: bool,
    /// An ordinary shorthand method, excluding getters and setters. Its
    /// function value is stored directly but has a receiver channel.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub method: bool,
    /// Declaration this shorthand property's value binding refers to, as
    /// resolved by the binder for that exact reference.
    ///
    /// A shorthand (`{ pathname }`) writes one identifier where a key and a
    /// value both stand, and TypeScript answers a symbol query at that span
    /// with the *property's* symbol, never the value binding's. Oxc's scope
    /// tree does resolve the value reference, so the declaration it chose is
    /// recorded here rather than left to be re-derived from the spelling.
    ///
    /// `None` for a non-shorthand property, and for a shorthand whose value
    /// the binder resolves to no declaration in this file's scope tree -- an
    /// import namespace member or a global. Consumers fail closed on `None`;
    /// it is the absence of a fact, never proof of a missing binding.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shorthand_binding: Option<Span>,
}

/// An untagged template literal and the expressions interpolated into it.
///
/// Tagged templates are deliberately excluded because the two coerce
/// differently: a tag receives the interpolations as values and may do
/// anything with them, while an untagged literal stringifies each one. That
/// makes this the fact that proves an interpolated accessor renders its own
/// source text.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateLiteralFact {
    pub span: Span,
    pub expressions: Vec<Span>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemberFact {
    pub span: Span,
    pub object: Span,
    pub property: Span,
}

/// A computed member access whose key is a literal naming one property: the
/// member's span and that property key. See
/// [`AstFacts::literal_computed_members`].
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiteralMemberKeyFact {
    pub span: Span,
    pub key: CompactString,
}

/// One member of an array or object literal written directly as a call
/// argument: the property key the runtime gives it and the span of the value
/// written there. See [`ArgumentFact::literal_members`].
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArgumentMemberFact {
    pub key: CompactString,
    pub value: Span,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpreadFact {
    pub span: Span,
    pub argument: Span,
}

/// A fresh literal, with its own enumerable string properties in source
/// order. `closed` is false on any spread, computed/numeric key, duplicate,
/// setter or prototype-setting member. No descriptor or proxy is inferred.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectGetShapeFact {
    pub span: Span,
    pub closed: bool,
    pub properties: Vec<ObjectGetPropertyFact>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectGetPropertyFact {
    pub key: CompactString,
    /// Exact function expression of a get descriptor; never a method value.
    pub getter: Option<Span>,
    /// A no-argument identifier Call evaluated as the first operation of the
    /// getter body. This is a positive prefix fact, not absence of hazards.
    pub entry_call: Option<Span>,
}

pub(super) fn getter_entry_call(value: &Expression<'_>) -> Option<Span> {
    fn first(expression: &Expression<'_>) -> Option<Span> {
        match expression.get_inner_expression() {
            Expression::CallExpression(call)
                if !call.optional
                    && call.arguments.is_empty()
                    && matches!(
                        call.callee.get_inner_expression(),
                        Expression::Identifier(_)
                    ) =>
            {
                Some(span(call.span))
            }
            // The receiver is evaluated before the member is obtained.
            Expression::StaticMemberExpression(member) => first(&member.object),
            _ => None,
        }
    }
    let Expression::FunctionExpression(function) = value else {
        return None;
    };
    if function.r#async || function.generator || !function.params.items.is_empty() {
        return None;
    }
    let body = function.body.as_ref()?;
    // Directives cause no effect, but no other statement is skipped.
    match body.statements.first()? {
        oxc_ast::ast::Statement::ReturnStatement(statement) => first(statement.argument.as_ref()?),
        oxc_ast::ast::Statement::ExpressionStatement(statement) => first(&statement.expression),
        oxc_ast::ast::Statement::VariableDeclaration(statement)
            if statement.declarations.len() == 1 =>
        {
            first(statement.declarations[0].init.as_ref()?)
        }
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CoerciveOperandKind {
    StringConcatenation,
    LogicalNot,
    /// Unary `+`, `-`, or `~`: the operand is read through `ToNumber`, and
    /// TypeScript accepts a function there.
    NumericCoercion,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoerciveOperandFact {
    pub span: Span,
    pub kind: CoerciveOperandKind,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssignmentFact {
    pub target: Span,
    pub value_span: Span,
    pub value: AssignmentValueKind,
    /// Whether evaluating the assignment reads the previous target value.
    /// Plain `=` does not; compound assignments and update expressions do.
    #[serde(default)]
    pub reads_target: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub call_initializer: Option<Span>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub array_slots: Vec<Option<Span>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AssignmentValueKind {
    Array,
    Function,
    Other,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IfRegionFact {
    pub test: Span,
    pub consequent: Span,
    /// The `else` arm, when one is written (facts schema 73). Only one of the
    /// two arms runs, so an await in one cannot precede code in the other.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alternate: Option<Span>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReturnValueKind {
    Undefined,
    Function,
    Identifier,
    Call,
    Member,
    Other,
}

impl AstFacts {
    /// The first element of `facts` (sorted by span, as extraction leaves
    /// every list below) whose span is exactly `span`: what a linear
    /// `iter().find(|fact| fact.span == span)` returns, by binary search.
    fn at_span<T>(facts: &[T], span: Span, of: impl Fn(&T) -> Span) -> Option<&T> {
        let index = facts.partition_point(|fact| of(fact) < span);
        facts.get(index).filter(|fact| of(fact) == span)
    }

    /// The member expression at exactly `span`.
    #[must_use]
    pub fn member_at(&self, span: Span) -> Option<&MemberFact> {
        Self::at_span(&self.members, span, |fact| fact.span)
    }

    /// The links of the member chain at `member` past its root: every object
    /// below `member` that is itself a member, outermost first, each as
    /// written (`store.a.b` and `store.a` in `store.a.b.c`; `store.n!` in
    /// `store.n!.b`, which TypeScript types by its wrapper). The walk goes
    /// through transparent wrappers and stops at the first object that is no
    /// member, the root. A store write reads every link before it writes
    /// (ADR 0460, 0465).
    #[must_use]
    pub fn member_chain_links(&self, member: Span) -> Vec<Span> {
        let mut links = Vec::new();
        let mut current = self.member_at(member);
        while let Some(access) = current {
            let next = self.member_at(self.peel_ts_sugar_span(access.object));
            if next.is_some() {
                links.push(access.object);
            }
            current = next;
        }
        links
    }

    /// The conditional expression at exactly `span`.
    #[must_use]
    pub fn conditional_at(&self, span: Span) -> Option<&ConditionalExpressionFact> {
        Self::at_span(&self.conditional_expressions, span, |fact| fact.span)
    }

    /// The logical expression at exactly `span`.
    #[must_use]
    pub fn logical_at(&self, span: Span) -> Option<&LogicalExpressionFact> {
        Self::at_span(&self.logical_expressions, span, |fact| fact.span)
    }

    /// The function at exactly `span`.
    #[must_use]
    pub fn function_at(&self, span: Span) -> Option<&FunctionFact> {
        Self::at_span(&self.functions, span, |fact| fact.span)
    }

    /// The JSX element at exactly `span`.
    #[must_use]
    pub fn jsx_element_at(&self, span: Span) -> Option<&JsxElementFact> {
        Self::at_span(&self.jsx_elements, span, |fact| fact.span)
    }

    /// The array literal (with its elements) at exactly `span`.
    #[must_use]
    pub fn array_literal_at(&self, span: Span) -> Option<&ArrayLiteralFact> {
        Self::at_span(&self.array_literal_elements, span, |fact| fact.span)
    }

    /// Whether `span` lies in one of [`Self::unreachable_regions`]: no
    /// execution runs it.
    #[must_use]
    pub fn unreachable_contains(&self, span: Span) -> bool {
        // Sorted by start: only a region starting at or before `span` can hold it.
        let end = self
            .unreachable_regions
            .partition_point(|region| region.start <= span.start);
        self.unreachable_regions[..end]
            .iter()
            .any(|region| region.contains(span))
    }

    /// The declaration the binder resolved the reference at `span` to.
    ///
    /// `None` when `span` is not an identifier reference, or when the binder
    /// resolved it to no declaration in this file. See
    /// [`AstFacts::reference_declarations`].
    #[must_use]
    pub fn reference_declaration(&self, span: Span) -> Option<Span> {
        self.reference_declarations
            .binary_search_by_key(&span, |(reference, _)| *reference)
            .ok()
            .map(|index| self.reference_declarations[index].1)
    }

    /// Whether `span` is *exactly* a class's binding name, or exactly the class
    /// node itself.
    ///
    /// Span identity, not containment: the name of a class declaration or of a
    /// named class expression, and — for an anonymous class, which has no name
    /// span — the `class …` node's own span. A span inside the class *body* is
    /// neither.
    ///
    /// This is a **span-addressing rule**, not a proof that some value is a
    /// class: it tells a caller that the span it is about to ask Type Facts
    /// about denotes the *constructor* rather than an instance, which is the
    /// one place the compiler's answer is about a different value than the
    /// export's. `export default class {}` is why the anonymous form is here:
    /// the export records the class node's span because there is no name to
    /// record, and the facts at that span describe the instance type. See
    /// [`ClassFact`], and `class_declaration_name` in solid-reactive-ir for the
    /// consumer.
    #[must_use]
    pub fn declares_class_at(&self, span: Span) -> bool {
        self.classes.iter().any(|class| {
            class.span == span || class.name.as_ref().is_some_and(|name| name.span == span)
        })
    }

    /// Whether `span` is exactly the runtime binding name of a TypeScript
    /// `namespace` or identifier-named `module` declaration.
    #[must_use]
    pub fn declares_namespace_at(&self, span: Span) -> bool {
        self.namespace_declarations
            .binary_search_by_key(&span, |name| name.span)
            .is_ok()
    }

    /// The direct function declaration whose binding name is exactly `span`.
    #[must_use]
    pub fn function_declaration_at(&self, span: Span) -> Option<&FunctionDeclarationFact> {
        self.function_declarations
            .binary_search_by_key(&span, |declaration| declaration.name.span)
            .ok()
            .map(|index| &self.function_declarations[index])
    }

    /// Whether `span` is *exactly* the target an assignment overwrites without
    /// reading the previous value — plain `=` and nothing else.
    ///
    /// Span identity is the whole point: a span merely *contained* in the
    /// target is a genuine read. `state.rows[props.index].done = true` writes
    /// one member and reads `props.index` to address it, and
    /// `({ a: local = profile.fallback } = obj)` reads `profile.fallback` to
    /// build the default. Compound assignments and update expressions are
    /// excluded outright: they evaluate the old value before storing the new
    /// one, so their target is a read as well as a write.
    #[must_use]
    pub fn is_plain_assignment_target(&self, span: Span) -> bool {
        self.assignments
            .iter()
            .any(|assignment| !assignment.reads_target && assignment.target == span)
    }

    /// Peel only syntax that preserves the runtime value of an expression.
    ///
    /// The parser already exposes the equivalent node-level operation as
    /// [`Expression::get_inner_expression`]. Structural facts are serialized
    /// as spans, so this small shared adapter preserves that operation for
    /// consumers that no longer have Oxc nodes. It intentionally does not
    /// peel arbitrary conversions, calls, or member expressions.
    #[must_use]
    pub fn peel_ts_sugar_span(&self, mut span: Span) -> Span {
        // `transparent_wrappers` is sorted by span, and this runs at every
        // callee and value resolution, so the lookup is a binary search
        // rather than a scan of every wrapper in the file.
        while let Ok(index) = self
            .transparent_wrappers
            .binary_search_by_key(&span, |wrapper| wrapper.span)
        {
            let wrapper = &self.transparent_wrappers[index];
            if wrapper.inner == span {
                break;
            }
            span = wrapper.inner;
        }
        span
    }

    /// Whether `span` lies inside a `namespace`, `module`, or `declare global`
    /// body — see [`AstFacts::module_blocks`].
    #[must_use]
    pub fn is_inside_module_block(&self, span: Span) -> bool {
        self.module_blocks.iter().any(|block| block.contains(span))
    }

    /// The exports that publish a name from *this module*.
    ///
    /// An `export` statement inside a `namespace` body is a member of the
    /// namespace object, not of the module: `export namespace Config { export
    /// const inner = 1 }` publishes `Config` and nothing else, and the same
    /// holds for an `export` inside `declare module "…" {}` (whose exports
    /// belong to the declared module) or inside `declare global {}`. Any
    /// consumer enumerating the module's export surface must iterate this
    /// rather than [`AstFacts::exports`], which is the complete syntactic table
    /// including the nested statements.
    pub fn module_level_exports(&self) -> impl Iterator<Item = &ExportFact> {
        self.exports
            .iter()
            .filter(|export| !self.is_inside_module_block(export.span))
    }

    /// The non-array bindings an export statement itself declares: inside the
    /// export's span, but not inside the body of any function or namespace the
    /// same export contains, and not inside the body of any class the same
    /// export contains (those belong to the function, the namespace object, or
    /// the class, not to the export surface).
    ///
    /// The namespace exclusion is what makes this agree with
    /// [`ExportFact::declarations`], which answers a `TSModuleDeclaration` with
    /// the namespace's own name alone. Without it `export namespace Config {
    /// export const inner = 1 }` contributes `inner` — a name no importer of
    /// this module can resolve, because it is a member of the `Config` object.
    ///
    /// The class exclusion is the same shape, for a **static block**: `export
    /// class Holder { static { const inside = 1 } }` binds `inside` inside
    /// `Holder`'s class body — neither a function body nor a module block, so
    /// neither of the other two exclusions catches it, and no importer of this
    /// module can resolve `inside`. The test is containment in a
    /// [`ClassFact::span`] that is *strictly smaller* than the declarator's own
    /// span, not mere containment: a class **expression** initializer —
    /// `export const boxed = class { static { const inside = 1 } }` — gives
    /// `boxed`'s own declarator span (identifier through initializer) a span
    /// that *contains* the class expression, the reverse relationship, so it
    /// must survive while `inside` is excluded. A module-level declarator can
    /// never itself sit inside a class body, so this containment check cannot
    /// misfire on one.
    pub fn exported_bindings<'a>(
        &'a self,
        export: &'a ExportFact,
    ) -> impl Iterator<Item = &'a BindingFact> {
        self.bindings.iter().filter(move |binding| {
            binding.shape != BindingShape::Array
                && export.span.contains(binding.declaration)
                && !self.is_inside_module_block(binding.declaration)
                && !self.classes.iter().any(|class| {
                    class.span.contains(binding.declaration) && class.span != binding.declaration
                })
                && !self.functions.iter().any(|function| {
                    export.span.contains(function.span)
                        && function.body.contains(binding.declaration)
                })
        })
    }

    /// An all-empty fact table for a source that carries no JavaScript
    /// syntax at all, such as a `.json` module.
    ///
    /// This is a proof, not an approximation: a JSON module's namespace is
    /// data, so it certifiably contributes no call, no binding, no function,
    /// no JSX, no read, no write -- every table is correctly empty because
    /// there is nothing in the source for any of them to describe. Consumers
    /// resolving a member or call against this file's exports terminate on a
    /// proven "not a function" rather than falling back to the conservative
    /// "unresolved import" path.
    #[must_use]
    pub fn empty(source: SourceIdentity) -> Self {
        Self {
            schema: AST_FACTS_SCHEMA,
            source,
            span_index: LazySpanIndex::default(),
            calls: Vec::new(),
            host_execution: Vec::new(),
            host_defaults: Vec::new(),
            host_call_sequences: Vec::new(),
            host_empty_arrays: Vec::new(),
            host_zero_values: Vec::new(),
            host_undefined_arguments: Vec::new(),
            bindings: Vec::new(),
            functions: Vec::new(),
            function_declarations: Vec::new(),
            classes: Vec::new(),
            namespace_declarations: Vec::new(),
            imports: Vec::new(),
            exports: Vec::new(),
            module_loads: Vec::new(),
            sequences: Vec::new(),
            import_equals: Vec::new(),
            type_imports: Vec::new(),
            type_queries: Vec::new(),
            type_names: Vec::new(),
            tagged_template_tags: Vec::new(),
            module_hazards: Vec::new(),
            module_blocks: Vec::new(),
            identifiers: Vec::new(),
            reference_declarations: Vec::new(),
            awaits: Vec::new(),
            unconditional_awaits: Vec::new(),
            dominating_awaits: Vec::new(),
            try_head_awaits: Vec::new(),
            in_place_import_globs: Vec::new(),
            unmodeled_require_references: Vec::new(),
            straight_line_calls: Vec::new(),
            straight_line_constructs: Vec::new(),
            completion_sites: Vec::new(),
            returns: Vec::new(),
            jsx_elements: Vec::new(),
            jsx_fragments: Vec::new(),
            transparent_wrappers: Vec::new(),
            members: Vec::new(),
            computed_members: Vec::new(),
            array_literals: Vec::new(),
            literal_computed_members: Vec::new(),
            optional_members: Vec::new(),
            parameter_properties: Vec::new(),
            spreads: Vec::new(),
            conditional_tests: Vec::new(),
            nullish_test_operands: Vec::new(),
            conditional_expressions: Vec::new(),
            logical_expressions: Vec::new(),
            object_properties: Vec::new(),
            object_get_shapes: Vec::new(),
            template_literals: Vec::new(),
            coercive_operands: Vec::new(),
            coercing_operands: Vec::new(),
            primitive_results: Vec::new(),
            iterated_operands: Vec::new(),
            assignments: Vec::new(),
            write_targets: Vec::new(),
            deleted_targets: Vec::new(),
            if_regions: Vec::new(),
            jump_statements: Vec::new(),
            iteration_targets: Vec::new(),
            loop_statements: Vec::new(),
            loop_repeats: Vec::new(),
            pattern_values: Vec::new(),
            implicit_suspensions: Vec::new(),
            module_directives: Vec::new(),
            stray_directives: Vec::new(),
            suppression_comments: Vec::new(),
            unreachable_regions: Vec::new(),
            tagged_templates: Vec::new(),
            array_literal_elements: Vec::new(),
            pattern_elements: Vec::new(),
            await_operands: Vec::new(),
            jsx_child_expressions: Vec::new(),
            disposal_initializers: Vec::new(),
        }
    }

    #[must_use]
    pub fn structural_seed_spans(&self) -> Vec<Span> {
        let mut spans = self
            .identifiers
            .iter()
            .map(|identifier| identifier.span)
            .chain(
                self.imports
                    .iter()
                    .flat_map(|import| import.bindings.iter())
                    .filter(|binding| binding.kind != ImportKind::SideEffect)
                    .map(|binding| binding.local.span),
            )
            .chain(
                self.bindings
                    .iter()
                    .flat_map(|binding| binding.names.iter())
                    .map(|name| name.span),
            )
            .chain(self.jsx_elements.iter().map(|element| element.name.span))
            .collect::<Vec<_>>();
        spans.sort_unstable();
        spans.dedup();
        spans
    }
}

#[derive(Debug, Error)]
pub enum AstFactsError {
    #[error(transparent)]
    Identity(#[from] crate::core::FactIdentityError),
    #[error("unsupported source path {path}: {message}")]
    SourceType { path: String, message: String },
    #[error("Oxc parse failed: {0}")]
    Parse(String),
}
