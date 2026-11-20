//! ADR 1151: a `focus()` call that delivers to no project code.
//!
//! The leaf-scope walk (`cleanup.rs`) refuses a `lib.dom` member that dispatches an event synchronously (ADR 0329): the listeners it
//! delivers to run on the stack of the leaf callback, under the leaf owner, where a cleanup, a flush or a creation is refused. The
//! refusal is an SC9012 `leaf-callback-opaque` obligation at the call. This module answers the one case where nothing can run: an
//! element's `focus()` in a program none of whose code may listen for what a focus move delivers, or stand in for `focus`.
//!
//! *What `focus()` delivers.* Measured in Chrome 148 (the pinned headless shell), every `on*` event type of `window` plus the legacy and
//! composition names, listened for in the capture phase on `window` and `document` and in a child frame, over 18 moves (from nothing,
//! from an edited input, into a shadow tree, a dialog, a popover, a frame and back, from a removed element, `preventScroll`): the
//! types delivered before `focus()` returns were `focus`, `focusin`, `DOMFocusIn` (the receiver's), `blur`, `focusout`, `DOMFocusOut`
//! (the element losing focus), `change` (an input edited by the user, on the element losing focus) and, on a frame boundary, `focus` and
//! `blur` at the window. A composition (an IME) active in the element losing focus is committed by the move, which delivers the
//! composition and input events of the commit: not reproducible in a headless page, named here after ADR 0642 (which measured
//! `compositionend`), never measured. [`EVENTS`] is that list.
//!
//! *The proof.* The call is `x.focus(…)` or `x?.focus(…)` whose exact declaration is `lib.dom`'s `HTMLOrSVGElement.focus` (or the
//! mixin it is named in newer libraries), its arguments inert ([`crate::kernel::evaluates_no_user_code`]: a getter on an options
//! object runs on the stack of the call), and the program holds nothing that could run on the stack of a focus move. Every name below is
//! compared by its cooked value (a decoded identifier, a string's or a template's cooked text: `addEventListener`, `"focus"`),
//! never by its source text. Every file is read, a dependency's source as the project's, except the files of the dialect's own runtime
//! (whose registrars are asked at the import, below):
//!
//! - no listener for an event of [`EVENTS`] on any element, window or document: a JSX attribute or namespaced attribute naming one
//!   (`onFocus`, `on:focus`, `oncapture:focus`, `prop:onfocus`), any JSX spread (whose keys no fact names), an `on<event>` property
//!   written (`el.onfocus = f`, `onfocus = f` on the global object, `onchange ??= h`) or a key of that name, an `addEventListener` that
//!   is not a direct call with a literal type outside the list (a value, an alias, `.call`, a destructured method, a variable type),
//!   a computed member the program may read as `addEventListener`, `focus`, `innerHTML`… (`el[add]("focus", h)`: a key no fact proves
//!   is another name, or one proven to be such a name, wherever the member's value is called or kept), code built from a string or
//!   from markup, a JSX `innerHTML`/`outerHTML`/`srcdoc` attribute whose value is not a constant free of script and event attributes,
//!   a private delegated key (`$$focusin`), the reflective routes of the shared standard-member premise (`Proxy`, `Reflect`,
//!   `Object.assign`, a prototype route, `with`) in any file, a write to a global the program does not declare under a listener's name;
//! - no code that stands in for `focus`: a class member, an object literal member or an assignment named `focus` (a custom element's
//!   override, an impostor receiver), anywhere in the program or a loaded package, nor a write the shared standard-member premise
//!   withholds ([`crate::interproc::accessor_provenance::standard_member_replaced`]: a replacement by any receiver, alias, definer,
//!   prototype route, Proxy, `Reflect`, `Object.assign`, code built from a string, a key no fact decides, an unread module);
//! - no registrar of the dialect's own runtime ([`solid_dialect::Dialect::runtime_module_listener_registrations`]): a program that
//!   imports `addEvent`, `delegateEvents`, `setProperty`, `setAttribute`, `spread`, `template`, a hydration or an asset helper, or any
//!   module of the frames tier, may register a listener the scans above cannot read, unless every call of a registrar that takes the
//!   event type or the property name as an argument passes a constant outside the focus list (`addEvent(el, "click", h)`). An export,
//!   or a module, the dialect has not audited is unknown and counts. A specifier that is no runtime package's by name but resolves
//!   into a file of one (a relative path into `node_modules`, a `paths` alias) reaches it by a name no audit lists, and counts;
//! - no code the build does not read: a module outside the program's files, an asset, a computed or glob load, and a package whose
//!   analysed files are declarations alone (TypeScript resolved the load to its `.d.ts`; the runtime source was never scanned);
//! - the page's own HTML: where the project has an `index.html` (or another page that can run code), the host-discovery grammar must
//!   read it (no inline `on*` handler), it must hold no `<script>` but the module entries it names, and each entry must be a file of
//!   the program ([`solid_facts::package_reach::PackageReachIndex::page_may_run_code`]); where it has no HTML the program is closed
//!   (ADR 0640, ADR 1080's listener census).
//!
//! The files of the dialect's own runtime packages are not read: what they register is what its exports register, which the answers above
//! hold (`addEvent` and the others are the registrars, the delegated listeners they attach deliver only handlers one of them stored, and
//! the runtime's own `change` listener on a `<select>` runs no project code). Under `--runtime-resolution required` the proof does not
//! fire, for a cause not found (ADR 1151); that is safe, a withheld proof being the fail-closed direction.
//!
//! Everything else stays what ADR 0329 made it: the obligation. `blur`, `click`, `dispatchEvent`, the dialog and popover members and
//! the bare global `focus()` are not asked.

use std::collections::BTreeSet;

use solid_dialect::{ListenerRegistrations, RuntimeListenerRole};
use solid_facts::FileFacts;
use solid_facts::ast::{
    ArgumentLiteralFact, CallFact, ClassElementKind, ComputedKeySite, ExportKind,
    ImportBindingFact, ImportKind, JsxAttributeFact, JsxAttributeValueKind, ModuleHazardKind,
    ModuleLoadConsumer, PremiseClause,
};
use solid_facts::core::Span;
use solid_facts::kernel::ValueKind;

use crate::indexes::SemanticLookup;

/// The proof's id: the proof log's `silent` lines name it, and `SOLID_CHECKER_IR_ABLATE_SILENCE` turns it off (ADR 1118's gate).
pub(crate) const PROOF: &str = "leaf_walk.focus_delivers_nothing";

/// The event types a focus move delivers before `focus()` returns, lowercase (module docs).
const EVENTS: [&str; 12] = [
    "focus",
    "blur",
    "focusin",
    "focusout",
    "domfocusin",
    "domfocusout",
    "change",
    "compositionupdate",
    "compositionend",
    "beforeinput",
    "input",
    "textinput",
];

/// The names (lowercase) under which a member's write or call registers a listener, writes markup, or stands in for `focus`, beside the handler
/// properties of [`EVENTS`] (`names_listener`): the registration and markup methods of the DOM, the attribute API an inline handler is set through,
/// and the member `focus`.
const REGISTERING_NAMES: [&str; 20] = [
    "addeventlistener",
    "attachevent",
    "focus",
    "handleevent",
    "innerhtml",
    "outerhtml",
    "insertadjacenthtml",
    "srcdoc",
    "sethtmlunsafe",
    "parsehtmlunsafe",
    "createcontextualfragment",
    "execcommand",
    "write",
    "writeln",
    "setattribute",
    "setattributens",
    "setattributenode",
    "setattributenodens",
    "setnameditem",
    "setnameditemns",
];

/// How many `const` initializers a constant text follows.
const CONSTANT_DEPTH: usize = 4;

