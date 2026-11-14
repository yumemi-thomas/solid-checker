//! Whether a file of the program can name `Object.prototype` (the review of #232, S3, and its re-checks).
//!
//! Every plain object inherits from `Object.prototype`, so an accessor a program puts on it runs with any object as its receiver where a member of that object is read: no
//! object is then provably no receiver, and the strict prefilter (`holder_confined`) has no holder to skip. A file that can *name* the object (to give it an accessor, or to
//! hand it to code that does) is one the program may do that in, so the answer is "touched", which only withholds the prefilter (an uncertifiable row, never a silence).
//!
//! The question is asked of what a file spells, with the polarity "touched unless the use provably gives nothing out", so a route nobody followed is touched, not
//! clean (the first version answered "not Object" for what it could not read, and the re-check of #232 found 19 Chrome false cleans through that exit, the next 12 more):
//!
//! - a reference to a global that can lead to it (`Object`, `Reflect`, `globalThis`, `window`, `self`, `global`, `top`, `parent`, `frames`; facts `global_value_references`) is
//!   touched unless it is an operand nothing takes (`typeof window`, `x === Object`) or the object of a member whose key the program spells and by which the global gives no
//!   prototype out: `Object.keys`, `window.addEventListener`. Any other use is a name for the global (an initializer, an argument, a return, a pattern, a heritage, a sequence, a
//!   logical operand), whose uses nothing here follows, and a key the program does not spell (`Object[key]`) may be `prototype`. `Function` and `eval` run code that may spell
//!   anything, so any use of them is touched;
//! - the same names as the key of a member read of anything (`x.Object`, `x.Reflect`, `x.Function`, `x.eval`) are asked as the references are: whatever `x` is (`top`, a
//!   `document.defaultView`, an event's `view`, an alias or a call's result), the last step from a window to the global is this read, so no route to a window needs a list;
//! - `Object.prototype` itself is touched unless it is an operand nothing takes or the object of a read of a built-in method that returns no object (`hasOwnProperty`,
//!   `isPrototypeOf`, `propertyIsEnumerable`, `toLocaleString`, `toString`): `valueOf` returns its receiver, so `Object.prototype.valueOf()` is the prototype itself;
//! - a call of `Object.getPrototypeOf`, `Reflect.getPrototypeOf`, `getOwnPropertyDescriptor` or `getOwnPropertyDescriptors` (of `Object` or `Reflect`) is touched unless its value is
//!   an operand nothing takes (`Object.getPrototypeOf(x) === Object.prototype`);
//! - a read of `constructor` (`({}).constructor` is `Object`), of `__proto__` is touched under the same rule, as is a pattern that binds `__proto__` or `constructor` (a
//!   declaration, a parameter, an assignment pattern's keys) and a call given one of them as a string; a write of either is no read;
//! - a read under a computed key (`x[k]`, `({ [k]: p } = x)`, `Reflect.get(x, k)`) is touched unless the facts prove its key is a number, a symbol or a value that is none of the
//!   keys that hand out a prototype (`constructor`, `__proto__`, `prototype`, and `Object`, `Reflect`, `Function`, `eval`, the names of what leads to it): a key built at run
//!   time (`"const" + "ructor"`) is a spelling as good as the word, and a key known only from its type (`k: number`) is no proof; a write and an operand nothing takes read none out.
//!   `Reflect.get` used other than called (an alias, `.call`) is touched. A file that spells `__defineGetter__`, `__defineSetter__`, `__lookupGetter__` or `__lookupSetter__` is touched.
//!
//! - a file that runs code it builds as a string (facts `code_sinks`: `eval`, `Function`, a `.constructor(..)` call, a timer given a string, a `<script>` element, an event
//!   attribute set as text, markup written as a string that is not proven inert, a load by a computed specifier or a `data:`/`blob:` URL, a `javascript:` URL, a global looked
//!   up by a computed name) is touched, and its receivers are unreadable ([`generates_code`]): the generated code may give `Object.prototype` an accessor that reads any key
//!   through `this`, and nothing in the file shows the read, so no store of a function is confined and any key may be read.
//!
//! `super.prototype` needs no rule of its own: a class that extends `Object` names it by the first one. Not followed, named in `docs/open-work.md`: a package the build does not read
//! as source is no premise any more: such a package is asked of the backend's walk (`PackageReachIndex::unseen_code`), and an unread one is touched. The top-level `this` of a
//! script that is no module is named in `docs/open-work.md`.

