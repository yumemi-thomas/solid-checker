# dead-site-nodelist

**Claim (ADR 1033).** A callback handed to `NodeList.forEach` of the `childNodes` of, or of a `querySelectorAll` on, an element `document.createElement("i")` makes there (the library's `document`, one quoted name with no `-`, no options) never runs: the element has no child. A write (SC2001), an action call (SC2002) or `onFlush` (SC3002) in it is uncertifiable (`function-unentered`), never a violation.

Uncertifiable, each a violation on main: `FreshChildNodes`, `FreshQueryAction`, `FreshQueryFlush`.

Violations kept: `DocumentQuery` (`document.querySelectorAll`: the document may hold any element; a known false positive, ledgered, ADR 1033), `BodyChildNodes`, `CustomElementChildNodes` (`x-card` may be a custom element whose constructor appends children), `NamedNode` (a held node given a child), `ShadowedDocument` (a local `document`), and the fragment cases (`document.createDocumentFragment()` has no resolved declaration here: a known false positive, docs/open-work.md).

Chrome: `fixtures/runtime-oracle/dead-site-nodelist` (quiet) and `-live` (2 of 2 covered). Stubs: `dead-site-empty-collection`'s. `tsc --noEmit` clean.
