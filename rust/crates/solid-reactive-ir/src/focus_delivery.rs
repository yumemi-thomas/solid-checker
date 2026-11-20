//! ADR 11xx-b: a `focus()` call that delivers to no project code.
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
//! object runs on the stack of the call), and the program holds nothing that could run on the stack of a focus move:
//!
//! - no listener for an event of [`EVENTS`] on any element, window or document: a JSX attribute or namespaced attribute naming one
//!   (`onFocus`, `on:focus`, `oncapture:focus`, `prop:onfocus`), any spread onto an intrinsic element (whose keys no fact names), an
//!   `on<event>` property written or a key of that name, an `addEventListener` that is not a direct call with a literal type outside
//!   the list (a value, an alias, `.call`, a destructured method, a variable type), code built from markup or an event attribute, a
//!   private delegated key (`$$focusin`);
//! - no code that stands in for `focus`: a class member, an object literal member or an assignment named `focus` (a custom element's
//!   override, an impostor receiver), anywhere in the program or a loaded package, nor a write the shared standard-member premise
//!   withholds ([`crate::interproc::accessor_provenance::standard_member_replaced`]: a replacement by any receiver, alias, definer,
//!   prototype route, Proxy, `Reflect`, `Object.assign`, code built from a string, a key no fact decides, an unread module);
//! - the standard call is not withheld ([`crate::indexes::SemanticLookup::standard_call_withheld`]).
//!
//! Everything else stays what ADR 0329 made it: the obligation. `blur`, `click`, `dispatchEvent`, the dialog and popover members and
//! the bare global `focus()` are not asked. The page's own HTML (a `<body onfocus>` in an `index.html` no analysed file holds) is the
//! program's closed-world premise, as for ADR 0640 and ADR 1080's listener census.

use solid_facts::FileFacts;
use solid_facts::ast::{ArgumentLiteralFact, ClassElementKind, CodeSinkKind, PremiseClause};
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
pub(crate) fn call_shape(file: &FileFacts, call: &solid_facts::ast::CallFact) -> bool {
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

/// Whether `name` is a handler name that registers a listener for an event of [`EVENTS`]: `onfocus`, `onFocus` (cased as written),
/// `on:focus`, `oncapture:focus`, `prop:onfocus`, `attr:onfocus`.
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

/// The key a member's name denotes, written (`el.onfocus`) or a literal under brackets (`el["onfocus"]`); `None` for a key no fact names.
fn member_key(file: &FileFacts, member: &solid_facts::ast::MemberFact) -> Option<String> {
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

/// Whether `callee` (a member of `addEventListener`, or the bare name) is called directly with a literal type no focus move delivers.
fn quiet_registration(file: &FileFacts, callee: solid_facts::core::Span) -> bool {
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

/// Whether a JSX element of the file may register a listener for an event of [`EVENTS`].
fn jsx_may_listen(file: &FileFacts) -> bool {
    file.ast.jsx_elements.iter().any(|element| {
        let intrinsic = file.source_text(element.name.span).is_some_and(|name| {
            name.starts_with(|first: char| first.is_ascii_lowercase()) && !name.contains('.')
        });
        (intrinsic && !element.spreads.is_empty())
            || element
                .attributes
                .iter()
                .any(|attribute| file.source_text(attribute.name).is_none_or(names_listener))
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
                .and_then(|member| member_key(file, member))
                .is_some_and(|key| names_listener(&key))
    }) || ast.object_properties.iter().any(|property| {
        crate::kernel::property_key(file, property.key).is_some_and(|key| names_listener(&key))
    }) || ast.members.iter().any(|member| {
        member_key(file, member).is_some_and(|key| key == "addEventListener")
            && !quiet_registration(file, member.span)
    }) || ast.identifiers.iter().any(|identifier| {
        file.source_text(identifier.span) == Some("addEventListener")
            && ast.reference_declaration(identifier.span).is_none()
            && !ast
                .members
                .iter()
                .any(|member| member.property == identifier.span)
            && !quiet_registration(file, identifier.span)
    }) || ast.bindings.iter().any(|binding| {
        binding
            .object_properties
            .iter()
            .any(|property| property.key.as_deref() == Some("addEventListener"))
    }) || ast.code_sinks.iter().any(|sink| {
        matches!(
            sink.kind,
            CodeSinkKind::EventAttribute | CodeSinkKind::MarkupWrite
        )
    }) || ast
        .premise_sites
        .iter()
        .any(|site| site.clause == PremiseClause::PrivateKey)
}

/// Whether the file defines a member named `focus` that is, or may be, a function: a class member (any kind, static too), an object
/// literal member, or an assignment to one.
fn defines_focus(file: &FileFacts) -> bool {
    let ast = &file.ast;
    ast.object_properties.iter().any(|property| {
        crate::kernel::property_key(file, property.key).is_some_and(|key| key == "focus")
            && !(property.data && property.value_kind.is_proven_noncallable())
    }) || ast
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
            ) && element
                .key
                .and_then(|key| crate::kernel::property_key(file, key))
                .is_some_and(|key| key == "focus")
        })
        || ast.assignments.iter().any(|assignment| {
            ast.member_at(ast.peel_ts_sugar_span(assignment.target))
                .and_then(|member| member_key(file, member))
                .is_some_and(|key| key == "focus")
        })
}

/// Whether some analysed file (the project's and every loaded package's) may run code on the stack of a focus move: a listener for what it
/// delivers, or a stand-in for `focus`. A whole-program read, recorded as every scan of the files is.
pub(crate) fn program_may_run_code_on_focus(lookup: &SemanticLookup<'_>) -> bool {
    lookup
        .files()
        .iter()
        .any(|file| jsx_may_listen(file) || code_may_listen(file) || defines_focus(file))
}

/// The silence's record in the proof log, at the callee of the call it makes complete.
pub(crate) fn log(file: &FileFacts, call: &solid_facts::ast::CallFact) {
    crate::proof_log::silent(
        "SC9012",
        &crate::location(file.path.shared(), call.callee),
        PROOF,
        &[],
    );
}
