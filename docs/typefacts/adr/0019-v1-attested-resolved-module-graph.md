---
status: accepted
---

# V1 adds the compiler's resolved module graph

## Decision

The lifecycle operation set gains `modules`, which answers for the module graph
of the accepted program. Handshake protocol moves 1 → 2; the schema digest moves
with it, as it does for every vocabulary change.

The answer has two halves.

**The module inventory** is unconditional and is every file the program
included: its cleaned absolute path, whether it is a declaration file, its emit
module format, the compiler's project-reference input/output pairing, and its
duplicate-install redirect targets. For a module reached through a symlink the
path is the realpath, matching the resolver's own.

**Resolved import provenance** is asked for, one fact per module specifier
occurrence in the requested files: the specifier's exact span and text, the file
the resolver selected, the file the program parses in its place, the symlink path
the resolver walked before taking a realpath, the extension, whether a
configured `paths` key matched the specifier, and — when asked for — the owning
`package.json`'s name, version, and path, plus the package identity the resolver
itself recorded.

The response carries these as three flat fields rather than a packed frame. The
Wire table schema is unchanged at v13: the module graph is a property of the
accepted program rather than of a demand set, so it takes no part in retained
table identity, edits no retained demand set, and advances no generation. A
`modules` request may be issued at any point in a session's life without
disturbing a materialized analysis.

`ModuleGraphDemand::import_paths` scopes the import half. A requested path the
program does not hold is reported in `unknownImportPaths`, never dropped, and
`ModuleGraph::is_complete` is the client-side name for "the answer covers
everything that was asked about". A backend with no compiler resolution fails the
request rather than answering a partial inventory.

## Why the protocol number moved and the digest was not enough

ADR 0003 established that a coordinated bump moves the schema digest, and that
the handshake's three-way refusal — protocol, digest, build id — means a producer
and a client have always had to ship together. Nine vocabulary changes since have
moved the digest alone, correctly: each added a fact to an existing operation, and
a peer that did not understand the fact simply never asked for it.

This one is different in kind. It adds an *operation*. A peer that does not know
`modules` exists cannot be paired with one that does in any partial way, and the
protocol number is the field that says so. Moving it costs nothing — both
executables ship in lockstep regardless — and it makes the refusal message name
the actual incompatibility rather than pointing at a digest that happens to have
changed. `a_producer_that_differs_on_any_handshake_field_is_refused` pins that
the refusal itself is unchanged.

## What the compiler knows about declaration-to-implementation pairing

This is the load-bearing question, because a consumer joining an import bound to
a `.d.ts` back to the runtime module it describes can close no more of that gap
than the compiler records. The answer is narrow, and stating it precisely is more
useful than widening it.

**TypeScript records exactly one such pairing: a configured project reference's
declaration output and the input it was emitted from.** The
`projectReferenceFileMapper` holds it in both directions, so the program can be
asked which input a `.d.ts` output came from and which output an input
corresponds to. `Program.GetParseFileRedirect` is the same record from the
resolution side: it names the file the program parses in place of the one a
specifier resolved to. That is reported as `ModuleFact.projectReference` and
`ModuleImportFact.includedPath`.

**It records nothing at all for the shape almost every published package has.**
`index.js` writes `import { channelFor } from "./channel.js"`; a `channel.d.ts`
beside `channel.js` wins that resolution in every mode, and the resolver returns
the declaration file and stops. It never opens `channel.js`, never compares the
two, and holds no link between them — they are separate modules that happen to
share a name on disk. `module.ResolvedModule` has no field for it, and no
`outDir`/`rootDir` mapping applies, because no emit produced the `.d.ts`.

So the graph reports the absence rather than inventing a pairing. The consequence
for a consumer is exact: **the `.d.ts` identity split cannot be closed by module
identity for a published package.** What the graph does supply is enough to see
the split rather than be surprised by it — `extension` shows the specifier landed
on a declaration file, the inventory shows the runtime file is present as its own
root, and `includedPath` is empty. A consumer must fail closed on that
combination; it must not pair the two by matching file names, which is the
substitution the precision contract forbids.
`TestDeclarationSiblingHasNoCompilerRecordedRuntimePairing` and the process test
both pin the absence so a later change cannot quietly fill it in.

## What the resolver records about *how* a specifier resolved, and what it does not

`ModuleResolution` carries only distinctions `module.ResolvedModule` makes:
`relative` (the resolver treated the specifier as relative or rooted),
`nodeModules` (`IsExternalLibraryImport`), `nonRelative` (a bare specifier that
resolved outside every `node_modules` tree), and `unresolved`.

`ResolvedModule` does **not** record whether a `paths` mapping or a
`package.json` `exports` entry participated. Neither is reported as though it
did.

For `paths` there is a separate, honest fact. `pathsPattern` runs the compiler's
own `core.Pattern` matcher over the configured keys, under the compiler's own
eligibility rule (`paths` non-empty and the specifier not relative) and its own
longest-prefix tie-break. It says *the mapping matched the specifier*, which is a
fact about the configuration and the text. It does not say the resolution came
through the mapping — TypeScript tries `paths` first and falls through to
ordinary resolution when the mapped candidate does not exist, and nothing records
which happened. Read with `resolution` it is still decisive for the case it
exists to serve: a bare specifier that a `paths` key matched and that did *not*
land in `node_modules` is not the installed package of that name, however
identical the specifier text is.

For `exports` there is no equivalent and none is claimed.
`ResolverPackageId.subpath` is `PackageId.SubModuleName` — the selected file's
path within the package, not the `exports` key that led there.

## Symlinks

`preserveSymlinks` is off by default, and the resolver takes a realpath for a
non-relative resolution landing in `node_modules`. `ResolvedFileName` is
therefore the realpath and `OriginalPath` the path it walked, populated only when
the two differ. Both are reported: `resolvedPath` and `symlinkPath`.

This is the pnpm and workspace-link shape, and the direction matters. The
inventory names realpaths, so a closure record built from it identifies one copy
of a package rather than one link into it, and two dependents sharing a store
entry agree about which bytes were read. The owning `package.json` is looked up
from the realpath for the same reason: a contract bound to it names the installed
copy. A consumer that needs the link path — to report a diagnostic in the terms
the user's `node_modules` uses — has it, separately and explicitly.

An empty `symlinkPath` means the resolver saw no divergence, not that none was
looked for; for a relative import it never looks.

## Consequences

- A consumer recording which modules an analysis read stops reconstructing the
  list by scanning source text and resolving specifiers in a second process. The
  reconstruction could disagree with the compiler in ways neither side reported —
  a `paths` mapping, a condition resolved differently, a specifier classified as
  external that the program in fact opened — because the process that resolved the
  modules was the other one. The inventory is that process's own answer.
- A contract keyed on a package *name* can be checked against a resolved
  *module*. `resolvedPath` plus `package` answers whether an import actually
  reached the package a contract describes; `pathsPattern` and `resolution`
  together identify the alias that shadows it.
- The inventory is the program's file list, so it includes the default library
  files the analysis opened. That is deliberate: a record of what was read should
  name what was read.
- Cost is proportional to the program for the inventory and to the requested
  files for the imports, and the whole answer is one plain CBOR frame. It is an
  explicit operation, like `sources`, not something an analysis pays for.
