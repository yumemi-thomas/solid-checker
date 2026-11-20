//! ADR 1148: a `change` handler of a literal checkbox or radio input keeps the host-delivery proof.
//!
//! ADR 0642 keeps `change` out of [`crate::runtime_semantics::event_keeps_delivery_proof`]: a text control (and a select) delivers it by a state change with no
//! call to name, whatever makes the focused element leave (`blur()`, a move, a removal, `disabled = true`, Solid's own unmount), on the stack of whatever made
//! it. So a handler of `change` may run inside an owned scope, and the placement of its body is unknown (`execution_role.denied-event`).
//!
//! A checkbox or a radio is delivered `change` by its click activation only. Measured in Chrome 148 (the runtime oracle's headless shell 1223; the scenarios and
//! their events are in `docs/decisions.md`, ADR 1148): a user's click, the Space key (a checkbox) and the arrow keys (a radio group) deliver `click`, `input` and
//! `change` on the host's stack; `el.click()`, `label.click()` and `dispatchEvent(new MouseEvent("click"))` deliver them synchronously inside the call, and
//! `dispatchEvent(new Event("change"))` delivers the one event; nothing else does. A write of `checked` delivers nothing, and `form.reset()`, `blur()`, `focus()`
//! elsewhere, `remove()` and `disabled = true` after a click deliver no `change` beyond the click's own. Every deliverer is the host (an empty stack) or a call the dispatch census
//! already counts (`HTMLElement.click`, `EventTarget.dispatchEvent`: [`crate::runtime_semantics::delivers_synchronously`]), exactly as for `click`, so the veto that
//! withholds a `click` handler's proofs where a dispatch may deliver inside an owned scope (ADR 1125) withholds this one's too.
//!
//! That holds only for a control that stays a checkbox or a radio. Retyped to text (`type = "text"`, `setAttribute("type", …)`, `removeAttribute("type")`) the same
//! element delivers `change` as a text control does: at the user's Tab, and synchronously inside a `remove()` or a `focus()` elsewhere (measured: `type = "text"` and
//! Tab, `setAttribute` and `remove()`, `removeAttribute` and `focus()`), and then its handler may run anywhere. So the proof asks, of the element: an intrinsic `<input>` with one plain `type` attribute that is the string `checkbox` or `radio`, no spread, no
//! `ref`, no `is`, and no namespaced attribute that can write the control (`use:`, `prop:`, `attr:`, `bool:`); and of the program: that delivery is intact (no
//! Proxy, `Reflect`, `with`, `Object.assign` onto a value not made in place, definer or code built from a string: ADR 1125's premise, which these would break) and
//! that no file the program runs may write an element's `type` ([`retype_hazard`]): a project file or a dependency's source, since a dependency needs no element handed
//! to it (`document.querySelectorAll("input").forEach((i) => { i.type = "text"; })`). A dependency whose source the program does not hold is the delivery premise's unread
//! module. The dialect's own runtime packages are not read (ADR 0027: the dialect states what its runtime does; its writes of a property are the compiler's, on elements
//! whose `type` is dynamic or spread); the exports of theirs that write by a name are the dialect's answer ([`solid_dialect::Dialect::element_property_writers`]), asked of
//! every other file as exact imported symbols, never by spelling. Code outside the analyzed program is the kind of premise ADR 0300 states.
//!
//! `SOLID_CHECKER_CHANGE_CENSUS=<file|dir>` records, per project that holds such a handler, the two program answers, the delivery premise's first hazard and the first
//! place that may retype (a measurement; it moves no verdict).
//!
//! The permit sits in [`crate::execution_role::handler_event_denied`], read by every consumer of a handler's event, and ablating it
//! (`SOLID_CHECKER_IR_ABLATE_ARM=change_delivery.checkable-input-activation`, or the same id as a silence) denies `change` again, as before. Each decision that
//! relies on it names the arm in the proof log.

use std::sync::OnceLock;

use solid_facts::ast::{
    ComputedKeySite, ExportKind, ImportKind, JsxAttributeValueKind, JsxElementFact,
    ModuleLoadConsumer,
};
use solid_facts::core::Span;
use solid_facts::{FileFacts, SpecifierAttestation};

use crate::ExecutionRole;
use crate::indexes::SemanticLookup;
use crate::role_log::role_arm;

