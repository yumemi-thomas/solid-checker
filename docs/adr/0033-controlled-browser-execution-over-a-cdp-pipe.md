# ADR 0033: Controlled browser execution over a checker-owned CDP pipe

- Status: accepted (decision before implementation)
- Date: 2026-09-05
- Owners: package-contract certifier and probe harness
- Relation: admits the profile ADR 0032 reserved, taking the operator decision
  and the two policy decisions that ADR named. ADR 0031's prohibition on
  Vitest browser mode, ambient Playwright caches and fake globals as
  *authority* is unchanged.

## Context

ADR 0032 measured that a headless Chromium boundary over a launcher-owned
Chrome DevTools Protocol pipe carries the harness mechanics, and withheld
admission on six premises. The lead has since taken the operator decision that
premise 1 required: a browser pinned exactly as Node is pinned — whatever bytes
`PROBE_BROWSER` names at build time, hashed into the verifier — is an
acceptable trust model for a *controlled* profile whose only consumer is the
checker itself. This ADR takes the remaining decisions and records what the
resulting certificate means.

## Decision

Add `chromium-headless-shell-cdp-pipe-esm-v1`, a controlled execution profile
with the following complete scope:

- **Module supply.** The selected runtime target and every transitively
  reached runtime module are UTF-8 `.ts` files in the same authenticated
  snapshot, admitted by ADR 0030's relative-import strip-only grammar (an
  import-free module is the one-node case). The runtime dependency closure is
  empty. Every module's derived bytes are the native byte-position-preserving
  erasure, and pinned Node 24.11.1's `stripTypeScriptTypes(mode: "strip")` must
  independently reproduce them before anything is served. The browser
  transforms nothing.
- **Browser pin.** `SOLID_CHECKER_PROBE_BROWSER_SHA256`, compiled in by the
  build from `PROBE_BROWSER` — the real path of the headless-shell executable —
  as the path-ordered tree digest of that executable's directory, in exactly
  `hash_tree`'s framing, refusing symlinks and any non-regular entry. It is
  verified before every launch and re-asserted against the pin in every
  census. The version is asked of the pinned bytes (`--version`) and must agree
  with `Browser.getVersion` and the startup frame's user agent. A build
  without the pin refuses this profile only; the Node profiles are unaffected.
- **Driver.** A checker-owned CDP client inside the verifier speaking over
  descriptors 3 and 4 (`--remote-debugging-pipe`). No debugging port, no npm
  driver, no Vitest. The page bootstrap is Rust-authored bytes compiled into
  the verifier, instantiated per launch with that launch's nonce and pid.
- **Resolution premise (replaces `verify_reported_resolution` for this profile
  only).** The browser has no package resolver. Every request the page makes is
  intercepted at request stage. A request whose exact URL is in the served map
  is fulfilled from the authenticated derived bytes; any other request fails
  *and refuses the launch*. The served map is the document, the recipe, and
  one exact URL per derived module under a fixed synthetic origin; bare
  specifiers reach it through an import map the checker derives from the plan's
  verified resolution and the native edge map. After the run the set of URLs
  requested must equal the served map exactly. This is stronger than an echo
  in one respect — the driver enumerates every request — and weaker in
  another — there is no independent interpreter selection to compare with —
  and the policy digest says so.
- **Profile directory (sandbox-scheme decision).** The browser writes into its
  `--user-data-dir` on every run. That directory is placed inside the 0700
  private workspace as one explicitly named subtree whose *contents* are not
  watched; its name is fixed, its existence is part of the watched
  private-directory entry census, and nothing the transaction reads lives in
  it. The property detect-and-refuse protects — no probe run altered an input
  the rest of the transaction reads — is preserved exactly; the policy digest
  names the carve-out.
- **Realm and service surface.** The served document carries a CSP that names
  the synthetic origin for `script-src` (plus one per-launch nonce for the
  import map) and sets `default-src`, `connect-src`, `worker-src`, `child-src`,
  `frame-src`, `object-src`, `base-uri` and `form-action` to `'none'`. The page
  session auto-attaches new targets paused, and any attached target — a
  worker, an iframe — refuses the launch. Downloads are denied, the host
  resolver maps every name to not-found, background networking, component
  updates, sync, crash reporting and GPU caches are disabled, and the browser
  runs headless. Storage is not denied and the digest says so.
- **Bootstrap.** Before the document exists, the bootstrap captures the report
  binding and every primordial the report path needs, deletes the binding from
  `window`, freezes `Object`, `Array` and `Function` prototypes, and reports
  one startup frame. After `DOMContentLoaded` it imports the recipe, hands it
  the harness, drains, and reports exactly one run frame in the Node worker's
  frame shape. Frames are null-prototype records serialized without consulting
  `toJSON`; a third frame, a frame from another execution context, or a frame
  without the launch nonce is a protocol refusal.