use std::collections::HashSet;

use solid_facts::FileFacts;
use solid_facts::ast::{ArgumentLiteralFact, AstFacts, BindingFact, BindingShape, ComputedKeySite};
use solid_facts::core::Span;

use super::{key_of_text, member_spelled_key};

/// What an expression the file writes evaluates to, for the question whether it can give `Object.prototype` out.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Value {
    /// The global `Object`, or what `x.constructor` may be.
    Object,
    /// The global `Reflect`.
    Reflect,
    /// The global object: `globalThis`, `window`, `self`, `global`, `top`, `parent`, `frames`.
    Scope,
}

/// The keys by which a value hands out a prototype or what leads to one: a plain object's `constructor` (the global `Object`) and `__proto__` (the prototype), a function's
/// `prototype`, and the global names a window holds (`Object`, `Reflect`, `Function`, `eval`).
const HAND_OUT_KEYS: [&str; 7] = [
    "constructor",
    "__proto__",
    "prototype",
    "Object",
    "Reflect",
    "Function",
    "eval",
];

/// Whether a file may name `Object.prototype`: see the module's text.
pub(super) fn names_object_prototype(file: &FileFacts) -> bool {
    generates_code(file) || names_by_spelling(file) || reads_an_unproven_key(file)
}

/// What the backend asks of each file of an installed package its walk reaches ([`crate::unseen_code_in`]): why the file may run code nothing here reads (a route to
/// `Object.prototype`, code it builds as a string), and whether it may read a member through the object a function runs against.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UnseenCode {
    /// Why the file may run code nothing here reads, in a word; `None` when nothing in it does.
    pub why: Option<&'static str>,
    /// Whether the file holds a `this` or a `super` (facts `uses_receiver`).
    pub reads_receiver: bool,
}

pub(super) fn unseen_code(file: &FileFacts) -> UnseenCode {
    let why = if generates_code(file) {
        Some("generates code")
    } else if names_by_spelling(file) || reads_an_unproven_key(file) {
        Some("names Object.prototype")
    } else {
        None
    };
    UnseenCode {
        why,
        reads_receiver: file.ast.uses_receiver,
    }
}

/// Whether the file runs code it builds as a string (facts `code_sinks`): its receivers are unreadable, and it is touched.
pub(super) fn generates_code(file: &FileFacts) -> bool {
    !file.ast.code_sinks.is_empty()
}

/// The routes a file spells: a global, a member, a call, a pattern.
fn names_by_spelling(file: &FileFacts) -> bool {
    let ast = &file.ast;
    if [
        "__defineGetter__",
        "__defineSetter__",
        "__lookupGetter__",
        "__lookupSetter__",
    ]
    .iter()
    .any(|name| file.source.contains(name))
    {
        return true;
    }
    let references = ast.global_value_references.iter().any(|reference| {
        match file.source_text(*reference) {
            Some("Object") => reaches(file, *reference, Value::Object),
            Some("Reflect") => reaches(file, *reference, Value::Reflect),
            Some("globalThis" | "window" | "self" | "global" | "top" | "parent" | "frames") => {
                reaches(file, *reference, Value::Scope)
            }
            // Code a string spells may spell anything: a use of these is touched unless nothing takes it.
            Some("Function" | "eval") => !inert(ast, *reference),
            _ => false,
        }
    });
    references
        || ast.members.iter().any(|member| {
            match member_spelled_key(file, member).as_deref() {
                // A read of `constructor` is the global `Object` where the object is a plain one, and `__proto__` is the prototype itself.
                Some("constructor") => {
                    !written(ast, member.span) && reaches(file, member.span, Value::Object)
                }
                Some("__proto__") => !written(ast, member.span) && !inert(ast, member.span),
                // The names of the globals read off any object: the last step from a window (`top`, `defaultView`, an event's `view`) to them.
                Some("Object") => !written(ast, member.span) && reaches(file, member.span, Value::Object),
                Some("Reflect") => {
                    !written(ast, member.span) && reaches(file, member.span, Value::Reflect)
                }
                Some("Function" | "eval") => !written(ast, member.span) && !inert(ast, member.span),
                _ => false,
            }
        })
        // A key spelled in a string a call is given (`Reflect.get(x, "__proto__")`) or in an assignment pattern (`({ __proto__: p } = x)`).
        || ast.calls.iter().any(|call| {
            call.arguments.iter().any(|argument| {
                matches!(&argument.literal_value, ArgumentLiteralFact::String(key) if names_prototype_key(key))
            })
        })
        || ast.assignment_pattern_keys.iter().any(|key| {
            file.source_text(*key)
                .and_then(|text| key_of_text(text, false))
                .is_some_and(|key| names_prototype_key(&key))
        })
        || ast.bindings.iter().any(binds_prototype_key)
        || ast
            .functions
            .iter()
            .any(|function| function.parameters.iter().any(binds_prototype_key))
}