/// The qualified names of the `lib.dom` declaration of an element's `focus`.
pub(crate) fn is_element_focus(qualified_name: &str) -> bool {
    matches!(
        qualified_name,
        "HTMLOrSVGElement.focus" | "HTMLOrSVGOrMathMLElement.focus"
    )
}

/// Whether the call is the proof's shape: a direct member call (`x.focus()`, `x?.focus()`), no `new`, no spread, and every argument a
/// value with no code in it: a primitive or an object literal of data ([`ValueKind`]). The host reads an option bag's members while it
/// converts the argument, so an argument that is an operand (a binding may hold an object with a getter) or a literal with an accessor
/// runs project code on the stack of the call.
pub(crate) fn call_shape(file: &FileFacts, call: &CallFact) -> bool {
    !call.construct
        && file
            .ast
            .member_at(file.ast.peel_ts_sugar_span(call.callee))
            .is_some()
        && call.arguments.iter().all(|argument| {
            !argument.spread
                && matches!(
                    crate::kernel::inert_evaluation(file, argument.span),
                    Some(ValueKind::Primitive | ValueKind::Data)
                )
        })
}

/// Whether `name` (a cooked name) is a handler name that registers a listener for an event of [`EVENTS`]: `onfocus`, `onFocus` (cased as
/// written), `on:focus`, `oncapture:focus`, `prop:onfocus`, `attr:onfocus`.
fn names_listener(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    let name = name
        .strip_prefix("prop:")
        .or_else(|| name.strip_prefix("attr:"))
        .unwrap_or(&name);
    name.strip_prefix("oncapture:")
        .or_else(|| name.strip_prefix("on:"))
        .is_some_and(|event| EVENTS.contains(&event))
        || name
            .strip_prefix("on")
            .is_some_and(|event| EVENTS.contains(&event))
}

/// Whether a member or key of this cooked name may register a listener, write markup or stand in for `focus` where it is called or written: a
/// handler property of [`EVENTS`], a name of [`REGISTERING_NAMES`], or a private key of Solid's delegation (`$$focusin`, `_$…`).
fn registers_by_name(name: &str) -> bool {
    names_listener(name)
        || REGISTERING_NAMES
            .iter()
            .any(|registering| name.eq_ignore_ascii_case(registering))
        || name.contains("$$")
        || name.starts_with("_$")
}

/// The key a member's name denotes, written (`el.onfocus`, `el.onfocus`) or a literal under brackets (`el["onfocus"]`), cooked; `None` for a key
/// no fact names.
fn member_name(file: &FileFacts, member: &solid_facts::ast::MemberFact) -> Option<String> {
    let ast = &file.ast;
    if ast.computed_members.binary_search(&member.span).is_ok() {
        ast.literal_computed_members
            .iter()
            .find(|literal| literal.span == member.span)
            .map(|literal| literal.key.to_string())
    } else {
        crate::kernel::property_key(file, member.property)
    }
}

/// The key of an object literal member, cooked: written, or a string literal under brackets; `None` for a key computed by an expression no literal
/// names (`[k]`, `[a + b]`), which may be any name.
fn literal_member_name(
    file: &FileFacts,
    property: &solid_facts::ast::ObjectPropertyFact,
) -> Option<String> {
    let text = file.source_text(property.key)?.trim();
    if !property.computed {
        return solid_facts::kernel::cooked_key(text);
    }
    let text = text
        .strip_prefix('[')
        .and_then(|inner| inner.strip_suffix(']'))
        .unwrap_or(text)
        .trim();
    let literal = text.starts_with(['"', '\'']) || (text.starts_with('`') && !text.contains("${"));
    literal
        .then(|| solid_facts::kernel::cooked_key(&format!("[{text}]")))
        .flatten()
}

/// Whether the member's value is no function by its own syntax (a data property of a literal value): a key that may be any name holds no code.
fn holds_no_code(property: &solid_facts::ast::ObjectPropertyFact) -> bool {
    property.data && property.value_kind.is_proven_noncallable()
}

/// The cooked text the expression at `span` denotes when it is a constant string: a string literal, a substitution-free template, or a `const`
/// binding of one (a few hops); `None` for anything else.
fn constant_text(file: &FileFacts, span: Span) -> Option<String> {
    constant_text_to(file, span, CONSTANT_DEPTH)
}

fn constant_text_to(file: &FileFacts, span: Span, depth: usize) -> Option<String> {
    let ast = &file.ast;
    let span = ast.peel_ts_sugar_span(span);
    let text = file.source_text(span)?.trim();
    let literal = text.starts_with(['"', '\'']) || (text.starts_with('`') && !text.contains("${"));
    if literal {
        // Under brackets, as a computed key: a template is a key only there, and a string reads the same.
        return solid_facts::kernel::cooked_key(&format!("[{text}]"));
    }
    if depth == 0 {
        return None;
    }
    let declaration = ast.reference_declaration(span)?;
    ast.bindings
        .iter()
        .find(|binding| {
            binding.immutable
                && !binding.in_switch_case
                && matches!(binding.shape, solid_facts::ast::BindingShape::Identifier)
                && binding.names.iter().any(|name| name.span == declaration)
        })
        .and_then(|binding| binding.initializer)
        .and_then(|initializer| constant_text_to(file, initializer, depth - 1))
}

/// Whether `callee` (a member of `addEventListener`, or the bare name) is called directly with a literal type no focus move delivers.
fn quiet_registration(file: &FileFacts, callee: Span) -> bool {
    file.ast
        .calls_with_peeled_callee(callee)
        .next()
        .is_some_and(|call| {
            !call.construct
                && call.arguments.first().is_some_and(|event| {
                    !event.spread
                        && matches!(&event.literal_value, ArgumentLiteralFact::String(name)
                            if !EVENTS.iter().any(|delivered| name.eq_ignore_ascii_case(delivered)))
                })
        })
}

/// Whether a JSX attribute may register a listener, or write markup that holds an event attribute or a script.
fn attribute_may_listen(file: &FileFacts, attribute: &JsxAttributeFact) -> bool {
    let Some(name) = file.source_text(attribute.name) else {
        return true;
    };
    if names_listener(name) {
        return true;
    }
    let lower = name.to_ascii_lowercase();
    let bare = lower
        .strip_prefix("prop:")
        .or_else(|| lower.strip_prefix("attr:"))
        .unwrap_or(&lower);
    if !matches!(bare, "innerhtml" | "outerhtml" | "srcdoc") {
        return false;
    }
    // A markup-bearing attribute: inert only when its value is a constant string the markup judgment reads as free of script and event attributes. Text
    // written as the attribute's own string literal is decoded by the JSX transform before the markup parser sees it (`&lt;` is `<`), so a character
    // reference in it is a value no text test reads.
    let inert = match attribute.value_kind {
        JsxAttributeValueKind::String => attribute
            .value
            .and_then(|value| constant_text(file, value))
            .is_some_and(|text| !text.contains('&') && crate::kernel::markup_is_inert(&text)),
        JsxAttributeValueKind::Expression => attribute
            .expression
            .and_then(|expression| constant_text(file, expression))
            .is_some_and(|text| crate::kernel::markup_is_inert(&text)),
        _ => false,
    };
    !inert
}

/// Whether a JSX element of the file may register a listener for an event of [`EVENTS`]: an attribute naming one, a markup-bearing attribute, or a
/// spread (on an element or on a component or `Dynamic` that forwards it: no fact names its keys).
fn jsx_may_listen(file: &FileFacts) -> bool {
    file.ast.jsx_elements.iter().any(|element| {
        !element.spreads.is_empty()
            || element
                .attributes
                .iter()
                .any(|attribute| attribute_may_listen(file, attribute))
    })
}

