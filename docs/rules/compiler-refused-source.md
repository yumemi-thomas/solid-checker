# compiler-refused-source

`SC9015` · **error** for a proven refusal, **warning** (uncertifiable notice) for any other

The pinned Solid 2 compiler refused one of the project's files, so that file
has no compiler facts. The rest of the project is analyzed
([ADR 0360](../decisions.md#adr-0360), [ADR 0441](../decisions.md#adr-0441)).

## What it does

Every analyzed file is compiled by the semantic-facts fork of the Solid 2
compiler the checker pins. When the compiler declines a file (its parser or
transform returns an error: rc.17 refuses `attr:`, `bool:`, `on:`,
`oncapture:` and `use:` on intrinsic elements, Solid 1's `classList`, and
markup a browser would restructure, such as a `<p>` inside a `<p>`), the file
keeps its syntax and Type Facts and carries no compiler facts:

- nothing in the file is proven: no site of it is placed (tracked JSX,
  event, render, deferred), its census is one hole, and a finding located in
  it is uncertifiable;
- nothing that depends on it is proven: a helper its JSX calls, a component it
  renders, a callback it passes, a function only it invokes, and a call of its
  exports from another file take no role from it, and any violation whose
  location, related location or evidence lies in the file is reported
  uncertifiable;
- every syntax-wide census (writes to intrinsics, reflective writes, provider
  and reference censuses) still reads the file, so no premise is kept because
  the file was dropped;
- one SC9015 per refused file names it and quotes the compiler's first line;
  the evidence carries the whole message.

Findings that do not depend on the file stand.

### An error: a removed 1.x attribute `tsc` accepts

The compiler errors on every removed Solid 1 attribute form on a native
element in the dom and ssr generates, whatever the options
(`packages/compiler/src/shared/removed.rs:19-52` in the pinned fork;
solidjs/solid #4199, merged before rc.17). The file does not build with the
pinned compiler, so SC9015 is an **error**, one per file, at the file's start.

It is the project's defect only where `tsc` is silent. TypeScript exempts a
name containing a hyphen from the excess-property check, so `attr:data-z`,
`on:custom-event`, `use:my-tip` and `bool:data-flag` type-check, while
`attr:title`, `bool:hidden`, `on:click`, `use:tip` and `classList` are TS2322
against the rc.17 typings (`fixtures/tsc-oracle`). Only the first kind is an
error.

**Installed compiler.** The checker judges against the pinned rc.17 compiler,
not the one a project installed. A project whose installed compiler is older
(the corpus app `app-game` has rc.4, which still accepts `attr:`) still gets
the error: its code does not build under the Solid release this checker
audits, and the installed one is not what the rest of the analysis assumes
either (see [unaudited-solid-release](unaudited-solid-release.md)).

### A notice: every other refusal

The finding is the project-level notice of ADR 0360, a **warning**,
uncertifiable, shown to users like SC9014's, so a run that would otherwise look
clean shows it and `--certify` fails. It claims no defect:

- `classList` and an unhyphenated removed form: `tsc` reports the same
  attribute, and the checker's claim is only that the file has no compiler
  facts;
- markup the compiler refuses: an error only while the compiler's `validate`
  option is on, which the checker does not model;
- a fragment under an element, an unimplemented form, a namespaced component
  callee: errors of a generate or a limitation of the pinned compiler that the
  checker does not claim for the project.

A selected dependency-source package (ADR 0291) is judged before it enters the
program: selection compiles every closure file that holds JSX, and a refusal
makes the closure unavailable. Under `*` or the default selection the package
is skipped with a note and stays off source, so its imports keep their
internal [package-behavior-unavailable](package-behavior-unavailable.md)
obligations; an explicitly named package fails the check, as every other
unanalyzable named package does. A closure file refused only at fact building
(no JSX at selection) is treated like a project file.

What stays fatal: a compiler run that returns no semantic trace, a trace whose
version or producer identity disagrees with the pin, a trace that leaves a JSX
expression unclassified, and a source file Oxc cannot parse. Each means the
pinned producer, not the source, cannot be trusted.

## How to fix

Rewrite the construct the compiler names in the form its message gives
(`attr:data-z={z}` as `data-z={z}`, `on:click` as `onClick`, `classList` as
`class`). A package shipping source this compiler refuses (Solid 1 syntax,
say) stays off source, and its imports stay obligations, until it ships source
the compiler reads.

## Suppression

There is no site to suppress, and disabling the rule in the options file has
no effect: the finding is added after rule enablement, like SC9014. Fix the
file.