/// Whether what the expression at `expr` evaluates to (`value`) can give `Object.prototype` out: it is used as anything but an operand nothing takes or the object of a member
/// whose key the program spells and by which `value` gives no prototype.
fn reaches(file: &FileFacts, expr: Span, value: Value) -> bool {
    let ast = &file.ast;
    if inert(ast, expr) {
        return false;
    }
    let Some(member) = ast.members_with_peeled_object(expr).next() else {
        return true;
    };
    let Some(key) = member_spelled_key(file, member) else {
        return true;
    };
    match (value, key.as_str()) {
        (Value::Object, "prototype") => prototype_given_out(file, member.span),
        // What these hand out: the prototype itself, or the accessor a plain object inherits (`getOwnPropertyDescriptor(Object.prototype, "__proto__").get`).
        (
            Value::Object | Value::Reflect,
            "getPrototypeOf" | "getOwnPropertyDescriptor" | "getOwnPropertyDescriptors",
        ) => !called_for_an_inert_value(ast, member.span),
        // `Reflect.get(x, k)` reads the key it is given: the call is asked as a read under that key (`reads_an_unproven_key`), and any other use of `Reflect.get` gives it out.
        (Value::Reflect, "get") => ast.calls_with_peeled_callee(member.span).next().is_none(),
        // A name for the global another way (`globalThis.Object`, `window.window`): the member is the expression asked in turn.
        (Value::Scope, "Object") => reaches(file, member.span, Value::Object),
        (Value::Scope, "Reflect") => reaches(file, member.span, Value::Reflect),
        (Value::Scope, "Function" | "eval") => !inert(ast, member.span),
        (
            Value::Scope,
            "globalThis" | "window" | "self" | "global" | "frames" | "top" | "parent",
        ) => reaches(file, member.span, Value::Scope),
        _ => false,
    }
}

/// Whether the member `Object.prototype` at `member` gives the prototype out: it is no operand nothing takes, and is not the object of a read of a built-in method that
/// returns no object (`valueOf` returns its receiver, and is not one of them).
fn prototype_given_out(file: &FileFacts, member: Span) -> bool {
    let ast = &file.ast;
    if inert(ast, member) {
        return false;
    }
    let Some(parent) = ast.members_with_peeled_object(member).next() else {
        return true;
    };
    let reads_a_builtin = member_spelled_key(file, parent).is_some_and(|key| {
        matches!(
            key.as_str(),
            "hasOwnProperty"
                | "isPrototypeOf"
                | "propertyIsEnumerable"
                | "toLocaleString"
                | "toString"
        )
    });
    written(ast, parent.span) || !reads_a_builtin
}

/// Whether a pattern binds a local to `__proto__` or `constructor` (`const { __proto__: P } = x`): the prototype, or the global `Object`.
fn binds_prototype_key(binding: &BindingFact) -> bool {
    binding.shape == BindingShape::Object
        && binding
            .object_slots
            .iter()
            .any(|slot| names_prototype_key(slot.property.as_str()))
}

/// Whether the key reads the prototype of an object (`__proto__`) or the global `Object` of a plain one (`constructor`).
fn names_prototype_key(key: &str) -> bool {
    matches!(key, "__proto__" | "constructor")
}

