# ADR 0318: Script-free DOM members are trusted in the optional form

- Status: accepted and implemented (2026-10-17).
- Owners: `SCRIPT_FREE_DOM_MEMBERS` and `script_free_dom_member`, consulted by
  `member_call_operations` (`solid-reactive-ir/src/cleanup.rs`).
- Relation: narrows the optional-DOM guard of the leaf-scope walk (ADR 0210);
  supersedes the blanket stance `leaf-scope-host-callbacks`' `Focus.tsx` pinned
  for optional calls, with the owner's approval.

## Context

The leaf-scope walk reads a standard-library call as running only the
functions it is handed, in the audited timing table's order (ADR 0210), and
says plainly that dispatch is not modeled. The non-optional `el.focus()` or
`el.querySelector("a")` on a typed DOM node is certified by that trust. The
optional form `el?.m()` was refused for every `lib.dom` member, because
optional-call signature recovery supplies declaration identity, not host
behavior, and focus/blur and custom-element reactions run registered author
code with no callback at the site. That refusal also held members that run no
author code at all.

## Decision

An optional call whose exact declaration (`lib.dom.d.ts` or
`lib.dom.iterable.d.ts`) is in an audited table is read exactly like the
non-optional form: its arguments by the standard-library timing table, the
callee's type still the exact declaration. The table holds members that, per
the DOM and HTML specifications, dispatch no event, run no custom-element
reaction and change no node's connectedness while they execute:

- `addEventListener` / `removeEventListener` on every event-target interface
  (registration; the listener stays a deferred argument);
- `ParentNode.querySelector`, `querySelectorAll`;
  `Document`/`Element.getElementsByTagName`, `getElementsByClassName`;
- `Element`/`Range.getBoundingClientRect`, `getClientRects` (layout reads; no
  event fires synchronously);
- `Node.contains`, `DOMTokenList.contains`, `DOMStringList.contains`;
- `Element.closest`, `matches`, `getAttribute`, `getAttributeNames`,
  `hasAttribute`, `hasAttributes`;
- `HTMLCanvasElement.toDataURL`.

Excluded, among everything else: `focus`, `blur`, `click`, `dispatchEvent`,
`select`, form `submit`/`requestSubmit`/`reset`, dialog `show`/`showModal`/
`close`, scrolling, tree mutation (`remove`, `append`, `prepend`,
`appendChild`, `insertBefore`, `replaceChildren`, `replaceWith`), attribute
writes (`setAttribute`, `removeAttribute`, `toggleAttribute`,
`setAttributeNode`: `attributeChangedCallback`), `animate`, and
`HTMLCanvasElement.getContext` (WebGL context creation can fire
`webglcontextcreationerror` synchronously).

Not modeled, as for the rest of the standard-library trust: getters,
`toString`/`valueOf` or Proxy traps on objects handed in (an options
dictionary, a selector object), and thenables.

## Consequences

49-project corpus: 24 SC9012 uncertifiable findings removed (optional
`addEventListener` 10, `querySelector` 11, `closest`, `matches`, and one leaf
root reaching such a call), none added, violations unchanged. Fixture:
`leaf-scope-optional-dom`; `Focus.tsx` keeps its results and its README now
states the per-member stance.

The remaining inconsistency: the non-optional form of a re-entrant member
(`el.focus()`, `el.click()`, `el.remove()`, `el.setAttribute(…)`) is still
certified by the standard-library trust, which does not model dispatch or
custom-element reactions. Measured, not implemented: refusing the non-optional
form of the excluded members would add 44 SC9012 uncertifiable findings on the
corpus. Recommendation: decide it together with the DOM-reference receivers
(the audited host-method table); a stricter non-optional stance is the
faithful one, at that cost.
