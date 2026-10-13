# ADR 0124: A negative row may be scoped to a host target

- Status: accepted and implemented (2026-09-25); written with the implementation
- Date: 2026-09-25
- Owners: the dialect's negative table (`solid-dialect/src/lib.rs`, `solid_2.rs`,
  `audited-archives.json`, audited slices), the census dialect-axiom terminator
  (`type_facts.rs`), the generator's `creates` walk (`creates_walk.rs`), and
  consumer case selection (`contract_interface.rs`, shared by project catalogs
  and compiled-in bundles)
- Relation: amends ADR 0007 (negative rows) a second time: rows were
  archive-scoped on 2026-09-25 and may now also be host-target-scoped. It
  implements option (c) of the rc.3 core-primitives audit § 7.4, under the
  owner's standing resolution of (a) plus (d)'s diagnosis ("a guarded reach
  still counts, and a flat row must withhold"). Step 7 part 2, slices 2a and 2b
  (ways-to-improve § 3.4, § 5 decision 5). Its delivery rests on ADR 0123.

## Context

`solid-js` re-declares `createSignal`, `createMemo` and `createEffect`. Their
browser bodies perform no `create`, but the `node`/`worker`/`deno` bodies can
reach `ctx.serialize`, which the 2026-09-04 decision counts as one. A row keyed
only by `(package, version, export, domain)` has to answer for every
condition, so R6 (`createSignal`) was withheld and `createEffect`'s row was
withdrawn. Those rows sit between the tier and any `solid-js` wrapper's
`creates` (ways-to-improve § 3.4: 33 dialect-silent domain-sites measured).

## Decision

**A negative row carries a scope: `EveryCondition` (all 74 existing rows,
unchanged) or `HostTarget { condition, runtime, delegates }`. A scoped row
answers only for a certification whose requested conditions select, from the
authenticated `package.json`, a runtime file the audit read, and only when
every delegate it names is denied by the one audited archive the closure
carries.**

### The row scope

`HostTargetCondition` is an enum (only `Browser` today), so a new host is a
compile error at every match. `denies` and `primitive_performs_no_operation`
read every-condition rows only. A scoped row is reachable only through
`host_target_row()`, which requires every dialect that lists the archive to
state the identical scope. A scoped row sits on a withheld flat row, and the
table derivation test enforces that; `HOST_TARGET_READINGS` ties each scoped row
to its audit section.

### Certification

The census terminator takes the plan's requested export conditions
(`import_request.export_conditions`). A scoped row answers only if all of
these hold:

1. the host condition is in the requested set;
2. `.` replayed with `resolve_snapshot_export` from the authenticated
   `package.json`, under exactly that set, selects a file in `runtime`;
3. for each delegate, exactly one distinct authenticated dependency snapshot of
   that package exists, it matches an audited archive in all four fields, and
   that archive's every-condition row denies the delegate.

The witness site names the condition, the resolved file and each delegate's
archive, so the receipt binds them. Every other case fails closed and says why:

- the condition is absent;
- the resolved file was not read by the audit;
- `.` does not resolve;
- there is no or an unusable `package.json`;
- the closure carries no copy of the delegate, or two;
- the delegate's archive is unaudited;
- the delegate's archive has no denying row.

The proposal side reads the same condition set, so an `["import"]` case still
declines as dialect-silent rather than proposing a closure the census would
only withhold.

### The consumer

Case selection, which project catalogs and compiled-in bundles share, drops
every case that carries a host-target condition when the host declared none.
That happens before candidates are compared. **An undeclared host, which is
every ESLint and Oxlint run, never receives a browser-scoped case**; users opt
in with `--runtime-target browser`. Declared hosts keep the subset rule. With a
delegate, the contract's truth also depends on the installed signals archive,
which ADR 0123 already checks at admission.

### The one row (2b)

`(solid-js, 2.0.0-rc.3, createSignal, creates)`, scoped `HostTarget { browser,
runtime: [dist/solid.js], delegates: [(@solidjs/signals, createSignal, creates),
(@solidjs/signals, getOwner, creates)] }`. Its citation is the rc.3 audit
§ 7.3 R6 reading of `dist/solid.js` bytes 23266..23358 (the wrapper). The
file digest matches the pinned `files.json`, and the slice is checked in.

Only `dist/solid.js` is listed: § 7.3 cites that file's lines, and notes only
that the wrapper in `dev.js`, `solid.cjs` and `dev.cjs` is identical, without
following their hydration helpers. So `development` and `require` resolutions
refuse. `getOwner` is a delegate because § 7.3 follows
`peekNextChildId(getOwner())` into signals. The other helpers it reaches are
not primitives, so no row can name them; they rest on the archive-wide
host-boundary censuses. Negative rows: 74 -> 75.

## Consequences

- No fixture snapshot, ownership ledger, contract-corpus figure or compiled-in
  bundle moved. No tier case carries `browser`.
- One unit expectation moved, as the consumer rule requires: with no declared
  conditions, an SSR package's client case (which carries `browser`) is no
  longer admissible beside its server case.
- The pinned census cannot measure the row, because it certifies no
  `browser` case. A separate run is recorded in the backlog.
- Still open:
  - `createMemo` and `createEffect` (slice 2c) need new hand readings;
  - an undeclared host still receives `node`-scoped cases;
  - a narrow guard-aware form (option (d): "argument 0 is not callable"),
    which would hold for every host, needs per-slot argument facts from the
    producer.