/// Whether code of the file may register a listener for an event of [`EVENTS`] other than by JSX.
fn code_may_listen(file: &FileFacts) -> bool {
    let ast = &file.ast;
    ast.assignments.iter().any(|assignment| {
        let target = ast.peel_ts_sugar_span(assignment.target);
        file.source_text(target).is_none()
            || ast
                .member_at(target)
                .and_then(|member| member_name(file, member))
                .is_some_and(|key| names_listener(&key))
    }) || ast
        .object_properties
        .iter()
        .any(|property| match literal_member_name(file, property) {
            Some(key) => names_listener(&key),
            // A key computed by an expression may be `onfocus`: a handler object handed to a registration the scans cannot read.
            None => !holds_no_code(property),
        })
        || ast.members.iter().any(|member| {
            member_name(file, member).is_some_and(|key| key == "addEventListener")
                && !quiet_registration(file, member.span)
        })
        || ast.identifiers.iter().any(|identifier| {
            // The cooked name: `addEventListener(…)` is the global `addEventListener`.
            crate::kernel::property_key(file, identifier.span).as_deref() == Some("addEventListener")
                && ast.reference_declaration(identifier.span).is_none()
                && !ast
                    .members
                    .iter()
                    .any(|member| member.property == identifier.span)
                && !quiet_registration(file, identifier.span)
        })
        || ast.bindings.iter().any(|binding| {
            binding
                .object_properties
                .iter()
                .any(|property| property.key.as_deref() == Some("addEventListener"))
        })
        // Code built from a string or from markup, wherever it is: a `<script>` made at run time, an event attribute set as text, markup written, a load
        // by a specifier nothing fixes (`MarkupWrite` and `EventAttribute` are written only where the facts do not prove the value inert).
        || !ast.code_sinks.is_empty()
        || ast.module_hazards.iter().any(|hazard| {
            matches!(
                hazard.kind,
                ModuleHazardKind::NonliteralDynamicLoading
                    | ModuleHazardKind::Eval
                    | ModuleHazardKind::OpaqueWasm
            )
        })
        // `import.meta.glob` loads files by a pattern: modules the program may not hold.
        || !ast.in_place_import_globs.is_empty()
        || ast.members.iter().any(|member| {
            file.source_text(ast.peel_ts_sugar_span(member.object)) == Some("import.meta")
                && member_name(file, member)
                    .is_some_and(|key| key == "glob" || key == "globEager")
        })
        || ast.premise_sites.iter().any(|site| premise_may_register(site.clause, &site.key))
}

/// Whether a premise site of the file may register a listener or replace `focus` by a route no scan above names: a reflective write (`Proxy`,
/// `Reflect`, `Object.assign`, a prototype route, `with`), Solid's private delegated key, or a write to a global the program does not declare
/// (`onfocus = f`, `onchange ??= h`: the global object's handler property) under a registering name or a name no fact fixes. The shared standard-member
/// premise withholds these in a project file already; a dependency's file is read here.
fn premise_may_register(clause: PremiseClause, key: &str) -> bool {
    match clause {
        PremiseClause::ProxyOrReflect
        | PremiseClause::PrototypeRoute
        | PremiseClause::With
        | PremiseClause::ObjectAssign
        | PremiseClause::PrivateKey => true,
        PremiseClause::GlobalWrite | PremiseClause::StandardKeyWrite => {
            key == "*" || registers_by_name(key)
        }
        PremiseClause::CoercionHook
        | PremiseClause::Accessor
        | PremiseClause::ShadowRoot
        | PremiseClause::KeyReader
        | PremiseClause::DispatchHandedOn => false,
    }
}

/// Whether a computed member (or computed pattern key, or `Reflect.get` key) of the file may read or write a registering member: a key nothing proves
/// is another name, or one proven to be a registering name (`el[add]("focus", h)` with `const add = "addEventListener"`), wherever the value is called,
/// written or kept. A read whose value is provably neither called nor kept (`arr[i] + 1`) runs no method of its own ([`solid_facts::ast::ComputedKeyFact::value_inert`]).
fn computed_may_register(file: &FileFacts) -> bool {
    file.ast.computed_keys.iter().any(|key| {
        if key.site == ComputedKeySite::Member && key.value_inert {
            return false;
        }
        !key.proven || key.texts.iter().any(|text| registers_by_name(text))
    })
}

/// Whether the file writes, with an escape, a word that names a registering member, handler property or key ([`solid_facts::ast::AstFacts::escaped_words`], facts
/// schema 107, ADR 1148 round 2): `\u0061ddEventListener`, `"\u0066ocus"`, `o\u006efocus` decode to the names a scan of the source text cannot see. Every scan above
/// compares a cooked value, so a name in a position a fact keys is read decoded already; this asks the same of the facts that list the escaped spellings themselves,
/// so an escaped name in a position no scan above reads (a word in a longer text, a template piece, a key of a table the program builds) still ends the proof.
fn escaped_word_registers(file: &FileFacts) -> bool {
    file.ast
        .escaped_words
        .iter()
        .any(|word| registers_by_name(word))
}

/// Whether the file declares a member named `focus` that is, or may be, a function: a class member (any kind, static too), an object
/// literal member, or an assignment to one.
fn defines_focus(file: &FileFacts) -> bool {
    let ast = &file.ast;
    ast.object_properties
        .iter()
        .any(|property| match literal_member_name(file, property) {
            Some(key) => key == "focus" && !holds_no_code(property),
            // A key computed by an expression may be `focus`.
            None => !holds_no_code(property),
        })
        || ast
            .classes
            .iter()
            .flat_map(|class| &class.elements)
            .any(|element| {
                matches!(
                    element.kind,
                    ClassElementKind::Method
                        | ClassElementKind::Getter
                        | ClassElementKind::Setter
                        | ClassElementKind::Field
                ) && (element.computed
                    || element
                        .key
                        .and_then(|key| crate::kernel::property_key(file, key))
                        .is_some_and(|key| key == "focus"))
            })
        || ast.assignments.iter().any(|assignment| {
            ast.member_at(ast.peel_ts_sugar_span(assignment.target))
                .and_then(|member| member_name(file, member))
                .is_some_and(|key| key == "focus")
        })
}

/// Whether `path` is a file of one of the dialect's own runtime packages (`…/node_modules/@solidjs/web/…`).
fn is_dialect_runtime(path: &str, runtime: &[&str]) -> bool {
    crate::plain_premise::package_name(path).is_some_and(|name| runtime.contains(&name))
}

/// What the dialect says of `module`: `None` for a module that is no runtime package's by name (the program's own or a dependency's, read as source),
/// `Some(None)` for a runtime module the dialect has not audited, `Some(Some(answer))` for an audited one.
fn runtime_answer(
    dialect: &dyn solid_dialect::Dialect,
    module: &str,
) -> Option<Option<ListenerRegistrations>> {
    dialect
        .primitive_defining_packages()
        .iter()
        .any(|package| {
            module == *package
                || module
                    .strip_prefix(package)
                    .is_some_and(|rest| rest.starts_with('/'))
        })
        .then(|| dialect.runtime_module_listener_registrations(module))
}

/// The cooked constant at argument `index` of the call, when no spread comes before it.
fn argument_text(file: &FileFacts, call: &CallFact, index: usize) -> Option<String> {
    if call
        .arguments
        .iter()
        .take(index + 1)
        .any(|argument| argument.spread)
    {
        return None;
    }
    constant_text(file, call.arguments.get(index)?.span)
}

