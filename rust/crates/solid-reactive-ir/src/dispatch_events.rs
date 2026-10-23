//! The event types ADR 0329's synchronously dispatching `lib.dom` members fire
//! on their caller's stack (ADR 0361): the table the per-type event delivery
//! reads. Every set is a superset of what a conforming engine fires
//! synchronously; a type fired later, as its own task (`toggle`, `close`,
//! `selectionchange`, `scroll`), is not on the dispatcher's stack and is left
//! out (probe: `fixtures/runtime-oracle/oracle-dispatch-type-sets`).

/// What one dispatching member fires before it returns.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Fires {
    /// Exactly these event types (lowercase, as a listener names them), at any
    /// target. Empty: the events go to objects that are no element (the
    /// signal, the request, the reader), which no compiler handler hears.
    Types(&'static [&'static str]),
    /// Any type: the argument or the command names it.
    AnyType,
}

/// The focus update steps (HTML Standard, interaction, "focus update steps"
/// and "unfocusing steps"): the previously focused element, which may be any
/// element, is blurred, a text control the user edited fires `change` as it
/// loses focus (4.10.5.5 common input element events), and an active
/// composition ends.
const FOCUS_MOVE: &[&str] = &[
    "focus",
    "focusin",
    "blur",
    "focusout",
    "change",
    "compositionend",
];

/// `blur()`: the unfocusing steps alone.
const UNFOCUS: &[&str] = &["blur", "focusout", "change", "compositionend"];

/// `click()` runs the element's activation behavior: a checkbox or radio fires
/// `input` and `change`; a submit button runs form submission (`submit`,
/// `formdata`, and `invalid` with the focus move of interactive validation); a
/// reset button `reset`; a `popovertarget`/`commandfor` button the `command`
/// event, `beforetoggle` and, for a dialog command, `cancel` and its focus
/// move; a label focuses its control; an anchor with a same-document fragment
/// navigates synchronously and its "scroll to the fragment" reveals a
/// `hidden=until-found` ancestor (`beforematch`, HTML Standard, "scroll to the
/// fragment" and "ancestor revealing algorithm"). `toggle` and `close` are
/// queued tasks.
const CLICK: &[&str] = &[
    "click",
    "input",
    "change",
    "submit",
    "formdata",
    "invalid",
    "reset",
    "beforetoggle",
    "command",
    "cancel",
    "beforematch",
    "focus",
    "focusin",
    "blur",
    "focusout",
    "compositionend",
];

const INVALID: &[&str] = &["invalid"];

/// Constraint validation that reports (HTML Standard, form control
/// infrastructure, "report the problems with the constraints"): `invalid`, then
/// the first invalid control is focused.
const REPORT_VALIDITY: &[&str] = &[
    "invalid",
    "focus",
    "focusin",
    "blur",
    "focusout",
    "change",
    "compositionend",
];

const RESET: &[&str] = &["reset"];

/// `requestSubmit()`: interactive validation (`invalid`, the focus move), then
/// the submit event and, when it is not canceled, the entry list's `formdata`;
/// a `method=dialog` form closes its dialog, which fires `beforetoggle` before
/// it queues `toggle` and `close` (HTML Standard, form submission algorithm,
/// dialog method; "close the dialog").
const REQUEST_SUBMIT: &[&str] = &[
    "submit",
    "formdata",
    "invalid",
    "beforetoggle",
    "focus",
    "focusin",
    "blur",
    "focusout",
    "change",
    "compositionend",
];

/// Popovers and dialogs: the `beforetoggle` event (also on the popovers or
/// dialog the call closes) and the focus move of the popover or dialog focusing
/// steps and of the previously focused element's restoration.
const TOGGLE: &[&str] = &[
    "beforetoggle",
    "focus",
    "focusin",
    "blur",
    "focusout",
    "change",
    "compositionend",
];

/// `requestClose()` fires `cancel` before it closes.
const REQUEST_CLOSE: &[&str] = &[
    "cancel",
    "beforetoggle",
    "focus",
    "focusin",
    "blur",
    "focusout",
    "change",
    "compositionend",
];