- **Drain.** A new bounded `animation-frames` drain step, admitted by the
  recipe policy's `maxAnimationFrameTurns`. The Node worker refuses it as an
  unknown step; the transcript records it only when nonzero, so existing
  transcripts, plan digests and corpus roots are byte-identical.
- **Lifecycle.** Own process group, `killpg` on every exit path, one startup
  budget and one run budget, `env_clear` with the existing allowlist, cwd inside
  the private directory, census before the first launch, between launches and
  on every exit path. The Node executable, Type Facts image and verifier image
  remain watched; the browser bundle is re-asserted against its pin.
- **Consumer.** The checker's own fresh replay under the same pins, exactly as
  ADRs 0028 and 0030. Controlled receipt version moves from 5 to 6 with a new
  signature domain, the profile pairs with proof identity
  `policy-2-with-browser-cdp-pipe-erasure-v1`, execution request schema 9
  selects it, and its sandbox policy is a separate browser scheme digest
  (`scheme-version:11`, `scheme-family:browser-cdp-pipe`). Ordinary policy-2
  consumers refuse the receipt, and the Node scheme-10 vector is unchanged.

## What the certificate means

A receipt under this profile means: the native implementation census closed the
selected `creates` domain over authenticated published source; pinned Node's
strip-only transformer reproduced the native derived bytes for every module;
and the mandatory finite veto observed no contradiction while exactly those
bytes, and nothing else, were executed by the pinned headless shell through
the checker's exact URL map under the policy above. A clean finite run never
supplies the closure; the census does.

It says nothing about any application's browser, bundler, transform, polyfill
or layout engine, about Node execution of the same source, or about storage,
timing or rendering behaviour the recipe did not observe. The interpretation
is a hybrid — Node-strip derived bytes executed by Chromium — that no
application performs, and the reflection counterexample of ADR 0028 continues
to forbid cross-profile reuse.

## Alternatives considered

- **Vitest browser mode as the harness.** Vite strips `.ts` with esbuild,
  which reformats the module, so the executed bytes can never equal the native
  byte-position-preserving witness pinned Node reproduces; and Vite's dev
  server, websocket, runner protocol and Playwright provider would sit
  unpinned on the report path. Rejected even with the lead's permission to use
  it: it can be an instrument, never the boundary.
- **A watched profile directory.** Refuses every run; the browser writes ~51
  files in a 364 ms launch. Rejected in favour of the named carve-out.
- **Serving modules from `file:` URLs or a local HTTP server.** Adds an origin
  the checker does not own. Rejected in favour of request-stage fulfilment.
- **Keeping the resolver-echo premise.** There is nothing independent to echo
  in a browser; the exact request census is the honest replacement, and the
  policy digest carries the change so the receipt cannot be read as Node's
  resolution claim.
- **Bumping the Node scheme to 11.** Would move every Node profile's receipt
  identity without changing its meaning. The browser scheme is a sibling
  vector instead.

## What implementation taught (2026-09-05)

Three facts the design did not predict, each now in code and test:

- **Two launcher-side echoes are not foreign realms.** `Target.attachToTarget`
  echoes the launcher's own attach as an `attachedToTarget` event on the
  browser session, and `Runtime.enable` reports the initial `about:blank`
  main-world context before navigation replaces it. The driver skips exactly
  the attach whose target id it created, and accepts frames only from the
  *current* default context whose frame id is the page's own; an iframe's
  default context names another frame and still refuses.
- **A blocking `Page.navigate` deadlocks against request-stage interception**:
  the navigation response arrives only after the document request the driver
  must answer. Navigation is therefore issued without awaiting its response.
- **`connect-src 'none'` acts before interception.** A recipe's `fetch` is
  rejected by the browser before any request exists, so the interception
  refusal is exercised by an unmapped *module* request (`script-src` admits the
  origin, the URL is not served). Both dispositions stand; the fixture recipe
  and README say which is which.

Measured on the three census-complete Kobalte candidates, one controlled
execution — census, pinned-Node reproduction, two veto launches, receipt
authentication, two consumer launches, and five bundle-tree censuses of a
196 MB directory — takes 36–38 s wall clock.

## Consequences

- At most the three census-complete browser candidates can gain a scoped
  closure; `exportsProven` stays 0 and no ordinary consumer accepts the result.
- The seventeen native census refusals are untouched; a browser executor
  cannot waive a census.
- Every launch pays a tree hash of a 196 MB bundle three times; measured cost
  is recorded in the dated report.
- Reviewers should read `probe_harness/browser.rs`'s header, which mirrors ADR
  0006's disposition discipline for the browser's own service surface, and the
  `BROWSER_SANDBOX_POLICY_FIELDS` literal test.