/// The id of the proof's arm: the proof log names it, and `SOLID_CHECKER_IR_ABLATE_ARM` (or `SOLID_CHECKER_IR_ABLATE_SILENCE`) turns the permit off.
pub(crate) const ACTIVATION_ONLY: &str = "change_delivery.checkable-input-activation";

/// The values of a `type` attribute whose control a `change` reaches by click activation alone.
const CHECKABLE_TYPES: [&str; 2] = ["checkbox", "radio"];

/// The DOM's own names, whose mention anywhere in the program (a project file or a dependency's source) may retype a control: the DOM's attribute API
/// (`el.setAttribute("type", …)`, an alias, a pattern key, `el.attributes.getNamedItem("type").value = …`), the accessor lookups, which hand out the `type` setter itself
/// (`HTMLInputElement.prototype.__lookupSetter__("type").call(el, "text")`), and every route to the control's `Attr` node, whose `value`, `nodeValue` or `textContent`
/// is the attribute (`document.evaluate("//input/@type", …).singleNodeValue.value = "text"`: a write no key `type` names, which Chrome retypes the control by).
/// A program that spells one is no program whose controls keep their type. A spelling is matched, never a symbol: the platform's methods are reached through
/// values the types may not name (`any`), so a veto by spelling is the fail-closed reading of the family, and it can only withdraw the proof. The dialect's own
/// exports are not in this list: they are matched as the exact imported symbols they are ([`takes_runtime_writer`]).
const ATTRIBUTE_API: [&str; 28] = [
    "setAttribute",
    "setAttributeNS",
    "removeAttribute",
    "removeAttributeNS",
    "toggleAttribute",
    "setAttributeNode",
    "setAttributeNodeNS",
    "removeAttributeNode",
    "getAttributeNode",
    "getAttributeNodeNS",
    "attributes",
    "getNamedItem",
    "getNamedItemNS",
    "setNamedItem",
    "removeNamedItem",
    "removeNamedItemNS",
    "__lookupGetter__",
    "__lookupSetter__",
    "__defineGetter__",
    "__defineSetter__",
    "createAttribute",
    "createAttributeNS",
    "evaluate",
    "createExpression",
    "XPathEvaluator",
    "singleNodeValue",
    "snapshotItem",
    "iterateNext",
];

/// Whether the permit is on: not ablated (as an arm, for the monotone gate's M2b, or as a silence).
fn enabled() -> bool {
    static ARM_ABLATED: OnceLock<bool> = OnceLock::new();
    !crate::site_accounting::ablated_id(ACTIVATION_ONLY)
        && !*ARM_ABLATED.get_or_init(|| {
            std::env::var("SOLID_CHECKER_IR_ABLATE_ARM").is_ok_and(|id| id == ACTIVATION_ONLY)
        })
}

/// Whether the attribute `attribute` (`onChange`, `on:change`, `onchange`) names the `change` event.
fn is_change(attribute: &str) -> bool {
    let name = attribute.to_ascii_lowercase();
    let name = name.strip_prefix("on").unwrap_or(&name);
    name.strip_prefix(':').unwrap_or(name) == "change"
}

/// Whether `callback` is the value of the `change` handler attribute of a literal checkbox or radio (module docs): the syntax half of the permit, no program asked.
pub(crate) fn checkable_change_handler(file: &FileFacts, callback: Span) -> bool {
    file.ast.jsx_containing(callback).any(|element| {
        element.attributes.iter().any(|attribute| {
            attribute
                .expression
                .or(attribute.value)
                .is_some_and(|value| value.contains(callback))
                && file.source_text(attribute.name).is_some_and(is_change)
                && checkable_input(file, element)
        })
    })
}

/// The program half of the permit: delivery is intact and no project file may write an element's `type`, and the permit is not ablated. Names the arm in the
/// decision it answers.
pub(crate) fn program_keeps_control_types(lookup: &SemanticLookup<'_>) -> bool {
    let intact = lookup.event_delivery_intact();
    let retype = lookup.element_retype_possible();
    census(lookup, intact, retype);
    if !enabled() || !intact || retype {
        return false;
    }
    let _ = role_arm!(
        "change_delivery.checkable-input-activation",
        ExecutionRole::EventCallback
    );
    true
}