/// What a synthetic `click` event (a `MouseEvent` or subclass of type `click`
/// passed to `dispatchEvent`) fires: the DOM dispatch algorithm runs the
/// target's activation behavior for an activation event, "a `MouseEvent` whose
/// type is `click`" (DOM Standard, `#concept-event-dispatch`, `isActivationEvent`),
/// which is what `click()` runs.
pub(crate) fn synthetic_click_fires() -> Fires {
    Fires::Types(CLICK)
}

/// Members that dispatch on every `lib.dom` interface declaring them.
const ANY_INTERFACE: &[(&str, Fires)] = &[
    ("focus", Fires::Types(FOCUS_MOVE)),
    ("blur", Fires::Types(UNFOCUS)),
    ("click", Fires::Types(CLICK)),
    // Overridden by the argument's literal type where one is read.
    ("dispatchEvent", Fires::AnyType),
    ("checkValidity", Fires::Types(INVALID)),
    ("reportValidity", Fires::Types(REPORT_VALIDITY)),
];

/// Members whose name other `lib.dom` members share, by exact owner.
const OWNED: &[(&str, &str, Fires)] = &[
    ("HTMLFormElement", "reset", Fires::Types(RESET)),
    (
        "HTMLFormElement",
        "requestSubmit",
        Fires::Types(REQUEST_SUBMIT),
    ),
    ("HTMLElement", "showPopover", Fires::Types(TOGGLE)),
    ("HTMLElement", "hidePopover", Fires::Types(TOGGLE)),
    ("HTMLElement", "togglePopover", Fires::Types(TOGGLE)),
    ("HTMLDialogElement", "show", Fires::Types(TOGGLE)),
    ("HTMLDialogElement", "showModal", Fires::Types(TOGGLE)),
    ("HTMLDialogElement", "close", Fires::Types(TOGGLE)),
    (
        "HTMLDialogElement",
        "requestClose",
        Fires::Types(REQUEST_CLOSE),
    ),
    // The signal, the request and the reader are no elements (premise: the
    // compiler's handlers are element attributes only).
    ("AbortController", "abort", Fires::Types(&[])),
    ("XMLHttpRequest", "open", Fires::Types(&[])),
    ("XMLHttpRequest", "abort", Fires::Types(&[])),
    ("XMLHttpRequest", "send", Fires::Types(&[])),
    ("FileReader", "abort", Fires::Types(&[])),
    // The command string picks among editing, clipboard and selection events.
    ("Document", "execCommand", Fires::AnyType),
];

/// What the exact `lib.dom` declaration `owner.member` fires, or `None` when it
/// is no dispatching member (ADR 0329).
pub(crate) fn member_fires(owner: &str, member: &str) -> Option<Fires> {
    ANY_INTERFACE
        .iter()
        .find(|(name, _)| *name == member)
        .map(|(_, fires)| *fires)
        .or_else(|| {
            OWNED
                .iter()
                .find(|(declaring, name, _)| *declaring == owner && *name == member)
                .map(|(_, _, fires)| *fires)
        })
}

/// What a member written `name` may fire when its owner is not read: the set of
/// each declaration that shares the spelling (empty for any other name).
pub(crate) fn spelled_member_fires(name: &str) -> impl Iterator<Item = Fires> + '_ {
    ANY_INTERFACE
        .iter()
        .filter(move |(member, _)| *member == name)
        .map(|(_, fires)| *fires)
        .chain(
            OWNED
                .iter()
                .filter(move |(_, member, _)| *member == name)
                .map(|(_, _, fires)| *fires),
        )
}

/// Whether `name` is a member that dispatches on every `lib.dom` interface
/// declaring it.
pub(crate) fn dispatches_on_any_interface(name: &str) -> bool {
    ANY_INTERFACE.iter().any(|(member, _)| *member == name)
}

/// Whether `name` is the spelling of a member that dispatches only on its exact
/// owner (the `OWNED` rows): a value reference to it may be any owner's.
pub(crate) fn dispatches_on_exact_owner(name: &str) -> bool {
    OWNED.iter().any(|(_, member, _)| *member == name)
}

