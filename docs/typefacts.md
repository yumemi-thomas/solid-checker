# Type Facts module

The Type Facts module consists of the local Rust producer under
`apps/solid-typefacts` (on the tsrs TypeScript checker; ADR 0365) and the Rust
process/session client under `rust/crates/typefacts`. Its versioned process protocol is the seam between the
certification engine and native TypeScript analysis.
Its interface exposes only opaque identities and original
source facts. TypeScript AST nodes, checker types, symbols, and programs
stay inside the producer's tsrs backend.

## Current behavior

- Open a real `tsconfig.json` project.
- Resolve the symbol at a UTF-8 byte location.
- Resolve import aliases to their original declarations.
- Return declaration locations and non-declaration references across files.
- Build a canonical-symbol reference index from path-owned contributions.
  Multiple reference queries traverse each TypeScript AST only once; small
  successful updates subtract and lazily replace affected-file fragments,
  while broad updates discard the merged index conservatively.
- Return an opaque canonical type identity at a source location.
- Optionally describe a named type's text and source alias declarations without
  exposing native checker objects. Reactive IR uses this to distinguish a
  `solid-js` `Accessor` from a structurally identical plain function.
- Resolve call targets and instantiated return types.
- Optionally enumerate parsed source calls in source order with exact whole-call,
  callee, and argument byte ranges plus an opaque alias-resolved target.
  Reactive IR uses this bulk capability instead of reconstructing call
  structure with regexes and reuses target identities for generation-scoped
  type-description caching; backend AST nodes remain private to the adapter.
- Optionally enumerate call-initialized variable bindings, preserving direct
  versus array destructuring, exact bound-name ranges, and omitted tuple slots.
  Reactive IR shares these facts across reactive-value and setter discovery.
- Optionally enumerate named block-bodied function declarations and direct
  identifier-bound arrows with exact name, body, and parameter ranges plus
  export, async, and declaration-kind flags.
- Optionally enumerate function expressions and declarations with checker-backed
  async return capability and AST-derived calls dominated by `await` on every
  reachable path. Branches, loops, `try`/`catch`/`finally`, concise expressions,
  imported callbacks, and local alias chains are handled conservatively; nested
  function calls are excluded. This keeps async classification and SC1002 flow
  facts inside the producer's tsrs backend.
- Apply monotonically versioned in-memory file overlays, incrementally update a
  single existing source file when safe, and report the changed files plus
  their transitive importers without writing editor contents to disk.
- Discover overlay-added and overlay-deleted files through the same `tsconfig`
  include rules and module-resolution view used by clean projects.
- Reject invalid ranges and ranges that split a UTF-8 code point.
- Apply updates transactionally: cancellation or a rebuild failure leaves the
  prior project generation queryable and unchanged.

Generation-scoped opaque identities include their project generation and must
be reacquired after every successful `Update`. Canonical identities
(`symbol:h:`) are minted again after an update, or answered again by a retained
binder-level answer that holds them (ADR 0369); any other ID held over an
update fails closed with not-found until a location scan mints it. A
binder-level symbol's hash ID is
a pure function of the program (ADR 0366): a checker singleton, a global by
name, a module export by declaring path and name, else its minimum declaration
by path and offset, with the checker's private and unique-symbol counters
stripped from names. Whatever symbol asks first, the ID is the same. A union,
intersection or spread property hashes its combination, name and origins'
binder keys when those fix its type: written annotations reading no `this` or
type parameter, through constituents that do not transform them (ADR 0371). Any other derived symbol (an
instantiation, a `this`-type or widened copy, a mapped property) and any symbol
not proven binder-level get counters (`symbol:<gen>:<n>`). A
table build discards every type-derived answer, so none is reused inside a
generation or across an update (ADR 0364). Answers the checker gives by scope
lookup alone are kept with the digests they read (file text, the exports scan,
the global scope, the options) and reused while those are unchanged: each
file's binder-resolved reference spans, entity symbols at binder-resolved
positions and binder-level symbols' declarations (ADR 0369). The compiler's
incremental program is still reused.

## Native integration pins

- tsrs: the fork `https://github.com/yumemi-thomas/tsrs` at
  `49f534856809ad8d5a596123f02175ba01675c8d`, every tsrs crate pinned to that
  one `rev` in `apps/solid-typefacts/Cargo.toml` (and so in its `Cargo.lock`).
  Upstream `oxc-project/tsrs@467182f` plus one marked commit that exports
  `is_untyped_function_call` and `get_reduced_apparent_type`; it changes no
  behavior. The fork is unpublished: a producer builds only with network access
  to it.
- TypeScript: commit `b85298b6a81f772d080b0455de0ca9d744cd6fd6`
  (`7.1.0-dev.20260929`), the commit tsrs ports. The `tsc` oracle runs the same
  release line (ADR 0361).
- Toolchain: Rust 1.99.1 (`apps/solid-typefacts/rust-toolchain.toml`), CMake for
  the native allocator (mimalloc), and the `wasm32-wasip1` target for the
  reactor.
- Producer identity (`scripts/typefacts-source-identity.mjs`): a digest over the
  producer and client sources, schemas, `Cargo.lock` and the build script, plus
  the tsrs rev, `rustc -vV` and the build id. It stamps `bin/solid-typefacts`
  (`.buildinfo`), keys the CI cache and is recorded in the WASM manifest. The
  native binary is `cargo build --release` of that workspace; the WASM reactor
  is its library built as a `wasm32-wasip1` cdylib
  (`packages/wasm/scripts/build-owned.mjs`).
- The retired Go producer (TypeScript-Go, `go.mod`, the tsgolint shims) is
  archived outside the repository; ADR 0365 records where, and
  [differential.md](typefacts/differential.md) how to compare against it.

No `unsafe` code is allowed outside the producer's single `unsafe_boundary`
module (tsrs frees memory through unsafe calls; the producer copies results out
before disposal).

`AffectedSet` starts from the union of the old and updated resolved-module
graphs. For a single accepted external-module edit, diagnostic-free forced
declaration emit may stop propagation at the edited file when the canonical
`.d.ts` shape and resolved imports are unchanged and external exports pair
bijectively. Canonical module-visible IDs are rebound to their new declaration
spans, so an implementation edit above an export can retain importer facts.
Any failed safety gate retains the transitive reverse-import set. This keeps
removed or redirected imports, inferred exported-type changes, global/module
augmentation, diagnostics, and ambiguous export changes conservative. A
single accepted, non-deleted source edit uses tsrs's
`Program.UpdateProgram`; its structural checks fall back to a new program for
import-graph changes. Added or deleted files, `tsconfig` edits, and multi-file
batches explicitly rebuild the complete program. Both paths remain behind the
same transactional interface.

## Hardening evidence

The integration suite exercises chained, namespace, default, and aliased
re-exports; package export subpaths through package-manager symlinks; project
references; JavaScript with JSDoc generics; overload and generic substitution;
CRLF and multibyte source; clean/overlay equivalence; failed and canceled
updates; overlay file creation/deletion; named-type alias origin; semantic
async returns; await dominance; nested callback exclusion; and conservative
affected-file sets. Reference-index coverage verifies imported
aliases, deterministic source ordering, and invalidation after an overlay
update.
The tsrs revision is one `rev` across all its crates, enforced by the identity
script.