/// Whether one call of a registrar by `role` registers nothing a focus move delivers to: the event type, or the property name, is a constant outside
/// the lists, or the markup is a constant free of script and event attributes. `Forwards` and `Opaque` registrars have no argument to read.
fn registrar_call_is_inert(file: &FileFacts, call: &CallFact, role: RuntimeListenerRole) -> bool {
    if call.construct {
        return false;
    }
    match role {
        RuntimeListenerRole::Forwards | RuntimeListenerRole::Opaque => false,
        RuntimeListenerRole::Event { argument } => argument_text(file, call, argument)
            .is_some_and(|name| !EVENTS.iter().any(|event| name.eq_ignore_ascii_case(event))),
        RuntimeListenerRole::Named { argument } => {
            argument_text(file, call, argument).is_some_and(|name| !registers_by_name(&name))
        }
        RuntimeListenerRole::Markup { argument } => argument_text(file, call, argument)
            .is_some_and(|markup| crate::kernel::markup_is_inert(&markup)),
    }
}

/// Whether every reference to the import binding is the name of a JSX element (`<Dynamic …/>`): the attributes and spreads of the element are what the
/// JSX scans read.
fn only_jsx_tags(file: &FileFacts, binding: &ImportBindingFact) -> bool {
    let ast = &file.ast;
    ast.references_to(binding.local.span)
        .iter()
        .all(|reference| {
            ast.jsx_elements.iter().any(|element| {
                element.name.span == *reference || element.closing_name == Some(*reference)
            })
        })
}

/// Whether the named import `binding` of `module` registers: the role of its export in the dialect's answer, and, for a registrar that reads an
/// argument, whether every reference is a direct call that passes a constant outside the lists. An export the module's audit does not list as a registrar
/// is inert only where it is a value export of the audited surface: a name no surface holds, used at run time, is unknown.
fn binding_may_register(
    dialect: &dyn solid_dialect::Dialect,
    file: &FileFacts,
    module: &str,
    binding: &ImportBindingFact,
    answer: Option<ListenerRegistrations>,
) -> bool {
    let name = binding
        .imported
        .as_deref()
        .or_else(|| file.source_text(binding.local.span));
    match answer {
        Some(ListenerRegistrations::None) => false,
        Some(ListenerRegistrations::Any) | None => true,
        Some(ListenerRegistrations::Exports(registrars)) => {
            let Some(name) = name else { return true };
            let Some((_, role)) = registrars.iter().find(|(export, _)| *export == name) else {
                return binding.runtime_referenced
                    && !dialect
                        .export_modules(name, solid_dialect::ExportPosition::Value)
                        .contains(&module);
            };
            // A binding handed on by a local export (`export { addEvent }`) reaches every importer of the file.
            let exported = file.ast.exports.iter().any(|export| {
                export.module.is_none()
                    && export.specifiers.iter().any(|specifier| {
                        file.source_text(specifier.local.span)
                            == file.source_text(binding.local.span)
                    })
            });
            if exported {
                return true;
            }
            match role {
                RuntimeListenerRole::Forwards => !only_jsx_tags(file, binding),
                role => file
                    .ast
                    .references_to(binding.local.span)
                    .iter()
                    .any(|reference| {
                        let mut calls = file.ast.calls_with_peeled_callee(*reference).peekable();
                        // A reference that is no callee hands the registrar on: a value use no argument reads.
                        calls.peek().is_none()
                            || calls.any(|call| !registrar_call_is_inert(file, call, *role))
                    }),
            }
        }
    }
}

/// Whether `file` takes a registrar of the dialect's own runtime into the program ([`solid_dialect::Dialect::runtime_module_listener_registrations`], asked
/// of the specifier as written, the root and every subpath). A module is the runtime's when its specifier is a runtime package or a path under one: a
/// specifier the dialect has not audited is unknown and taken to register. A named import names its export whatever the local alias is, and takes a
/// registrar only where the dialect lists that export (or any, for a module audited as `Any`); a default or namespace import exposes every export;
/// `export … from` and `export * from` hand them on to every importer; an `import()` or a `require()` reaches the exports its consumer names, and all
/// when it escapes unnamed. A specifier that is no runtime package's by name but resolves into a file of one reaches that file by a name no audit lists:
/// the build's attested resolution (`attested_path`: the specifier's resolved path) says where it lands. An import that erases takes nothing.
fn takes_registrar(
    dialect: &dyn solid_dialect::Dialect,
    file: &FileFacts,
    attested_path: &dyn Fn(Span, &str) -> Option<String>,
) -> bool {
    let runtime = dialect.primitive_defining_packages();
    // Whether any export of `module` may register (a namespace, a default, an `export *`).
    let holds = |module: &str| match runtime_answer(dialect, module) {
        None | Some(Some(ListenerRegistrations::None)) => false,
        Some(Some(ListenerRegistrations::Exports(registrars))) => !registrars.is_empty(),
        Some(Some(ListenerRegistrations::Any) | None) => true,
    };
    // Whether the export `name` of `module` may register, with no call to read.
    let names = |module: &str, name: Option<&str>| match runtime_answer(dialect, module) {
        None | Some(Some(ListenerRegistrations::None)) => false,
        Some(Some(ListenerRegistrations::Exports(registrars))) => {
            name.is_none_or(|name| registrars.iter().any(|(export, _)| *export == name))
        }
        Some(Some(ListenerRegistrations::Any) | None) => true,
    };
    // Whether merely loading `module` may: the frames tier, and a module no release was audited for, run code of their own when they load.
    let loads_rewriting = |module: &str| {
        matches!(
            runtime_answer(dialect, module),
            Some(Some(ListenerRegistrations::Any) | None)
        )
    };
    let reaches_runtime_file = |at: Span, module: &str| {
        runtime_answer(dialect, module).is_none()
            && attested_path(at, module).is_some_and(|path| is_dialect_runtime(&path, runtime))
    };
    let ast = &file.ast;
    let imported = ast
        .imports
        .iter()
        .filter(|import| !import.type_only)
        .flat_map(|import| {
            import
                .bindings
                .iter()
                .filter(|binding| !binding.type_only)
                .map(move |binding| (import.module.as_str(), binding))
        })
        .any(|(module, binding)| match binding.kind {
            ImportKind::Named => runtime_answer(dialect, module)
                .is_some_and(|answer| binding_may_register(dialect, file, module, binding, answer)),
            // A namespace or a default import exposes every export, where something uses it; the module's own load-time code runs either way.
            ImportKind::Default | ImportKind::Namespace => {
                (binding.runtime_referenced && holds(module)) || loads_rewriting(module)
            }
            // `import "m"` binds nothing, so no export is reached; the module's own load-time code runs.
            ImportKind::SideEffect => loads_rewriting(module),
        });
    let imported_equals = ast
        .import_equals
        .iter()
        .any(|import| !import.type_only && holds(import.module.as_str()));
    let re_exported = ast
        .exports
        .iter()
        .filter(|export| !export.type_only)
        .filter_map(|export| {
            Some((
                export,
                export.module.as_deref().filter(|module| holds(module))?,
            ))
        })
        .any(|(export, module)| {
            export.kind == ExportKind::All
                || export.namespace.is_some()
                || export
                    .specifiers
                    .iter()
                    .filter(|specifier| !specifier.type_only)
                    .any(|specifier| {
                        let name = file
                            .source_text(specifier.local.span)
                            .and_then(solid_facts::kernel::cooked_key);
                        names(module, name.as_deref())
                    })
        });
    let loaded = ast
        .module_loads
        .iter()
        .any(|load| match load.specifier.as_deref() {
            // A computed specifier may name any module: unread code the load-by-load scan below withholds as well.
            None => true,
            Some(module) if !holds(module) => false,
            Some(module) => match &load.consumer {
                Some(ModuleLoadConsumer::Members { names: reads }) => {
                    reads.iter().any(|name| names(module, Some(name)))
                }
                Some(ModuleLoadConsumer::Lazy { export, .. }) => names(module, Some(export)),
                None => true,
            },
        });
    let reached = ast
        .imports
        .iter()
        .filter(|import| !import.type_only)
        .any(|import| reaches_runtime_file(import.span, import.module.as_str()))
        || ast
            .import_equals
            .iter()
            .filter(|import| !import.type_only)
            .any(|import| reaches_runtime_file(import.span, import.module.as_str()))
        || ast
            .exports
            .iter()
            .filter(|export| !export.type_only)
            .any(|export| {
                export
                    .module
                    .as_deref()
                    .is_some_and(|module| reaches_runtime_file(export.span, module))
            })
        || ast.module_loads.iter().any(|load| {
            load.specifier
                .as_deref()
                .is_some_and(|module| reaches_runtime_file(load.span, module))
        });
    imported || imported_equals || re_exported || loaded || reached
}

