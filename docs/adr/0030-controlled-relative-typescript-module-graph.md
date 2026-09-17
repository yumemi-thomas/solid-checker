# ADR 0030: Controlled relative TypeScript module graphs

- Status: accepted
- Date: 2026-09-05
- Owners: package-contract certifier and probe harness

## Context

ADR 0028 deliberately starts with import-free TypeScript. Eight of the original
40 Kobalte candidates import another package-local source file through an
extensionless relative specifier. Pinned Node 24.11.1 can strip each `.ts` file,
but its ordinary ESM resolver does not append `.ts`; letting a loader guess a
suffix would execute a graph the certification plan never authenticated.

The package closure verifier already replays static imports from authenticated
snapshot bytes with one deterministic resolver and requires the supplied
closure manifest to equal that replay. The missing premise is an execution
profile that transports those exact replayed edges to the pinned worker and
binds every derived module, instead of asking Node to rediscover them under a
different algorithm.

## Decision

Add `node-strip-relative-ts-graph-esm-v1`, a controlled execution profile with
the following complete scope:

- the selected runtime target and every transitively reached runtime module are
  UTF-8 `.ts` files in the same authenticated package snapshot;
- every import is a static runtime relative import admitted by the profile's
  syntax whitelist; re-exports, dynamic imports, `require`, `import.meta`, type-
  only imports, import attributes, bare specifiers and URL imports refuse;
- each `(importer, literal specifier, target)` edge is produced by the same
  native snapshot resolver used to replay the closure, and the target must be a
  runtime entry in that verified closure;
- the graph walk must reach exactly the runtime entries relevant to these
  static edges. Missing, ambiguous, declaration-only and external targets
  refuse.

For every module, Oxc derives the byte-position-preserving strip-only output and
pinned Node independently recomputes it with
`stripTypeScriptTypes(source, { mode: "strip" })`. Source path, source digest,
derived digest, module format, graph edge map, resolver identity, Node identity
and runtime environment are part of the execution binding and receipt. The
private workspace writes and watches every derived file. Its 0700 directory,
cleared environment, process group and killpg, single startup/run frame,
primordial capture and prototype freezing, exact top-level resolution check,
and detect-and-refuse write census remain unchanged.

The worker installs one in-realm hook before recipe import. From a controlled
source URL, only an exact edge-map key is accepted; it short-circuits to that
edge's exact controlled source URL. Any other import from a controlled module
refuses. The load hook supplies only the verified derived bytes for an exact
controlled source URL. Recipe and package entry resolution remain Node-owned,
and Rust still compares the reported entry target with the selected artifact
case.

## Certificate meaning and consumer compatibility

A receipt under this profile means that the native implementation census closed
the claim over authenticated published source bytes, and the mandatory veto
found no contradiction while the exact source graph was interpreted by pinned
Node's strip-only transformer and the profile's exact edge map. The observed
bytes are derived and the receipt says so for every module.

The only production consumer is the transaction's fresh recipe replay under the
same worker, Node pin, graph and environment. Ordinary policy-2 consumers reject
the receipt. A digest by itself does not make another compiler or bundler
compatible; a future consumer must explicitly implement and authenticate this
profile's erasure and edge semantics, including source-reflection behavior.

The transformation can hide contradictions that depend on erased type syntax,
and a different compiler can print runtime functions differently. The scoped
claim remains defensible because it is restricted to this named interpretation
and cannot be reused across profiles. The ADR 0028 reflection counterexample
continues to prevent such reuse.

## Alternatives considered

Use Node's ordinary resolution after writing `.js` copies. This changes both
module URLs and the package's published relative graph, and it can silently
select another file. It is not the artifact case the census authenticated.

Append `.ts` in the worker. This makes an unauthenticated suffix heuristic the
resolver. Even when it happens to match these eight packages, the receipt would
not say why that target was selected.

Rewrite import specifiers in derived output. The output would no longer be
pinned Node's strip-only result, widening the transformation and its possible
semantic differences. The exact hook preserves source text and source URLs.

Treat a completed POC sample as sufficient. The POC did not run the native
census or issue receipts. A finite non-observation never closes a domain.

## Identity changes

Controlled execution receipt version moves from 4 to 5, its signature domain
moves with it, and runtime-probe worker protocol moves from v4 to v5 because the
execution request and echo now carry a module graph. Sandbox scheme moves from
9 to 10: its policy fields now state exact authenticated relative edge mapping,
all-source transform verification, and watched derived graphs rather than a
single exact source URL. Existing profiles use the same stronger protocol and
retain their previous behavior.

## Remaining refusals

This profile does not supply browser APIs. A module that loads but whose recipe
requires `document`, layout, animation frames or another browser service needs
a separately pinned browser execution profile. General package dependencies,
type-only import elision, re-export graphs and dynamic loading also remain
outside this first relative graph profile.