/// Whether the file reads under a computed key the facts do not prove is none of [`HAND_OUT_KEYS`] and uses the value: a computed member (a write and an operand nothing takes
/// read none out), a computed property of an object pattern, and the key argument of `Reflect.get`. A key is proven from its expression alone (facts `computed_keys`): a number,
/// a symbol, a constant outside the list; a type is no proof.
fn reads_an_unproven_key(file: &FileFacts) -> bool {
    let ast = &file.ast;
    if ast.computed_keys.is_empty() {
        return false;
    }
    // Asked once for the file, not once per read: the facts list one entry for every `obj[key]` a file holds.
    let spans = |spans: &[Span]| {
        spans
            .iter()
            .map(|span| ast.peel_ts_sugar_span(*span))
            .collect::<HashSet<_>>()
    };
    let written = ast
        .write_targets
        .iter()
        .chain(&ast.deleted_targets)
        .map(|span| ast.peel_ts_sugar_span(*span))
        .collect::<HashSet<_>>();
    let inert = spans(&ast.inert_operands);
    ast.computed_keys.iter().any(|fact| {
        let hands_out = !fact.proven
            || fact
                .texts
                .iter()
                .any(|text| HAND_OUT_KEYS.contains(&text.as_str()));
        hands_out
            && match fact.site {
                ComputedKeySite::Member => {
                    !written.contains(&fact.read) && !inert.contains(&fact.read)
                }
                ComputedKeySite::Pattern => true,
                ComputedKeySite::ReflectGet => !inert.contains(&fact.read),
            }
    })
}

/// Whether the expression at `expr` is an operand nothing takes (`typeof x`, `x === y`, an expression statement).
fn inert(ast: &AstFacts, expr: Span) -> bool {
    let expr = ast.peel_ts_sugar_span(expr);
    ast.inert_operands
        .iter()
        .any(|operand| ast.peel_ts_sugar_span(*operand) == expr)
}

/// Whether the member at `member` is the target of a write or the operand of a `delete`: neither holds the value the member names (a `delete` runs a trap and no Get).
fn written(ast: &AstFacts, member: Span) -> bool {
    ast.write_targets
        .iter()
        .chain(&ast.deleted_targets)
        .any(|target| ast.peel_ts_sugar_span(*target) == member)
}

/// Whether the member at `member` is the callee of a call whose value is an operand nothing takes.
fn called_for_an_inert_value(ast: &AstFacts, member: Span) -> bool {
    ast.calls_with_peeled_callee(member)
        .next()
        .is_some_and(|call| inert(ast, call.span))
}

#[cfg(test)]
mod tests {
    use super::{names_object_prototype, reads_an_unproven_key};

    fn file(source: &str) -> solid_facts::FileFacts {
        use solid_facts::core::{Generation, SourceHash, SourcePath};
        let ast = solid_facts::ast::extract("names.tsx", source).unwrap();
        solid_facts::FileFacts {
            generation: Generation::new(1).unwrap(),
            path: SourcePath::new("names.tsx").unwrap(),
            source_hash: SourceHash::of(source),
            source: std::sync::Arc::from(source),
            ast: std::sync::Arc::new(ast),
            compiler: std::sync::Arc::new(solid_facts::compiler::ExecutionMap {
                compiler_facts_protocol: solid_facts::compiler::COMPILER_FACTS_PROTOCOL,
                source_hash: SourceHash::of(source),
                semantic_model: Default::default(),
                tracked_regions: vec![],
                untracked_regions: vec![],
                discarded_regions: vec![],
                ownership_regions: vec![],
                callback_roles: vec![],
                jsx_operations: vec![],
            }),
        }
    }

