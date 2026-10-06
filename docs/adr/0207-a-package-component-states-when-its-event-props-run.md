# ADR 0207: A package component states when its event props run

- Status: accepted and implemented (2026-10-06). Track B (authored contracts, ADR 0198) meets Track A step A3
  (ADR 0199).
- Owners:
  - the contract format: `ValueSource::ParameterMembers`
    (`solid-reactive-ir/src/contract_semantics.rs`), its wire form and
    validation (`solid-facts-backend/src/contract_document.rs`,
    `schema/solid-reactivity.schema.json`), and its canonical encoding;
  - its projection (`contracts.rs`, `ContractExport::event_handler_props`);
  - its consumer, `package_prop_runs_only_deferred`
    (`solid-reactive-ir/src/execution_role.rs`);
  - the authored tier's identity source (`scripts/author-contracts.mjs`).

## Context

ADR 0199 proves a callback prop runs only on DOM event dispatch when the
project component that receives it only ever hands it to an intrinsic
element's event handler. The proof stops at a package component. On the rc.13
corpus, about 35 of the 693 strict reads in "Needs review" end there:
`openbot`'s `Button`, `IconButton`, `DropdownMenu.Item`, `Switch` and others
spread their props onto Kobalte (`@kobalte/core` 2.0.0-alpha.0) parts.

What a contract must state to close that gap is not expressible today, and
the obvious approximation would be false:

- A component's callbacks domain, closed, must list every caller callable it
  invokes. Kobalte's `Button.Root` forwards every prop it does not consume to
  `Dynamic`. That runtime attaches every un-namespaced `on…` property as an
  event listener, and runs `ref` and a callable `children` while rendering.
  The set of `on…` names is unbounded. A closed domain listing `onClick`
  would claim `onKeyDown` is never invoked.
- What runs those props is conditional. A caller may pass `as`, and when
  `as` is a component every prop goes to it, and it may invoke any of them
  while rendering.

Kobalte also has no certified bundle in the tier, so ADR 0198's build has no
identity to start a document from.

## Decision

1. **A callback source can name a class of members.**
   - `{ "arg": N, "path": [...], "members": "event-handler-props" }` names
     every own property of that value whose key starts with `on` and contains
     no `:`. That is exactly the set the Solid runtime's spread attaches as
     event listeners (`@solidjs/web` `assignProp`, ADR 0199 § 4).
   - The item is **exhaustive for its class**. Every invocation of such a
     member is the item's operation, whatever the domain's closure says.
   - The domain's own closure keeps its meaning for everything else. A
     document that states the item and leaves `callbacks` open says nothing
     about `ref`, `children` or `as`.
   - `members` requires `arg` and an `invoke` operation of the call protocol.
     A decoder that predates the field refuses the document, which is
     intended.
2. **Generic consumers do not see the item.** Projection treats it like any
   source it does not model: the generic callbacks claim opens. The item is
   projected on its own (`event_handler_props`), with its execution word and
   the operation's guard kept whole, never dropped.
3. **The consumer** is ADR 0199's proof, at the one place it stops: a JSX
   element whose tag resolves to a package export rather than a project
   function. A prop `on…` reaches only event dispatch there when all of
   these hold:
   - the export's accepted contract states an `event-handler-props` item on
     argument 0 with an empty path, and its execution is `deferred` (an
     external event or a queue, never the render's stack);
   - every atom of the operation's guard is proven at the element. The one
     atom read today is `{ arg: 0, path: [p], kind: "plain" }`, proven when
     the props object can hold no `p`: the element writes no `p` attribute
     and no spread. Where the element is reached through a project
     component's spread (ADR 0199 § 4), the original element that wrote the
     literal must also write no `p` attribute and no spread, and no hop may
     write `p`. Every other atom proves nothing.
4. **The authored tier takes identity from a generated proposal when no
   certified bundle exists.**
   - A spec may name an `identity` file: the artifact cases of a
     `solid-checker contract generate --host browser` proposal for one
     entrypoint, with each case's condition set and runtime and declaration
     targets taken from its certification inputs. The build computes the
     snapshot root from the probe install, as
     `installed_package_snapshot_root` does.
   - The proposal's claims are discarded. Only its identity fields enter the
     document, exactly as ADR 0198 § 6 keeps only a certified case's
     identity.
5. **A timing claim's probe clicks.** The pair for an event claim runs a
   scenario: render, click, then wait for text the handler writes.
   - The correct twin passes when no `STRICT_READ_UNTRACKED` is raised while
     it renders, although the handler reads a signal, and the handler then
     runs on the click.
   - The misuse twin reads the same signal in the body. That shows the
     strict-read window is live on the page.

## Consequences

- An `on…` prop of a package component with such a contract is classified
  like one written on an intrinsic element. A component passed as `as`, a
  spread the proof cannot follow, or any other guard atom keeps the read
  uncertifiable.
- The format addition is additive. Every existing document decodes and
  hashes as before.

## Evidence

- **Fixture** `fixtures/reactive-ir/package-event-props-consumer`. A fixture
  package's `Root` is authorized out of band with one guarded
  `event-handler-props` item:
  - `Direct`, `AnotherEvent` and `Wrapped` (through an `omit` view) are clean;
  - `AsComponent`, `SpreadAtOrigin`, `MergeWrapped` (a `merge` can add
    `as`) and `NotAnEvent` stay `SC1001` uncertifiable.

  Before this ADR, all seven were uncertifiable.
- **Coverage:** 177 fixture projects, 934 findings. Only the new snapshot is
  new.
- **Authored tier:**
  - `@kobalte/core@2.0.0-alpha.0` `./button` `Root` and `Button` take their
    identity from a `contract generate --host browser` proposal, through
    `author-contracts.mjs identity`. The build's snapshot root reproduces an
    existing entry's byte for byte (`@solidjs/router@2.0.0-next.16`,
    `sha256:932380…`).
  - The pairs pass in Chrome on probus-hk's install, rc.13 and unpatched:
    - the misuse twin raises `STRICT_READ_UNTRACKED` at its body read;
    - the correct twin raises nothing while it renders, although its
      `onClick` reads a signal, and the click scenario sees the handler run.
- **The authored tier's loading test** (`the_shipped_index_loads`) caught the
  first draft's single-case entrypoint form, which no document uses. The
  build now writes `{ "cases": [...] }`.

## Consequences found on the corpus

- **Patched installs are refused.** `openbot` patches
  `@kobalte/core@2.0.0-alpha.0` (`bun.lock` `patchedDependencies`). The patch
  rewrites the button chunk this claim is about. Admission refuses a patched
  package by design (ADR 0198 § 3), so openbot's `Button` sites stay
  uncertifiable. They are about 31 of the 35 this ADR was aimed at.
  - The first probe ran on openbot's patched files. It was redone on
    probus-hk's published bytes before the tier was built.
  - Admitting an authored claim for one project's patch would need the patch
    digest in the identity. That is a separate policy decision.
- **rc.13 corpus**, browser host, release binary, against
  `rc13-i-browser.json`: violations unchanged at 285, uncertifiable unchanged
  at 3,421.
  - The one moved finding is `readingroom`'s `ui/Button.tsx`, whose
    unpatched `@kobalte/core@2.0.0-alpha.0` import now binds the authored
    contract. "No entrypoint/export summary for Button" became "leaves
    reactiveReads, returns, ownerRequirements unknown", which is right for a
    claim that states only event props.
  - No strict read in `readingroom` or `probus-hk`, the two unpatched Kobalte
    installs, depends on it.