/// `SOLID_CHECKER_CHANGE_CENSUS=<file|dir>` appends one line per project that asks the program half (it has a literal checkbox or radio with a `change` handler): how
/// many such handlers it holds and the two answers, with the delivery premise's first hazard where it fails. A measurement: it moves no verdict.
fn census(lookup: &SemanticLookup<'_>, intact: bool, retype: bool) {
    use std::io::Write as _;
    use std::sync::Mutex;
    static SEEN: Mutex<Vec<usize>> = Mutex::new(Vec::new());
    let Some(target) =
        std::env::var_os("SOLID_CHECKER_CHANGE_CENSUS").filter(|target| !target.is_empty())
    else {
        return;
    };
    let at = std::ptr::from_ref(lookup) as usize;
    {
        let mut seen = SEEN
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if seen.contains(&at) {
            return;
        }
        seen.push(at);
    }
    let candidates = lookup
        .files()
        .iter()
        .filter(|file| !crate::plain_premise::is_package(file.path.as_str()))
        .map(|file| {
            file.ast
                .jsx_elements
                .iter()
                .filter(|element| {
                    checkable_input(file, element)
                        && element.attributes.iter().any(|attribute| {
                            file.source_text(attribute.name).is_some_and(is_change)
                        })
                })
                .count()
        })
        .sum::<usize>();
    let project = lookup
        .files()
        .first()
        .map(|file| file.path.to_string())
        .unwrap_or_default();
    let hazard = crate::indexes::event_delivery::delivery_hazard_reason(lookup)
        .map(|(path, why, key)| serde_json::json!({ "path": path, "why": why, "key": key }));
    let retype_hazard =
        retype_hazard(lookup).map(|(path, why)| serde_json::json!({ "path": path, "why": why }));
    let line = serde_json::json!({
        "changeDelivery": {
            "project": project,
            "candidates": candidates,
            "deliveryIntact": intact,
            "deliveryHazard": hazard,
            "retypePossible": retype,
            "retypeHazard": retype_hazard,
        },
    });
    let mut path = std::path::PathBuf::from(target);
    if path.is_dir() {
        path = path.join(format!("change-{}.jsonl", std::process::id()));
    }
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = writeln!(file, "{line}");
    }
}

/// An intrinsic `<input>` with exactly one plain `type` attribute, the string `checkbox` or `radio`, and nothing that can retype or hand out the control: no
/// spread, no `ref`, no `is`, no `use:`, `prop:`, `attr:` or `bool:` attribute.
fn checkable_input(file: &FileFacts, element: &JsxElementFact) -> bool {
    if !crate::object_members::intrinsic_element(file, element)
        || file.source_text(element.name.span) != Some("input")
        || !element.spreads.is_empty()
    {
        return false;
    }
    let mut checkable = false;
    for attribute in &element.attributes {
        let local = file.source_text(attribute.local_name).unwrap_or_default();
        match attribute
            .namespace
            .map(|namespace| file.source_text(namespace).unwrap_or_default())
        {
            None => match local {
                "type" => {
                    let literal = attribute.value_kind == JsxAttributeValueKind::String
                        && attribute
                            .value
                            .and_then(|value| file.source_text(value))
                            .map(|text| text.trim_matches(['"', '\'']))
                            .is_some_and(|text| {
                                CHECKABLE_TYPES
                                    .iter()
                                    .any(|checkable| text.eq_ignore_ascii_case(checkable))
                            });
                    if checkable || !literal {
                        return false;
                    }
                    checkable = true;
                }
                "ref" | "is" => return false,
                _ => {}
            },
            // `on:change` is the listener itself; `class:` and `style:` set no property a control's type rests on.
            Some("on" | "oncapture" | "class" | "style") => {}
            Some(_) => return false,
        }
    }
    checkable
}

/// Whether any file the program runs may write the `type` of an element ([`retype_hazard`]).
pub(crate) fn program_may_retype(lookup: &SemanticLookup<'_>) -> bool {
    retype_hazard(lookup).is_some()
}

