# ADR 0296: The daemon retains dependency-source programs

- Status: accepted and implemented (2026-10-14). The sibling became a
  scratch file opened at a logical path: ADR 0303.
- Owner: `dependency_sources::prepare_program` and
  `PreparedDependencyProgram` (preparation), `dependency_program.rs` (the
  shared fold and bind), `daemon.rs` (retention).
- Step 3 of moving dependency analysis from contracts to source (ADRs 0284,
  0291).

## Context

`--dependency-source` (ADRs 0284, 0291) bypassed the daemon. Every check
opened a Type Facts program on the project's own tsconfig to read its
configured sources and module graph, selected packages, wrote a sibling
tsconfig, opened a second program on it, folded host constants, and bound
redirects. A repeat check of an unchanged project paid all of it again: about
5.8 s on `probus-hk` under `*`.

## Decision

**One path.** Preparation no longer writes anything. `prepare_program`
answers a `PreparedDependencyProgram`: the original tsconfig, the host, the
project's program files, the selection (roots, contract-removal names,
closure files, resolution rows) and the sibling tsconfig's exact bytes. Notes
are returned instead of printed. `dependency_program.rs` holds the fold and
the binding of roots and redirects. A one-shot check and the daemon call the
same three functions. The one-shot check writes the sibling under its process
id and removes it, as before, and now reports under the project's own
tsconfig rather than the sibling's name.

**Retention.** A dependency-source check is daemon-eligible. The daemon
retains two programs: the project's own and the analyzed one. Before every
answer it:

1. refreshes the analyzed program with the existing freshness checks
   (tsconfig, directory stamps, file fingerprints). Package files are program
   files there, so their edits are seen;
2. refreshes the project's own program the same way, applying edits as
   overlay updates;
3. prepares again from that program and compares the whole
   `PreparedDependencyProgram` and its notes with the retained one;
4. folds every package file again and compares each fold's digest with the
   retained one. This sees changes to files the fold reads but neither program
   holds, such as `@solidjs/web`'s `dist/web.dev.js`.

A difference in 3 or 4 reopens the analyzed program from scratch. A change in
1 or 2 that the freshness checks call a rebuild reopens both. Otherwise the
analyzed program takes the edits, except an edit to a package file whose old
or new text folds: an edit carries the program's own text, so that file
reopens the program too. Roots and redirects are bound to every generation's
facts by a session transform, never attached once.

**Sibling.** The daemon names its sibling by a SHA-256 of its selection domain
and bytes. It reuses an existing file only when its bytes are equal, never
rewrites one, and refuses one with other bytes. It removes the sibling when a
reopened program no longer uses it and when it exits.

**Identity.** The socket and the request carry the normalized selection and
the whole runtime host. The daemon refuses a request for another selection or
host. Notes reach the client in the response header, before the catalog
notice, where a one-shot check prints them.

**Fails closed.** A dependency-source check with contract-emission inputs,
`--feedback-facts`, producer arguments or ADR 0295's `--runtime-realm window`
stays one-shot. A refused selection fails the daemon's open, and the client
runs one-shot.

## Consequences

- `probus-hk`, `*`, release binary, AC power, other builds running:

  | Run | Wall (3 runs) |
  | --- | --- |
  | one-shot | 5.85, 5.72, 5.73 s |
  | cold daemon | 5.94, 6.17, 6.18 s |
  | warm daemon (cache hit) | 0.24, 0.24, 0.23 s |

  All nine outputs are byte-identical JSON.
- A cold check costs what a one-shot check costs. A changed input costs a
  reopen, except a project-file edit or a package-file edit that does not
  fold, which are incremental.
- The daemon trusts what the base daemon trusts: a file whose stamp is
  unchanged is not reread, and inputs neither program tracks (declaration
  files, an extended tsconfig, files under `node_modules` that are not program
  files) are not watched. Step 2 of the design (a producer input manifest)
  would close this.
- Each retained check holds two producer processes.

## Evidence

- **Process tests** (`tests/dependency_daemon_process.rs`, armed): every
  `dependency-source*` fixture, its own list and `*`: cold daemon, warm daemon
  and one-shot agree on JSON and notes, and the warm one is a cache hit.
  Mutations on copies each agree with a forced fresh run, with the change
  taken as stated: a package runtime edit (edit), a project file edit (edit,
  list and `*`), an added `.mjs` beside a `.js` closure file (reopen), a
  changed `exports` map (reopen), a host-fold runtime target edit (reopen),
  and an edit to a folded package file (reopen).
- **Unit tests:** the sibling is keyed by its bytes and never rewritten; the
  selection domain is part of the socket identity.
- **Coverage:** no snapshot moved.

## Review round 1

Five defects, all reproduced as process tests and fixed. Three were in the
base daemon too, and are fixed there.
- **A failed refresh consumed its edits.** A refresh records each file as it
  reads it, and the project's own program took its edits before preparation
  could still refuse. A later request then saw nothing new and returned the
  answer from before the edit. Now any request that fails discards the whole
  retained state (session, trackers, cached answer, sibling), and the next
  request opens it from scratch. Base mode had the same defect for a file
  refused as not UTF-8 after another file's edit was read.
- **Program membership was fixed at open.** An edited import can add a file
  to a program or drop one without any file changing on disk. The project's
  program now compares its module list after every edit, and the analyzed
  program does the same around its own edits; any change opens the program
  again. Both directions, and base mode, are tested.
- **Relative paths were the first caller's.** The client now makes
  `--accepted-contracts`, `--receipt-trust-configuration` and the Type Facts
  path absolute against its own directory before it computes the socket or
  sends the request. A bare producer name is resolved through the client's
  `PATH`. A JSON request with a relative project id stays one-shot.
- **The idle watchdog could end a request, and a short body was an
  answer.** The watchdog now skips while a request is being served. The
  client reads the whole body, compares its length with the header's, and
  writes nothing until it has. A short body falls back to one-shot. A failed
  write to stdout is reported and does not fall back.
- **Fields the daemon drops were accepted.** A request is retained only with
  generation 1, and, in both modes, without `--contract-package-root`,
  runtime module resolutions, a proposal catalog, contract batches, a module
  inventory, `--feedback-facts` or producer arguments. Before, base mode
  answered these without them.

Evidence: six new process tests in `tests/dependency_daemon_process.rs`,
each failing before its fix. No snapshot moved.