/// Whether the page may run code the analysed files do not hold ([`solid_facts::package_reach::PackageReachIndex::page_may_run_code`]).
fn page_may_run_code(lookup: &SemanticLookup<'_>) -> bool {
    lookup
        .package_reach()
        .is_some_and(|index| index.page_may_run_code)
}

/// Whether some analysed file (the project's and every loaded package's, but the dialect's own runtime) may run code on the stack of a focus move: a
/// listener for what it delivers, a stand-in for `focus`, a registrar of the runtime, code the build does not read, or the page's own HTML. A
/// whole-program read, recorded as every scan of the files is.
pub(crate) fn program_may_run_code_on_focus(lookup: &SemanticLookup<'_>) -> bool {
    if page_may_run_code(lookup) {
        return true;
    }
    let dialect = lookup.dialect;
    let runtime = dialect.primitive_defining_packages();
    let files = lookup.files();
    let program: BTreeSet<&str> = files.iter().map(|file| file.path.as_str()).collect();
    let imports = lookup.resolved_imports();
    files
        .iter()
        .filter(|file| !is_dialect_runtime(file.path.as_str(), runtime))
        .any(|file| {
            let attested = |at: Span, module: &str| {
                imports.and_then(|imports| {
                    match imports.specifier(file.path.as_str(), at, module) {
                        solid_facts::SpecifierAttestation::Attested(row) => {
                            Some(row.resolved_path.to_string())
                        }
                        solid_facts::SpecifierAttestation::Unattested => None,
                    }
                })
            };
            jsx_may_listen(file)
                || code_may_listen(file)
                || defines_focus(file)
                || computed_may_register(file)
                || escaped_word_registers(file)
                || takes_registrar(dialect, file, &attested)
                || crate::plain_premise::loads_unscanned_code(lookup, file, &program, &|module| {
                    runtime_answer(dialect, module).is_some()
                })
        })
}