/// The first place that may write the `type` of an element, as its file and what it does: a write whose key is `type`, or is a computed one no proof narrows away from it
/// ([`write_may_be_type`]) (`type-write`); a spelling of a name of the DOM's attribute API, of an accessor lookup or of a route to an `Attr` node ([`ATTRIBUTE_API`])
/// (`spelled-name`); a computed key that folds to such a name (`el["set" + "Attribute"]("type", "text")`: the name is spelled nowhere, the key's proof names it)
/// (`folded-name`); or a module of the dialect's own runtime taken into the file that may write an element ([`takes_runtime_writer`]) (`runtime-writer`).
///
/// Every file of the program is read, a dependency's source as the project's: a dependency needs no element handed to it (`document.querySelectorAll("input").forEach((i)
/// => { i.type = "text"; })`), and a library setter (`$(el).prop("type", "text")`) is a computed write in the library's own source. A dependency whose source the program
/// does not hold is the delivery premise's unread module (ADR 1125), which withholds the proof beside this answer. The files of the dialect's own runtime packages
/// ([`solid_dialect::Dialect::primitive_defining_packages`]) are not read (ADR 0027: the dialect states what its runtime does), because what they do is asked at the
/// import instead, per module specifier ([`solid_dialect::Dialect::runtime_module_element_writes`]): the root's writes of a property are the compiler's, on elements whose
/// `type` is dynamic or spread, and that is no checkable input here ([`checkable_input`]); its exports that write by a name, and the frames tier that morphs the children it
/// finds with the server's attributes, are what the dialect names. The ecosystem packages (`@solidjs/router`, …) are consumers of the runtime, not part of it, and are read
/// like any other. The reflective routes (`Object.assign` onto an element it was handed, `defineProperty`, `Reflect.set`, `with`, a key reader, code from a string) are the
/// delivery premise's, asked beside this answer.
pub(crate) fn retype_hazard(lookup: &SemanticLookup<'_>) -> Option<(String, &'static str)> {
    let runtime = lookup.dialect.primitive_defining_packages();
    lookup
        .files()
        .iter()
        .filter(|file| !is_dialect_runtime(file.path.as_str(), runtime))
        .find_map(|file| file_retype_hazard(lookup, file).map(|why| (file.path.to_string(), why)))
}

/// Whether `path` is a file of one of the dialect's own runtime packages (`…/node_modules/@solidjs/web/…`).
fn is_dialect_runtime(path: &str, runtime: &[&str]) -> bool {
    crate::plain_premise::package_name(path).is_some_and(|name| runtime.contains(&name))
}

