# A private helper that escapes as a value breaks the caller enumeration

The call graph's answer is *fail closed or exact*: either every way of entering
a function is one of the call sites it enumerated, or the enumeration is
incomplete and emission must mark every export. A function referenced anywhere
other than its declaration, one of those call sites, or the module surface that
forwards it has escaped as a value — something the graph did not model can
invoke it.

The escape test used to accept any reference inside an `ExportFact.span`. For an
`ExportNamedDeclaration` that span covers the **whole declaration, body
included**, so every reference written inside an exported function read as
"export surface". `apply(Panel, ...)`, `return Panel`, and `<PanelView/>` are
all such references: each one is a real escape, each one was accounted for, and
the enumeration was then trusted while being wrong. Only the export that
happened to *call* the helper was marked; the export that leaked it — and every
export beside it — was published as certified.

Four entrypoints, one claim each, because a single entrypoint could not
distinguish them: every escape widens to *all* exports of its entrypoint, so
three escapes in one export map would mask each other.

- `./called` — the control. `Direct` calls `Panel`; nothing takes it as a value.
  The enumeration is exact, `Direct` is marked, and `Isolated` stays certified.
  This is the export that a widen-everything regression would break.
- `./argument` — `Escaped` hands `Panel` to `apply`, which invokes it. Both
  exports go unknown.
- `./returned` — `makePanel` returns `Panel` to the caller. Both exports go
  unknown.
- `./rendered` — `App` renders `<PanelView/>`. Both exports go unknown.

Widening to every export here is imprecise, not wrong: nothing in the package
proves the escaped helper is unreachable from `Isolated`'s caller, and the
opposite error publishes a certified negative about an export whose behavior
depends on an unresolved obligation.

The declarations are exact for this fixture package. `channel.js` and
`panel.js` deliberately have no sibling `.d.ts`: with one, every importer binds
to the declaration file, the caller edges vanish, and the enumeration widens to
every export for a reason that has nothing to do with an escape — which would
make three of these four entrypoints pass for the wrong reason.
`declaration-sibling-reach` is where that shape is pinned.