/// The silence's record in the proof log, at a span a row it removes would stand at.
pub(crate) fn log(path: &std::sync::Arc<str>, at: Span) {
    crate::proof_log::silent(
        "SC9012",
        &crate::location(std::sync::Arc::clone(path), at),
        PROOF,
        &[],
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use solid_facts::compiler::{COMPILER_FACTS_PROTOCOL, ExecutionMap};
    use solid_facts::core::{Generation, SourceHash, SourcePath};
    use std::sync::Arc;

    fn facts(source: &str) -> FileFacts {
        FileFacts {
            generation: Generation::new(1).unwrap(),
            path: SourcePath::new("app.tsx").unwrap(),
            source_hash: SourceHash::of(source),
            source: Arc::from(source),
            ast: Arc::new(solid_facts::ast::extract("app.tsx", source).unwrap()),
            compiler: Arc::new(ExecutionMap {
                compiler_facts_protocol: COMPILER_FACTS_PROTOCOL,
                source_hash: SourceHash::of(source),
                semantic_model: Default::default(),
                tracked_regions: Vec::new(),
                untracked_regions: Vec::new(),
                discarded_regions: Vec::new(),
                ownership_regions: Vec::new(),
                callback_roles: Vec::new(),
                jsx_operations: Vec::new(),
            }),
        }
    }

    /// The events Chrome 148 delivered before `focus()` returned over the 18 moves of the probe (module docs): each is in the list a
    /// census asks about, so a listener for it is one.
    #[test]
    fn every_event_a_focus_move_was_measured_to_deliver_is_one_a_listener_is_asked_about() {
        for event in [
            "focus",
            "blur",
            "focusin",
            "focusout",
            "domfocusin",
            "domfocusout",
            "change",
            "compositionend",
        ] {
            assert!(EVENTS.contains(&event), "{event}");
            assert!(names_listener(&format!("on{event}")), "{event}");
        }
    }

    #[test]
    fn a_handler_name_is_one_by_its_event_in_every_spelling_the_runtime_takes() {
        for name in [
            "onfocus",
            "onFocus",
            "onBlur",
            "onFocusIn",
            "onFocusOut",
            "onChange",
            "onInput",
            "onCompositionEnd",
            "on:focus",
            "oncapture:blur",
            "prop:onfocus",
            "attr:onchange",
        ] {
            assert!(names_listener(name), "{name}");
        }
        for name in [
            "onClick",
            "onKeyDown",
            "onScroll",
            "onSubmit",
            "on",
            "onfoc",
            "focus",
            "change",
            "class",
            "ref",
        ] {
            assert!(!names_listener(name), "{name}");
        }
    }

    #[test]
    fn a_registering_name_is_a_handler_property_a_dom_registration_or_markup_method_or_a_private_delegated_key()
     {
        for name in [
            "onfocus",
            "addEventListener",
            "ADDEVENTLISTENER",
            "focus",
            "innerHTML",
            "outerHTML",
            "insertAdjacentHTML",
            "srcdoc",
            "setAttribute",
            "setAttributeNS",
            "$$focusin",
            "_$host",
        ] {
            assert!(registers_by_name(name), "{name}");
        }
        for name in [
            "onclick",
            "click",
            "textContent",
            "className",
            "value",
            "length",
            "0",
        ] {
            assert!(!registers_by_name(name), "{name}");
        }
    }

    #[test]
    fn the_element_focus_declarations_are_the_two_names_lib_dom_gives_it() {
        assert!(is_element_focus("HTMLOrSVGElement.focus"));
        assert!(is_element_focus("HTMLOrSVGOrMathMLElement.focus"));
        for name in [
            "Window.focus",
            "focus",
            "HTMLElement.click",
            "HTMLOrSVGElement.blur",
        ] {
            assert!(!is_element_focus(name), "{name}");
        }
    }

    #[test]
    fn jsx_listens_by_attribute_name_or_by_any_spread_or_by_markup_it_cannot_prove_inert() {
        for (source, listens) in [
            ("const a = <input onFocus={() => 1} />;", true),
            ("const a = <input onBlur={() => 1} />;", true),
            ("const a = <div onFocusIn={() => 1} />;", true),
            ("const a = <Field onChange={() => 1} />;", true),
            ("const a = <input onfocus=\"run()\" />;", true),
            ("const a = <input {...p} />;", true),
            // A spread onto a component or `Dynamic` forwards keys no fact names.
            ("const a = <Field {...p} />;", true),
            (
                "const a = <Dynamic component=\"input\" {...Object.fromEntries([[\"onFocus\", h]])} />;",
                true,
            ),
            (
                "const a = <input onClick={() => 1} onKeyDown={() => 1} />;",
                false,
            ),
            ("const a = <input value=\"x\" class=\"y\" />;", false),
            // A markup-bearing attribute: inert only as a constant free of event attributes and scripts.
            ("const a = <div innerHTML=\"<b>bold</b>\" />;", false),
            ("const a = <div innerHTML={'<b>bold</b>'} />;", false),
            (
                "const html = '<b>bold</b>'; const a = <div innerHTML={html} />;",
                false,
            ),
            (
                "const a = <div innerHTML={'<input onfocus=\"hook()\">'} />;",
                true,
            ),
            (
                "const a = <div innerHTML={'<input o\\u006efocus=\"hook()\">'} />;",
                true,
            ),
            (
                "const a = <div innerHTML={`<img onerror=\"hook()\">`} />;",
                true,
            ),
            (
                "const a = <div innerHTML={'<script>hook()</script>'} />;",
                true,
            ),
            ("const a = <div innerHTML={html} />;", true),
            ("const a = <div innerHTML={read()} />;", true),
            (
                "const a = <div prop:innerHTML={'<input onfocus=\"x\">'} />;",
                true,
            ),
            (
                "const a = <div outerHTML={'<input onblur=\"x\">'} />;",
                true,
            ),
            // A frame of constant markup with no script or event attribute holds no code; one with either, or a value no fact fixes, may.
            ("const a = <iframe srcdoc={'<p>hi</p>'} />;", false),
            ("const a = <iframe srcdoc=\"<p>hi</p>\" />;", false),
            (
                "const a = <iframe srcdoc={'<script>hook()</script>'} />;",
                true,
            ),
            (
                "const a = <iframe srcdoc={'<body onfocus=\"hook()\">'} />;",
                true,
            ),
            ("const a = <iframe srcdoc={html} />;", true),
            // JSX decodes a character reference in an attribute's string before the markup parser reads it.
            (
                "const a = <div innerHTML=\"&lt;img onerror=hook()&gt;\" />;",
                true,
            ),
            (
                "const a = <div innerHTML=\"&#60;img onerror=hook()&#62;\" />;",
                true,
            ),
            (
                "const a = <div textContent={'<input onfocus=\"x\">'} />;",
                false,
            ),
        ] {
            assert_eq!(jsx_may_listen(&facts(source)), listens, "{source}");
        }
    }

    #[test]
    fn code_listens_by_a_registration_a_handler_property_or_a_key_of_one() {
        for (source, listens) in [
            ("el.addEventListener('focus', f);", true),
            ("el.addEventListener('FOCUSIN', f);", true),
            ("el.addEventListener(type, f);", true),
            ("el.addEventListener('keydown', f);", false),
            ("el.addEventListener('scroll', f, true);", false),
            ("const add = el.addEventListener.bind(el);", true),
            ("el['addEventListener']('focus', f);", true),
            ("const { addEventListener } = el;", true),
            ("el.onfocus = f;", true),
            ("el['onchange'] = f;", true),
            ("el.onclick = f;", false),
            ("Object.assign(el, { onblur: f });", true),
            // A literal source writes only its keys, each judged as a written key; a source no fact fixes may write any.
            ("Object.assign(el, { onclick: f });", false),
            ("Object.assign(el, { onfocus: f });", true),
            ("Object.assign(el, props);", true),
            (
                "Object.assign(el, Object.fromEntries([['onfocus', f]]));",
                true,
            ),
            ("Object.assign(el, { [evt]: f });", true),
            ("const o = { [k]: 1 };", false),
            ("const o = { 'prop:onfocus': f };", true),
            ("el.setAttribute('onfocus', 'run()');", true),
            ("el.id = 'a';", false),
            ("el.focus();", false),
            // The global object's own handler properties and `addEventListener`.
            ("onfocus = f;", true),
            ("onchange ??= h;", true),
            ("onblur ||= h;", true),
            ("window.onfocus = f;", true),
            ("globalThis['onblur'] = f;", true),
            ("addEventListener('focus', f);", true),
            ("addEventListener('keydown', f);", false),
            ("onclick = f;", false),
            // Code the program builds, and loads it does not fix.
            ("eval('el.addEventListener(\"focus\", f)');", true),
            ("new Function('el', 'el.onfocus = f')(el);", true),
            ("setTimeout('hook()', 0);", true),
            ("el.innerHTML = html;", true),
            ("el.insertAdjacentHTML('beforeend', html);", true),
            ("iframe.srcdoc = html;", true),
            ("import(name);", true),
            ("const m = import.meta.glob('./x/*.ts');", true),
            // A reflective route in a dependency's file.
            ("Reflect.set(el, 'onfocus', f);", true),
            ("Object.defineProperty(el, 'focus', { value: f });", true),
            ("const p = new Proxy(el, {});", true),
            // Solid's private delegated key.
            ("el.$$focusin = f;", true),
            ("el['$$' + 'focusin'] = f;", true),
        ] {
            assert_eq!(
                code_may_listen(&facts(source)) || computed_may_register(&facts(source)),
                listens,
                "{source}"
            );
        }
    }

    /// The coordinator's round-1 item: a spelling is compared by its cooked value, so a unicode escape in an identifier, a string or a template names the
    /// same member, event type or key as its plain spelling, and the scans see it.
    #[test]
    fn an_escaped_spelling_is_the_same_registration_as_its_plain_one() {
        for (source, listens) in [
            // The method `addEventListener`, escaped in an identifier (member and bare global), and a literal key escaped in a string or a template.
            ("el.\\u0061ddEventListener('focus', f);", true),
            ("\\u0061ddEventListener('focus', f);", true),
            ("el['\\u0061ddEventListener']('focus', f);", true),
            ("el[`\\u0061ddEventListener`]('focus', f);", true),
            ("const { \\u0061ddEventListener } = el;", true),
            ("const { '\\u0061ddEventListener': add } = el;", true),
            // The event type escaped in the string the registration passes: a listener for `focus`.
            ("el.addEventListener('\\u0066ocus', f);", true),
            ("el.addEventListener('\\x66ocus', f);", true),
            ("el.addEventListener('\\u{66}ocus', f);", true),
            ("el.addEventListener(`\\u0066ocus`, f);", true),
            // A type that is no focus-family event stays quiet even escaped.
            ("el.addEventListener('\\u006beydown', f);", false),
            // The handler property escaped: an identifier, a string key, an object key.
            ("el.\\u006fnfocus = f;", true),
            ("el['\\u006fnfocus'] = f;", true),
            ("el[`\\u006fnblur`] = f;", true),
            ("el.o\\u006efocus = f;", true),
            ("Object.assign(el, { \\u006fnfocus: f });", true),
            ("Object.assign(el, { '\\u006fnfocus': f });", true),
            ("const o = { ['\\u006fnchange']: f };", true),
            // The global object's handler property, escaped.
            ("\\u006fnfocus = f;", true),
            // A string escaped to a registering member through a constant.
            (
                "const add = '\\u0061ddEventListener'; el[add]('focus', f);",
                true,
            ),
            (
                "const add = `\\u0061ddEventListener`; el[add]('focus', f);",
                true,
            ),
            // Markup that holds an escaped attribute name, in a constant the markup judgment reads.
            ("el.setAttribute('\\u006fnfocus', 'run()');", true),
        ] {
            assert_eq!(
                code_may_listen(&facts(source)) || computed_may_register(&facts(source)),
                listens,
                "{source}"
            );
        }
    }

    /// ADR 1148 round 2's facts (`escaped_words`): a word written with an escape is asked decoded, in any position.
    #[test]
    fn a_word_written_with_an_escape_that_names_a_registering_member_ends_the_proof_wherever_it_stands()
     {
        for (source, registers) in [
            ("const n = '\\u0061ddEventListener';", true),
            ("const n = `\\u0061ddEventListener`;", true),
            ("const n = 'call \\u0061ddEventListener here';", true),
            ("class A { static \\u0066ocus() {} }", true),
            ("const o = { \\u006fnfocus: 1 };", true),
            ("const t = { k: `\\u0069nnerHTML` };", true),
            ("const t = { k: `x \\u0069nnerHTML` };", true),
            ("const m = 'set\\u0041ttribute';", true),
            // An escape that decodes to no registering word, and no escape at all.
            ("const n = 'a\\nb';", false),
            ("console.log('Press a key\\n');", false),
            ("const n = '\\u0063lick';", false),
            ("const n = 'addEventListener';", false),
            ("el.addEventListener('keydown', f);", false),
        ] {
            assert_eq!(
                escaped_word_registers(&facts(source)),
                registers,
                "{source}"
            );
        }
    }

    #[test]
    fn a_member_named_focus_is_a_stand_in_wherever_it_is_defined() {
        for (source, defined) in [
            ("class A extends HTMLElement { focus() {} }", true),
            ("class A { static focus() {} }", true),
            ("class A { get focus() { return 1; } }", true),
            ("class A { focus = () => 1; }", true),
            ("const o = { focus() {} };", true),
            ("const o = { 'focus': () => 1 };", true),
            ("const o = { get focus() { return 1; } };", true),
            // A key computed by an expression may be any name; a literal under brackets is read; a datum holds no code.
            ("const o = { [k]: () => 1 };", true),
            ("const o = { ['foc' + 'us']: () => 1 };", true),
            ("const o = { ['focus']: () => 1 };", true),
            ("const o = { ['blur']: () => 1 };", false),
            ("const o = { [k]: 1 };", false),
            ("class A extends HTMLElement { [k]() {} }", true),
            ("class A { ['blur']() {} }", true),
            ("el.focus = f;", true),
            ("el['focus'] = f;", true),
            ("const o = { focus: 1 };", false),
            ("class A { blur() {} }", false),
            ("const o = { focused: () => 1 };", false),
            ("el.focus();", false),
            // The same names, escaped: the cooked key is `focus`.
            ("class A extends HTMLElement { f\\u006fcus() {} }", true),
            ("class A { 'f\\u006fcus'() {} }", true),
            ("class A { static ['f\\u006fcus']() {} }", true),
            ("const o = { f\\u006fcus() {} };", true),
            ("const o = { 'f\\u006fcus': () => 1 };", true),
            ("const o = { [`f\\u006fcus`]: () => 1 };", true),
            ("el.f\\u006fcus = f;", true),
            ("el['f\\u006fcus'] = f;", true),
            ("el[`f\\u006fcus`] = f;", true),
        ] {
            assert_eq!(defines_focus(&facts(source)), defined, "{source}");
        }
    }

    #[test]
    fn a_computed_member_is_a_registration_unless_its_key_is_proven_another_name_or_its_value_is_inert()
     {
        for (source, registers) in [
            // The reviewer's F3: a constant that names the method.
            ("const add = 'addEventListener'; el[add]('focus', h);", true),
            ("el['add' + 'EventListener']('focus', h);", true),
            (
                "const k = cond ? 'addEventListener' : 'removeEventListener'; el[k]('focus', h);",
                true,
            ),
            // A key nothing proves, on any receiver, whose value is called, written or kept.
            ("el[name]('focus', h);", true),
            ("el[name] = h;", true),
            ("const f = el[name];", true),
            ("use(el[name]);", true),
            ("const { [name]: f } = el;", true),
            ("Reflect.get(el, name);", true),
            // A constant that names a registering member is one wherever it is read.
            ("const k = 'innerHTML'; el[k] = html;", true),
            ("const k = 'focus'; el[k] = f;", true),
            ("const k = 'onfocus'; el[k] = f;", true),
            // A key proven to be another name, or a number; a read that is neither called nor kept.
            ("const k = 'value'; el[k] = 1;", false),
            ("const k = 'textContent'; el[k] = 'x';", false),
            ("const i = 0; items[i].x;", false),
            ("const total = a[i] + 1;", false),
            ("if (a[i]) { go(); }", false),
            ("void a[i];", false),
        ] {
            assert_eq!(computed_may_register(&facts(source)), registers, "{source}");
        }
    }

    #[test]
    fn a_focus_call_is_the_proofs_shape_with_no_argument_or_data_only() {
        for (source, shape) in [
            ("el.focus();", true),
            ("el?.focus();", true),
            ("el!.focus();", true),
            ("el.focus({ preventScroll: true });", true),
            ("el.focus(null);", true),
            // An identifier is an operand, `undefined` included: the proof asks nothing it cannot read.
            ("el.focus(undefined);", false),
            ("el.focus(options);", false),
            ("el.focus({ get preventScroll() { return true; } });", false),
            ("el.focus({ preventScroll });", false),
            ("el.focus(...args);", false),
            ("el.focus(read());", false),
            ("focus();", false),
            ("new el.focus();", false),
        ] {
            let file = facts(source);
            let call = file
                .ast
                .calls
                .iter()
                .find(|call| {
                    file.source_text(call.callee)
                        .is_some_and(|text| text.ends_with("focus"))
                })
                .expect("the focus call");
            assert_eq!(call_shape(&file, call), shape, "{source}");
        }
    }

    /// The constant text an argument holds: a literal, a substitution-free template, or a `const` of one, cooked.
    #[test]
    fn a_constant_text_is_a_cooked_literal_or_a_const_of_one() {
        for (source, text) in [
            ("f('click');", Some("click")),
            ("f(\"click\");", Some("click")),
            ("f(`click`);", Some("click")),
            ("f('\\u0066ocus');", Some("focus")),
            ("f(`\\u0066ocus`);", Some("focus")),
            ("const e = 'blur'; f(e);", Some("blur")),
            ("const a = 'fo'; const e = a; f(e);", Some("fo")),
            ("f(`a${b}`);", None),
            ("f(name);", None),
            ("let e = 'blur'; f(e);", None),
            ("f(a + b);", None),
            ("f(1);", None),
        ] {
            let file = facts(source);
            let call = file
                .ast
                .calls
                .iter()
                .find(|call| file.source_text(call.callee) == Some("f"))
                .expect("the call");
            let found = constant_text(&file, call.arguments[0].span);
            assert_eq!(found.as_deref(), text, "{source}");
        }
    }

    /// A runtime module taken into a file, by the dialect's audited answers (ADR 1148's shape, a sibling question).
    fn takes(source: &str) -> bool {
        takes_with(source, &|_, _| None)
    }

    fn takes_with(source: &str, attested: &dyn Fn(Span, &str) -> Option<String>) -> bool {
        let dialect = solid_dialect::Solid2::default();
        takes_registrar(&dialect, &facts(source), attested)
    }

    #[test]
    fn an_import_of_a_registrar_of_the_runtime_is_a_registration_unless_every_call_passes_a_constant_outside_the_lists()
     {
        for (source, takes_one) in [
            // The registrars.
            (
                "import { addEvent } from '@solidjs/web'; addEvent(el, 'focus', h, false);",
                true,
            ),
            (
                "import { addEvent } from '@solidjs/web'; addEvent(el, 'focusin', h, true);",
                true,
            ),
            (
                "import { addEvent as add } from '@solidjs/web'; add(el, 'blur', h);",
                true,
            ),
            (
                "import { addEvent } from '@solidjs/web'; addEvent(el, name, h);",
                true,
            ),
            (
                "import { addEvent } from '@solidjs/web'; const f = addEvent; f(el, 'click', h);",
                true,
            ),
            (
                "import { addEvent } from '@solidjs/web'; addEvent(el, ...args);",
                true,
            ),
            (
                "import { delegateEvents } from '@solidjs/web'; delegateEvents(['focusin']);",
                true,
            ),
            (
                "import { delegateEvents } from '@solidjs/web'; delegateEvents(['click']);",
                true,
            ),
            (
                "import { setAttribute } from '@solidjs/web'; setAttribute(el, 'onfocus', 'hook()');",
                true,
            ),
            (
                "import { setAttribute } from '@solidjs/web'; setAttribute(el, name, 'x');",
                true,
            ),
            (
                "import { setAttributeNS } from '@solidjs/web'; setAttributeNS(el, ns, 'onfocus', 'x');",
                true,
            ),
            (
                "import { setProperty } from '@solidjs/web'; setProperty(el, 'innerHTML', html);",
                true,
            ),
            (
                "import { setProperty } from '@solidjs/web'; setProperty(el, 'focus', f);",
                true,
            ),
            (
                "import { spread } from '@solidjs/web'; spread(el, props);",
                true,
            ),
            (
                "import { assign } from '@solidjs/web'; assign(el, props);",
                true,
            ),
            (
                "import { template } from '@solidjs/web'; template('<input onfocus=\"hook()\">');",
                true,
            ),
            (
                "import { template } from '@solidjs/web'; template(html);",
                true,
            ),
            (
                "import { hydrate } from '@solidjs/web'; hydrate(() => 1, el);",
                true,
            ),
            ("import { useHead } from '@solidjs/web'; useHead(x);", true),
            // Constants outside the lists: no focus-family listener, no markup with a handler, no registering property name.
            (
                "import { addEvent } from '@solidjs/web'; addEvent(el, 'click', h, true);",
                false,
            ),
            (
                "import { addEvent } from '@solidjs/web'; addEvent(el, 'keydown', h);",
                false,
            ),
            (
                "import { addEvent } from '@solidjs/web'; const e = 'click'; addEvent(el, e, h);",
                false,
            ),
            (
                "import { setAttribute } from '@solidjs/web'; setAttribute(el, 'class', 'x');",
                false,
            ),
            (
                "import { setProperty } from '@solidjs/web'; setProperty(el, 'textContent', 'x');",
                false,
            ),
            (
                "import { template } from '@solidjs/web'; template('<div></div>');",
                false,
            ),
            // An export the dialect audited as registering nothing, and the controls.
            (
                "import { render, Show, For, Portal, createComponent } from '@solidjs/web'; render(() => 1, el);",
                false,
            ),
            (
                "import { isServer, className, style, insert, effect, memo } from '@solidjs/web'; className(el, 'a');",
                false,
            ),
            (
                "import { createSignal, onSettled, createEffect } from 'solid-js';",
                false,
            ),
            ("import * as S from 'solid-js'; S.createSignal(1);", false),
            // A type import, an erased import and an unused import register nothing.
            ("import type { addEvent } from '@solidjs/web';", false),
            ("import { type addEvent } from '@solidjs/web';", false),
            ("import { addEvent } from '@solidjs/web';", false),
            // A namespace or a default import exposes every export; a re-export hands them on.
            (
                "import * as web from '@solidjs/web'; web.addEvent(el, 'click', h);",
                true,
            ),
            (
                "import web from '@solidjs/web'; web.addEvent(el, 'click', h);",
                true,
            ),
            ("import * as web from '@solidjs/web'; const n = web;", true),
            ("export { addEvent } from '@solidjs/web';", true),
            ("export { render } from '@solidjs/web';", false),
            ("export * from '@solidjs/web';", true),
            ("export * as web from '@solidjs/web';", true),
            ("export * from 'solid-js';", false),
            // A dynamic load reaches the exports its consumer names, and all when it escapes.
            ("const { addEvent } = await import('@solidjs/web');", true),
            ("const { render } = await import('@solidjs/web');", false),
            (
                "const web = await import('@solidjs/web'); web.addEvent(el, 'click', h);",
                true,
            ),
            ("const web = require('@solidjs/web');", true),
            ("await import(name);", true),
            // The frames tier is audited as any, and a subpath nobody audited is unknown.
            (
                "import { createFrame } from '@solidjs/web/frames/client';",
                true,
            ),
            ("import '@solidjs/web/frames/client';", true),
            ("import { x } from '@solidjs/web/frames/new-tier';", true),
            ("import { x } from '@solidjs/web/new-tier';", true),
            ("import { x } from '@solidjs/web/dist/web.js';", true),
            ("import { x } from '@solidjs/web/storage';", false),
            ("import { x } from '@solidjs/web/serialization';", false),
            (
                "import { x } from '@solidjs/web/server-functions/client';",
                false,
            ),
            // `jsx-runtime` is the root's file.
            (
                "import { addEvent } from '@solidjs/web/jsx-runtime'; addEvent(el, 'focus', h);",
                true,
            ),
            // A module that is no runtime package's is read as source by the other scans, not here.
            (
                "import { addEvent } from 'some-lib'; addEvent(el, 'focus', h);",
                false,
            ),
            ("import { x } from '@solidjs/router';", false),
            ("import { x } from './local';", false),
            // A name of the runtime's the program declares itself is no import.
            (
                "function addEvent(a, b, c) {} addEvent(el, 'focus', h);",
                false,
            ),
        ] {
            assert_eq!(takes(source), takes_one, "{source}");
        }
    }

    #[test]
    fn dynamic_is_judged_where_it_is_a_jsx_tag_and_is_a_registration_anywhere_else() {
        for (source, takes_one) in [
            (
                "import { Dynamic } from '@solidjs/web'; const a = <Dynamic component=\"input\" />;",
                false,
            ),
            (
                "import { Dynamic } from '@solidjs/web'; const a = <Dynamic component=\"input\">x</Dynamic>;",
                false,
            ),
            (
                "import { Dynamic } from '@solidjs/web'; const a = createComponent(Dynamic, { component: 'input' });",
                true,
            ),
            (
                "import { Dynamic } from '@solidjs/web'; const C = Dynamic; const a = <C component=\"input\" />;",
                true,
            ),
            (
                "import { Dynamic } from '@solidjs/web'; export { Dynamic };",
                true,
            ),
            (
                "import { dynamic } from '@solidjs/web'; const D = dynamic(() => 'input', {});",
                true,
            ),
        ] {
            assert_eq!(takes(source), takes_one, "{source}");
        }
    }

    /// 1148 round 1b: a specifier that is no runtime package's by name but resolves into a file of one (a relative path into `node_modules`, a `paths` alias)
    /// reaches it by a name no audit lists.
    #[test]
    fn a_specifier_that_resolves_into_a_runtime_file_is_a_registration() {
        let into_web: &dyn Fn(Span, &str) -> Option<String> = &|_, specifier| {
            (specifier == "../node_modules/@solidjs/web/dist/web.js"
                || specifier == "#web"
                || specifier == "web-alias")
                .then(|| "/p/node_modules/@solidjs/web/dist/web.js".to_string())
        };
        for (source, takes_one) in [
            (
                "import { addEvent } from '../node_modules/@solidjs/web/dist/web.js'; addEvent(el, 'click', h);",
                true,
            ),
            (
                "import { render } from '../node_modules/@solidjs/web/dist/web.js';",
                true,
            ),
            (
                "import { addEvent } from 'web-alias'; addEvent(el, 'click', h);",
                true,
            ),
            ("export { render } from 'web-alias';", true),
            ("const m = await import('web-alias');", true),
            ("import x = require('web-alias');", true),
            // The same specifiers resolve nowhere: the other scans' unread module, not this scan's.
            ("import { x } from './local';", false),
        ] {
            assert_eq!(takes_with(source, into_web), takes_one, "{source}");
        }
        // Another package's file reached by an alias is read as source.
        let into_lib: &dyn Fn(Span, &str) -> Option<String> =
            &|_, _| Some("/p/node_modules/some-lib/index.js".to_string());
        assert!(!takes_with("import { x } from 'lib-alias';", into_lib));
    }
}