/// What in `file` may write the `type` of an element, if anything ([`retype_hazard`]).
fn file_retype_hazard(lookup: &SemanticLookup<'_>, file: &FileFacts) -> Option<&'static str> {
    let ast = &file.ast;
    if ast
        .write_targets
        .iter()
        .chain(&ast.deleted_targets)
        .any(|target| write_may_be_type(file, *target))
    {
        return Some("type-write");
    }
    if ATTRIBUTE_API
        .iter()
        .any(|name| spells_word(&file.source, name))
    {
        return Some("spelled-name");
    }
    if ast.computed_keys.iter().any(|key| {
        key.texts
            .iter()
            .any(|text| ATTRIBUTE_API.contains(&text.as_str()))
    }) {
        return Some("folded-name");
    }
    takes_runtime_writer(lookup, file).then_some("runtime-writer")
}

/// Whether `file` takes a module of the dialect's own runtime into the program that may write an element ([`solid_dialect::Dialect::runtime_module_element_writes`], asked
/// of the specifier as written, the root and every subpath). A module is the runtime's when its specifier is a runtime package or a path under one: a specifier the dialect
/// has not audited is unknown and taken to write, so a subpath a later release adds withdraws the proof until it is audited. A named import names its export whatever the
/// local alias is, and takes a writer only where the dialect lists that export (or all of them, for the frames tier that rewrites the children it finds); a default or
/// namespace import exposes every export; `export … from` and `export * from` hand them on to every importer; an `import()` or a `require()` reaches the exports its consumer
/// names, and all when it escapes unnamed. A specifier that is no runtime package's by name but resolves into a file of one (a relative path into `node_modules`, a `paths`
/// alias) reaches that file by a name no audit lists, so it is unknown too: the build's attested resolution says where it lands. An import that erases (`import type`) takes
/// nothing. A function of the same name that the program declares is no import: the proof is not refused by a name.
fn takes_runtime_writer(lookup: &SemanticLookup<'_>, file: &FileFacts) -> bool {
    use solid_dialect::ElementWrites;
    let dialect = lookup.dialect;
    let packages = dialect.primitive_defining_packages();
    // What the dialect says of `module`: `None` for a module that is no runtime package's (the program's own or a dependency's, read as source).
    let answer = |module: &str| {
        packages
            .iter()
            .any(|package| {
                module == *package
                    || module
                        .strip_prefix(package)
                        .is_some_and(|rest| rest.starts_with('/'))
            })
            .then(|| dialect.runtime_module_element_writes(module))
    };
    // Whether a binding `export` of `module` (any of its exports, when no fact names which) may write an element.
    let may = |module: &str, export: Option<&str>| match answer(module) {
        None | Some(Some(ElementWrites::None)) => false,
        Some(Some(ElementWrites::Exports(names))) => {
            export.is_none_or(|export| names.contains(&export))
        }
        Some(Some(ElementWrites::Any) | None) => true,
    };
    // Whether merely loading `module` may: the frames tier, and a module no release was audited for, run code of their own when they load.
    let loads_rewriting =
        |module: &str| matches!(answer(module), Some(Some(ElementWrites::Any) | None));
    let holds = |module: &str| may(module, None);
    let exact = |module: &str, export: Option<&str>| may(module, export);
    // A specifier that is no runtime package's by name but resolves into a file of one (a relative path into `node_modules`, a `paths` alias) reaches that file by a
    // name no audit lists: unknown, and taken as writing. The resolution is the build's attested one; an unattested specifier is the delivery premise's unread module.
    let imports = lookup.resolved_imports();
    let reaches_runtime_file = |at: Span, module: &str| {
        answer(module).is_none()
            && imports.is_some_and(|imports| {
                match imports.specifier(file.path.as_str(), at, module) {
                    SpecifierAttestation::Attested(row) => {
                        is_dialect_runtime(row.resolved_path.as_ref(), packages)
                    }
                    SpecifierAttestation::Unattested => false,
                }
            })
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
            ImportKind::Named => exact(module, binding.imported.as_deref()),
            ImportKind::Default | ImportKind::Namespace => holds(module),
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
                        exact(module, name.as_deref())
                    })
        });
    let loaded = ast
        .module_loads
        .iter()
        .any(|load| match load.specifier.as_deref() {
            // A computed specifier may name any module: a delivery hazard already, and no file may be taken to avoid a writer.
            None => true,
            Some(module) if !holds(module) => false,
            Some(module) => match &load.consumer {
                Some(ModuleLoadConsumer::Members { names }) => {
                    names.iter().any(|name| exact(module, Some(name)))
                }
                Some(ModuleLoadConsumer::Lazy { export, .. }) => exact(module, Some(export)),
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

/// Whether the member written at `target` (an assignment, update, destructuring or iteration target, or a deleted operand) may be the property `type`: its name
/// is `type`; its key is a literal `"type"`; or its key is no literal and nothing proves it is another string.
fn write_may_be_type(file: &FileFacts, target: Span) -> bool {
    let ast = &file.ast;
    let target = ast.peel_ts_sugar_span(target);
    let at = ast.computed_keys.partition_point(|key| key.read < target);
    if let Some(key) = ast.computed_keys[at..]
        .iter()
        .take_while(|key| key.read == target)
        .find(|key| key.site == ComputedKeySite::Member)
    {
        return !key.proven || key.texts.iter().any(|text| text.as_str() == "type");
    }
    if let Some(literal) = ast
        .literal_computed_members
        .iter()
        .find(|member| member.span == target)
    {
        return literal.key.as_str() == "type";
    }
    ast.member_at(target)
        .and_then(|member| file.source_text(member.property))
        == Some("type")
}

/// Whether `source` holds `word` as an identifier: not part of a longer one.
fn spells_word(source: &str, word: &str) -> bool {
    let identifier = |c: char| c.is_alphanumeric() || c == '_' || c == '$';
    source.match_indices(word).any(|(at, _)| {
        !source[..at].chars().next_back().is_some_and(identifier)
            && !source[at + word.len()..]
                .chars()
                .next()
                .is_some_and(identifier)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_arm_id_the_proof_log_names_is_the_one_ablation_names() {
        // `role_arm!` takes a literal, and the ablation a constant: they are one string.
        assert_eq!(
            ACTIVATION_ONLY,
            "change_delivery.checkable-input-activation"
        );
    }

    #[test]
    fn change_is_named_by_its_attribute_forms_only() {
        for name in ["onChange", "onchange", "on:change", "onCHANGE"] {
            assert!(is_change(name), "{name}");
        }
        for name in [
            "onInput",
            "oncapture:change",
            "change",
            "onChanged",
            "onclick",
            "",
        ] {
            // `change` bare is no attribute of a handler: the compiler reads an event as `on` and a name.
            assert_eq!(is_change(name), name == "change", "{name}");
        }
    }

    /// The file the source makes, as the analysis holds one (no compiler facts: the syntax half reads none).
    fn file(source: &str) -> FileFacts {
        use solid_facts::compiler::{COMPILER_FACTS_PROTOCOL, ExecutionMap};
        use solid_facts::core::{Generation, SourceHash, SourcePath};
        let generation = Generation::new(1).unwrap();
        FileFacts {
            generation,
            path: SourcePath::new("app.tsx").unwrap(),
            source_hash: SourceHash::of(source),
            source: std::sync::Arc::from(source),
            ast: std::sync::Arc::new(solid_facts::ast::extract("app.tsx", source).unwrap()),
            compiler: std::sync::Arc::new(ExecutionMap {
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

    /// Whether the arrow `() => {}` written in `source` is the `change` handler of a literal checkbox or radio.
    fn takes_the_proof(source: &str) -> bool {
        let at = source.find("() => {}").expect("the handler is written");
        let handler = Span::new(
            u32::try_from(at).unwrap(),
            u32::try_from(at + "() => {}".len()).unwrap(),
        );
        checkable_change_handler(&file(source), handler)
    }

    #[test]
    fn only_a_literal_checkbox_or_radio_with_nothing_to_retype_it_takes_the_proof() {
        for element in [
            "<input type=\"checkbox\" onChange={() => {}} />",
            "<input onChange={() => {}} type=\"radio\" />",
            "<input type=\"CheckBox\" onChange={() => {}} />",
            "<input type='radio' name=\"g\" checked={x()} onChange={() => {}} class:on={y()} style:color=\"red\" />",
            "<input type=\"checkbox\" onchange={() => {}} />",
        ] {
            assert!(
                takes_the_proof(&format!("const e = {element};")),
                "{element}"
            );
        }
        for element in [
            // Another control, or none a `change` reaches by activation alone.
            "<input onChange={() => {}} />",
            "<input type=\"text\" onChange={() => {}} />",
            "<input type=\"search\" onChange={() => {}} />",
            "<select onChange={() => {}} />",
            "<button type=\"checkbox\" onChange={() => {}} />",
            "<Input type=\"checkbox\" onChange={() => {}} />",
            "<ui.input type=\"checkbox\" onChange={() => {}} />",
            // A type that is no literal, or may change: a signal, a second `type`, a spread, a `ref`, `is`, a namespaced writer.
            "<input type={kind()} onChange={() => {}} />",
            "<input type={'checkbox'} onChange={() => {}} />",
            "<input type=\"checkbox\" type=\"text\" onChange={() => {}} />",
            "<input type=\"checkbox\" {...p} onChange={() => {}} />",
            "<input {...p} type=\"checkbox\" onChange={() => {}} />",
            "<input type=\"checkbox\" onChange={() => {}} {...p} />",
            "<input type=\"checkbox\" ref={r} onChange={() => {}} />",
            "<input type=\"checkbox\" is=\"x-box\" onChange={() => {}} />",
            "<input type=\"checkbox\" use:d onChange={() => {}} />",
            "<input type=\"checkbox\" prop:type=\"text\" onChange={() => {}} />",
            "<input type=\"checkbox\" attr:type=\"text\" onChange={() => {}} />",
            "<input type=\"checkbox\" bool:disabled={true} onChange={() => {}} />",
            "<input type=\"checkbox\" bool:type={true} onChange={() => {}} />",
            // Another event.
            "<input type=\"checkbox\" onInput={() => {}} />",
            "<input type=\"checkbox\" onClick={() => {}} />",
        ] {
            assert!(
                !takes_the_proof(&format!("const e = {element};")),
                "{element}"
            );
        }
        // The handler is the attribute's value: an arrow elsewhere in the element is none.
        assert!(!takes_the_proof(
            "const e = <input type=\"checkbox\" title={(() => {})()} onChange={f} />;"
        ));
    }

    #[test]
    fn words_are_matched_whole() {
        assert!(spells_word("el.setAttribute('a', 1)", "setAttribute"));
        assert!(spells_word("const { attributes } = el;", "attributes"));
        assert!(!spells_word("mySetAttribute()", "setAttribute"));
        assert!(!spells_word("el.attributesList", "attributes"));
        assert!(!spells_word("$attributes", "attributes"));
    }
}