    /// The review of #232 (S3) and its re-check: each program names `Object.prototype` (touched) or provably does not give it out.
    #[test]
    fn a_program_that_can_name_object_prototype_is_touched() {
        for (source, touched) in [
            // The member itself.
            ("Object.defineProperty(Object.prototype, 'h', {});", true),
            ("Object.prototype.h = 1;", true),
            ("Reflect.defineProperty(Object.prototype, 'h', {});", true),
            ("const p = Object.prototype; use(p);", true),
            ("use(Object['prototype']);", true),
            ("use(Object[key as string]);", true),
            // `valueOf` returns its receiver (the re-check, B1).
            (
                "Object.defineProperty(Object.prototype.valueOf(), 'h', {});",
                true,
            ),
            (
                "Object.defineProperty((Object.prototype as any)['valueOf'](), 'h', {});",
                true,
            ),
            ("const v = Object.prototype.valueOf; use(v.call(x));", true),
            (
                "Object.defineProperty(Object.prototype.valueOf.call(Object.prototype), 'h', {});",
                true,
            ),
            // Another name for the global (S1): every use that is no member with a spelled key.
            ("const g: any = globalThis; use(g.Object.prototype);", true),
            (
                "const { Object: O }: any = globalThis; use(O.prototype);",
                true,
            ),
            ("const { assign, keys } = Object; use(assign, keys);", true),
            ("let O: any; O = Object; use(O.prototype);", true),
            (
                "function g(O: any) { return O.prototype; } use(g(Object));",
                true,
            ),
            ("const [O]: any[] = [Object]; use(O.prototype);", true),
            ("let P: any; ({ prototype: P } = Object); use(P);", true),
            ("const P = Reflect.get(Object, 'prototype'); use(P);", true),
            (
                "function getO(): any { return Object; } use(getO().prototype);",
                true,
            ),
            ("use((0, Object).prototype);", true),
            ("use((Object ?? null).prototype);", true),
            ("const O = Object; use(O.prototype);", true),
            (
                "var A = Object, B = A, C = B, D = C, E = D; use(E.prototype);",
                true,
            ),
            (
                "class X extends Object { static p(): any { return super.prototype; } }",
                true,
            ),
            ("use(window.Object.prototype);", true),
            ("use(globalThis.globalThis.Object.prototype);", true),
            ("use(window.parent.Object.prototype);", true),
            ("const w = window; use(w.Object);", true),
            ("use(self[name as string]);", true),
            ("use(document.defaultView!.Object.prototype);", true),
            ("use(el.ownerDocument.defaultView!.Object.prototype);", true),
            // Every other name for the window (the re-check, round 2): no list of the ways to a window, the last step from it to the global is a read of `Object`.
            ("use((top as any).Object.prototype);", true),
            ("use((parent as any).Object.prototype);", true),
            ("use((frames as any).Object.prototype);", true),
            (
                "use((window as any).document.defaultView.Object.prototype);",
                true,
            ),
            (
                "use((globalThis as any).document.defaultView.Object.prototype);",
                true,
            ),
            (
                "const d: any = document; use(d.defaultView.Object.prototype);",
                true,
            ),
            (
                "const { defaultView }: any = document; use(defaultView.Object.prototype);",
                true,
            ),
            (
                "function getDoc(): any { return document; } use(getDoc().defaultView.Object.prototype);",
                true,
            ),
            ("use((ev as any).view.Object.prototype);", true),
            ("use((frame as any).contentWindow.Object.prototype);", true),
            ("use((x as any).Reflect.getPrototypeOf({}));", true),
            ("use((x as any).Function('return this')());", true),
            ("use((x as any).eval);", true),
            ("use(top);", true),
            ("use(top!.length);", false),
            // The routes that spell no `Object.prototype` (S2).
            (
                "Object.defineProperty(Object.getPrototypeOf({}), 'h', {});",
                true,
            ),
            (
                "Object.defineProperty(Reflect.getPrototypeOf({})!, 'h', {});",
                true,
            ),
            ("const proto = Object.getPrototypeOf(x); use(proto);", true),
            (
                "Object.defineProperty(({} as any).constructor.prototype, 'h', {});",
                true,
            ),
            ("const C = ({} as any).constructor; use(C);", true),
            (
                "Object.defineProperty(({} as any).__proto__, 'h', {});",
                true,
            ),
            (
                "class X {} Object.defineProperty((X.prototype as any).__proto__, 'h', {});",
                true,
            ),
            ("const { __proto__: P } = x as any; use(P);", true),
            ("const { constructor: C } = x as any; use(C);", true),
            ("function f({ __proto__: P }: any) { use(P); } f(x);", true),
            ("(Object.prototype as any).__defineGetter__('h', f);", true),
            ("(x as any).__defineSetter__('h', f);", true),
            // A key a string or a pattern spells.
            ("use(Reflect.get({}, '__proto__'));", true),
            ("use(Reflect.get({}, 'constructor'));", true),
            ("let p: any; ({ __proto__: p } = x as any); use(p);", true),
            ("let c: any; ({ constructor: c } = x as any); use(c);", true),
            (
                "let c: any; ({ 'constructor': c } = x as any); use(c);",
                true,
            ),
            // `Reflect.get` called is a read under its key; used any other way it gives out what it reads.
            ("const g = Reflect.get; use(g(x, 'a'));", true),
            ("use(Reflect.get.call(null, x, 'a'));", true),
            ("use(Reflect.apply(Reflect.get, null, [x, 'a']));", true),
            // A key built at run time is a spelling (the re-check, round 2): a concatenation, a constant, a template, a conditional.
            ("use((x as any)['const' + 'ructor'].prototype);", true),
            (
                "const k = 'const' + 'ructor'; use((x as any)[k].prototype);",
                true,
            ),
            ("use((x as any)[`__pro${'to__'}`]);", true),
            ("use((x as any)[flag ? 'prototype' : 'a']);", true),
            ("use((x as any)['Obj' + 'ect']);", true),
            ("use(Reflect.get(x, 'const' + 'ructor'));", true),
            ("use(Reflect.get(x, key as string));", true),
            // A key nothing proves is a key that may be one of them: a parameter, a call, a member, an iteration variable.
            ("use(x[key as string]);", true),
            ("function f(k: string) { return x[k]; } use(f);", true),
            ("use(x[g()]); use(x[y.z]);", true),
            ("for (const k in x) use(x[k]);", true),
            ("let s = 'a'; s = g(); use(x[s]);", true),
            ("let s = 'a'; s += 'b'; use(x[s]);", true),
            ("const { [key as string]: p } = x as any; use(p);", true),
            (
                "let p: any; ({ [key as string]: p } = x as any); use(p);",
                true,
            ),
            (
                "function f({ [key as string]: p }: any) { return p; } f(x);",
                true,
            ),
            ("const { ['constructor']: C } = x as any; use(C);", true),
            // Code a program builds as a string (the second re-check of #232): a file that runs one is touched, whatever its own text says.
            (
                "setTimeout('Object.defineProperty(Object.prototype, \"h\", {})', 0);",
                true,
            ),
            ("setTimeout(later as any, 0);", true),
            (
                "const s = document.createElement('script'); s.textContent = code; document.head.append(s);",
                true,
            ),
            ("img.setAttribute('onerror', code);", true),
            ("el.setAttribute(name as string, code);", true),
            ("host.innerHTML = markup as string;", true),
            ("host.innerHTML = '<img src=x onerror=run()>';", true),
            (
                "el.insertAdjacentHTML('beforeend', markup as string);",
                true,
            ),
            ("document.write(markup as string);", true),
            (
                "await import(('data:text/javascript,' + code) as string);",
                true,
            ),
            ("await import(spec as string);", true),
            ("const link = 'javascript:run()'; use(link);", true),
            ("use(window[name as string]);", true),
            ("use((window as any).eval('this'));", true),
            ("use(<img onerror=\"run()\" />);", true),
            // The accessors a plain object inherits, looked up (the second re-check).
            ("use((({}) as any).__lookupGetter__(key as string));", true),
            ("use((x as any).__lookupSetter__);", true),
            (
                "use(Object.getOwnPropertyDescriptor(x, key as string));",
                true,
            ),
            ("use(Reflect.getOwnPropertyDescriptor(x, 'a')!.get);", true),
            ("use(Object.getOwnPropertyDescriptors(x));", true),
            ("void Object.getOwnPropertyDescriptor(x, 'a');", false),
            // Code a string spells.
            ("Function('return this')();", true),
            ("new Function('return this');", true),
            ("eval('this');", true),
            ("(x as any).constructor('return this')();", true),
            // Operands nothing takes, and uses that give no prototype out.
            ("Object.keys(x);", false),
            ("Object.assign({}, x);", false),
            ("Object.defineProperty(o, 'h', {});", false),
            ("Object.freeze(x); Object.entries(x);", false),
            ("Object.prototype.hasOwnProperty.call(a, b);", false),
            (
                "const hasOwn = Object.prototype.hasOwnProperty; use(hasOwn);",
                false,
            ),
            ("use(Object.prototype.toString.call(x));", false),
            ("void (Object.prototype === x);", false),
            (
                "void (Object.getPrototypeOf(x) === Object.prototype);",
                false,
            ),
            (
                "void (typeof Object); void (typeof window !== 'undefined'); void (typeof globalThis === 'object');",
                false,
            ),
            (
                "window.addEventListener('x', f); globalThis.setTimeout(() => {}); use(window.location.href);",
                false,
            ),
            ("void (typeof eval); void (typeof Function);", false),
            (
                "use(props.defaultView ?? 'pages'); use(document.body);",
                false,
            ),
            (
                "void (x.constructor === Foo); use((x as any).constructor.name);",
                false,
            ),
            (
                "function Child() {} Child.prototype.constructor = Child;",
                false,
            ),
            (
                "const o: any = {}; o.__proto__ = base; void (o.__proto__ === base);",
                false,
            ),
            ("Reflect.has(x, 'a'); Reflect.ownKeys(x);", false),
            (
                "let a: any, b: any; ({ a, k: b } = x as any); use(a, b, Reflect.get(x, 'a'));",
                false,
            ),
            // Places that run no built code: a function written there, a literal module path, inert markup, an element of a named tag, an attribute that is no handler.
            (
                "function tick() {} setTimeout(tick, 1); setTimeout(() => {}, 2);",
                false,
            ),
            (
                "const el2 = document.createElement('div'); el2.setAttribute('class', 'x');",
                false,
            ),
            (
                "host.innerHTML = ''; host.innerHTML = '<b>x</b>'; document.write('<p>hi</p>');",
                false,
            ),
            (
                "await import('./Page'); use(<div onClick={handler} />);",
                false,
            ),
            // Keys the facts prove are none of the keys that hand out a prototype: a number, a symbol, a constant, a text that holds a digit.
            ("use(a[0]); use(a['k']);", false),
            ("for (let i = 0; i < 9; i++) use(a[i]);", false),
            ("let i = 0; i += step(); use(a[i]);", false),
            (
                "use(a[i + 1]); use(a[n - 1]); use(a[-n]); use(a[x | 0]); use(a[`k${n * 2}`]);",
                false,
            ),
            ("const KEY = 'key'; use(a[KEY]);", false),
            (
                "const first = 'a', second = 'b'; use(a[flag ? first : second]);",
                false,
            ),
            (
                "use(a[Symbol.iterator]); const s = Symbol('s'); use(a[s]);",
                false,
            ),
            ("use(a[typeof x]); use(a[!x]);", false),
            (
                "use(Reflect.get(x, 'a')); use(Reflect.get(x, 1)); use(Reflect.get(x));",
                false,
            ),
            (
                "let j: any = 0; for (j of list) { void j; } use(a[0]);",
                false,
            ),
            // A write under an unproven key and an operand nothing takes read none out.
            ("a[key as string] = 1;", false),
            ("a[key as string] += 1;", false),
            ("delete a[key as string];", false),
            (
                "void (a[key as string] === b); void typeof a[key as string];",
                false,
            ),
            ("void (Reflect.get(x, key as string) === b);", false),
            (
                "class A {} A.prototype.x = 1; (A as any).prototype.y = 2;",
                false,
            ),
            (
                "function Foo() {} Foo.prototype.bar = function () {};",
                false,
            ),
            // Not the globals: a binding of the file, a type.
            (
                "function f(Object: any) { return Object.prototype; } use(f(x));",
                false,
            ),
            // A binding of the file named `window` is no window, but the key `Object` read off it is asked of anything.
            ("const window: any = {}; use(window.Object);", true),
            ("const window: any = {}; use(window.location);", false),
            (
                "let a: Object = {}; type T = typeof Object; let h: Function = f; use({} as Object, a, h);",
                false,
            ),
        ] {
            assert_eq!(names_object_prototype(&file(source)), touched, "{source}");
        }
    }

    /// A read under a key the facts do not prove is none of the keys that hand out a prototype: the sites, and what decides each.
    #[test]
    fn a_read_under_an_unproven_key_is_told_from_the_rest() {
        for (source, reads) in [
            ("use(x[key as string]);", true),
            ("use(x[a + b]);", true),
            ("use(x['const' + 'ructor']);", true),
            ("const { [key as string]: p } = x as any; use(p);", true),
            ("use(Reflect.get(x, key as string));", true),
            ("use(Reflect.get(x, 'prototype'));", true),
            ("use(x[0]); use(x['a']); use(x[`b`]);", false),
            (
                "use(x[i + 1]); use(x[Symbol.iterator]); use(x[typeof y]);",
                false,
            ),
            ("x[key as string] = 1;", false),
            ("x[key as string]++;", false),
            ("void (x[key as string] === y);", false),
            ("use(x.a);", false),
            ("use(Reflect.get(x, 'a'));", false),
        ] {
            assert_eq!(reads_an_unproven_key(&file(source)), reads, "{source}");
        }
    }
}
