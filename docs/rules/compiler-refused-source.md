# compiler-refused-source

`SC9015` · **warning** · uncertifiable

The pinned Solid 2 compiler refused one of the project's files, so that file
has no compiler facts. The rest of the project is analyzed, and this notice
says the result cannot certify it ([ADR 0360](../decisions.md#adr-0360)).

## What it does

Every analyzed file is compiled by the semantic-facts fork of the Solid 2
compiler the checker pins. When the compiler declines a file (its parser or
transform returns an error: rc.17 refuses `attr:`, `bool:`, `on:`,
`oncapture:` and `use:` on intrinsic elements, Solid 1's `classList`, and
markup a browser would restructure, such as a `<p>` inside a `<p>`), the check
used to stop with a fatal
`native Solid compiler facts error`. Now the file keeps its syntax and Type
Facts and carries no compiler facts:

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
- one project-level notice per refused file names it and quotes the
  compiler's first line; the evidence carries the whole message.

Findings that do not depend on the file stand. The notice is user-facing, as
SC9014's is, so a run that would otherwise look clean shows it and `--certify`
fails.

A selected dependency-source package (ADR 0291) is judged before it enters the
program: selection compiles every closure file that holds JSX, and a refusal
makes the closure unavailable. Under `*` or the default selection the package
is skipped with a note and stays off source, so its imports keep their
[package-behavior-unavailable](package-behavior-unavailable.md) obligations;
an explicitly named package fails the check, as every other unanalyzable named
package does. A closure file refused only at fact building (no JSX at
selection) is treated like a project file.

What stays fatal: a compiler run that returns no semantic trace, a trace whose
version or producer identity disagrees with the pin, a trace that leaves a JSX
expression unclassified, and a source file Oxc cannot parse. Each means the
pinned producer, not the source, cannot be trusted.

## How to fix

Rewrite the construct the compiler names in a form the pinned compiler
accepts (`attr:data-z={z}` as `data-z={z}`, `classList` as `class`). A package
shipping source this compiler refuses (Solid 1 syntax, say) stays off source, and its
imports stay obligations, until it ships source the compiler reads.

## Suppression

There is no site to suppress, and disabling the rule in the options file has
no effect: the notice is added after rule enablement, like SC9014.
