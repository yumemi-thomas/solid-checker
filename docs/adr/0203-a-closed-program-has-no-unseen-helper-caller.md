# ADR 0203: A closed program has no unseen caller of an exported helper

- Status: accepted and implemented (2026-10-06). Track A, step A5, of
  `docs/2026-10-05-package-direction.md`.
- Owners: `discharge_closed_program_export_dispatch` and
  `CallGraph::entered_only_through_call_expressions` in
  `solid-reactive-ir/src/attribution.rs`, called from `pipeline.rs`. The
  producer of the obligation (`interproc.rs`) is unchanged.
- Relation: applies ADR 0193 (an application is the whole program) to the
  `exported-parameter-member-dispatch` obligation, as `owner_node` already
  does for the unowned seed of an exported helper.

## Context

A helper that invokes a member of its own parameter (`item.label()`) leaves
the choice of implementation to each call. Each call in a rendering function
resolves the member from the argument it passes, or files its own
`parameter-member-target-unresolved` obligation.

An *exported* helper also files one obligation at its declaration
(`exported-parameter-member-dispatch`), for the callers outside the analyzed
files. On the rc.13 corpus, browser host (`rc13-e-browser.json`), that is 814
of the 1,784 caller-supplied-member results. All 49 projects are
applications, which have no outside caller.

## Decision

1. **In a closed program, the declaration obligation is discharged when every
   way of entering the helper is a call expression the call graph resolves to
   it.** Each such call already resolves the member, or keeps its own
   obligation, exactly as a call of an unexported helper does.
2. **Entries are counted by reference, strictly.** Every reference to the
   helper's symbols (its binding and its aliases) must be:
   - the callee of a call expression that resolves to the helper;
   - its declaration or binding;
   - an import, or an export specifier.

   A JSX render, a render through a dialect renderer or a rendering prop, a
   value handed out (`[helper]`, `register(helper)`), a class member, and a
   function with no binding name all keep the obligation. A JSX render is
   excluded on purpose: no call-site analysis resolves the members a
   component invokes on its props.
3. **Open programs are unchanged.** So is contract generation, which never
   runs closed.

## Consequences

- Each discharged obligation was a statement about callers that do not exist.
  Its site-level twin stays wherever a call cannot select the member.
- **Same limit as an unexported helper.** A call made from a function that is
  not a rendering function files no obligation, and its member reads are not
  followed into that function's own callers. This was already true for every
  unexported helper. A closed program now treats an exported one the same.
- Not changed:
  - **object-typed built-ins** (`Array.map`, `Date.getTime`, `Element.focus`).
    Their resolution names a declaration, not the value that arrives. A
    subclass or a structurally compatible object can override the method.
    ADR 0190 drew this line, and call sites with such members keep their
    obligation;
  - **object-literal properties whose value is an arrow function, a function
    expression or a named function.** Only method shorthand
    (`{ label() { … } }`) resolves today. Those properties have no summary
    node (`parameter-member-targets-diverge`, 92 sites); see the precision
    backlog.

## Evidence

- **Fixtures** `closed-export-member-dispatch` (a private `package.json`) and
  `closed-export-member-dispatch-open` (none) share one source:
  - `describe`, entered only through calls: no declaration obligation when
    closed, `SC9012` when open;
  - `escapes`, also handed out as a value, and `Card`, rendered through JSX,
    keep it in both;
  - `describe(props.item)` keeps its call-site obligation in both, and the
    method-shorthand argument is clean in both.
- **Coverage:** 174 fixture projects, 917 findings. Only the two new
  snapshots are new; no existing finding moved.
- **rc.13 corpus**, browser host, release binary, against
  `rc13-e-browser.json`:
  - violations unchanged at 285 (none added, none removed);
  - uncertifiable 4,011 to 3,435: 511 distinct sites removed, none added,
    all `exported-parameter-member-dispatch`. 238 of the 814 stay, for helpers
    rendered through JSX, handed out as values, or declared as class
    members.