/// Whether `name` is the spelling of any dispatching member.
pub(crate) fn dispatching_spelling(name: &str) -> bool {
    dispatches_on_any_interface(name) || OWNED.iter().any(|(_, member, _)| *member == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spelled(name: &str) -> Vec<Fires> {
        spelled_member_fires(name).collect()
    }

    #[test]
    fn every_listed_member_is_known_by_owner_and_by_spelling() {
        for (name, fires) in ANY_INTERFACE {
            assert_eq!(member_fires("Anything", name), Some(*fires), "{name}");
            assert_eq!(spelled(name), vec![*fires], "{name}");
            assert!(dispatches_on_any_interface(name), "{name}");
        }
        for (owner, member, fires) in OWNED {
            assert_eq!(
                member_fires(owner, member),
                Some(*fires),
                "{owner}.{member}"
            );
            assert_eq!(member_fires("Other", member), None, "Other.{member}");
            assert!(spelled(member).contains(fires), "{member}");
            assert!(dispatching_spelling(member), "{member}");
            assert!(!dispatches_on_any_interface(member), "{member}");
        }
        assert!(spelled("addEventListener").is_empty());
        assert!(!dispatching_spelling("addEventListener"));
        assert_eq!(member_fires("EventTarget", "addEventListener"), None);
    }

    #[test]
    fn a_spelling_shared_across_owners_keeps_each_declarations_set() {
        // `abort` is three declarations, each firing at non-elements.
        assert_eq!(spelled("abort"), vec![Fires::Types(&[]); 3]);
        assert_eq!(spelled("close"), vec![Fires::Types(TOGGLE)]);
        assert_eq!(spelled("execCommand"), vec![Fires::AnyType]);
    }

    #[test]
    fn the_sets_are_lowercase_and_hold_no_task_events() {
        let all = ANY_INTERFACE
            .iter()
            .map(|(_, fires)| *fires)
            .chain(OWNED.iter().map(|(_, _, fires)| *fires));
        for fires in all {
            if let Fires::Types(set) = fires {
                for event in set {
                    assert_eq!(*event, event.to_ascii_lowercase());
                    assert!(
                        !["toggle", "close", "selectionchange", "scroll"].contains(event),
                        "{event} is queued as a task"
                    );
                }
            }
        }
    }

    #[test]
    fn a_synthetic_click_runs_the_activation_behavior_click_runs() {
        assert_eq!(synthetic_click_fires(), Fires::Types(CLICK));
        assert_eq!(
            member_fires("Anything", "click"),
            Some(synthetic_click_fires())
        );
    }

    #[test]
    fn the_members_that_close_a_dialog_or_reveal_a_fragment_fire_what_they_run() {
        // `method=dialog` submission closes the dialog (`beforetoggle`); a
        // same-document anchor click reveals a `hidden=until-found` ancestor.
        assert!(REQUEST_SUBMIT.contains(&"beforetoggle"));
        assert!(CLICK.contains(&"beforematch"));
        assert!(CLICK.contains(&"beforetoggle"));
    }

    #[test]
    fn an_owner_specific_spelling_is_not_an_any_interface_one() {
        for name in ["showModal", "reset", "requestSubmit", "close", "abort"] {
            assert!(dispatches_on_exact_owner(name), "{name}");
            assert!(!dispatches_on_any_interface(name), "{name}");
        }
        for name in ["click", "focus", "addEventListener", "value"] {
            assert!(!dispatches_on_exact_owner(name), "{name}");
        }
    }

    #[test]
    fn the_focus_move_is_in_every_family_that_moves_focus() {
        for family in [
            CLICK,
            REPORT_VALIDITY,
            REQUEST_SUBMIT,
            TOGGLE,
            REQUEST_CLOSE,
        ] {
            for event in ["focus", "focusin", "blur", "focusout", "change"] {
                assert!(family.contains(&event), "{event}");
            }
        }
        for event in UNFOCUS {
            assert!(FOCUS_MOVE.contains(event));
        }
    }
}
