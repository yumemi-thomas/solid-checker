import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { test } from "vitest";

import {
  assertPhase20Ledger,
  assertFrozenPhase20Ledger,
  buildPhase20Ledger,
  classifyArtifactApplicability
} from "./package-contract-v2-phase20-ledger.mjs";

test("a conditional-only dependency plan requires exact nonempty conditional evidence", () => {
  const bytes = readFileSync(new URL("../benchmarks/ecosystem/report.json", import.meta.url));
  const ledger = buildPhase20Ledger(JSON.parse(bytes.toString("utf8")), {
    reportSha256: createHash("sha256").update(bytes).digest("hex")
  });
  const row = ledger.rows.find(candidate => candidate.externalEdges.length > 0);
  row.dependencyPlan = {
    complete: true,
    status: "conditional-only",
    leaves: [],
    cycles: [],
    conditionalDependencies: [{ kind: "absent-optional-peer", specifier: "optional-peer" }]
  };
  assert.doesNotThrow(() => assertPhase20Ledger(ledger));

  row.dependencyPlan.conditionalDependencies = [];
  assert.throws(() => assertPhase20Ledger(ledger));
});

test("Phase 20 check remains frozen after the Phase 21 report replaces the live corpus", () => {
  assert.doesNotThrow(() => assertFrozenPhase20Ledger({
    ledgerBytes: readFileSync(
      new URL("../benchmarks/package-contract-v2/phase20/row-ledger.json", import.meta.url)
    ),
    markdown: readFileSync(
      new URL("../benchmarks/package-contract-v2/phase20/row-ledger.md", import.meta.url),
      "utf8"
    ),
    phase21Baseline: JSON.parse(readFileSync(
      new URL("../benchmarks/package-contract-v2/phase21/baseline-cohort.json", import.meta.url),
      "utf8"
    ))
  }));
});

test("artifact applicability refuses to reinterpret missing or unsupported bytes as runtime proof", () => {
  assert.equal(classifyArtifactApplicability({ accepted: true }), "runtime-module");
  assert.equal(
    classifyArtifactApplicability({ reason: "resolved target <package-root>/src/index.ts is not a file" }),
    "unavailable-published-target"
  );
  assert.equal(
    classifyArtifactApplicability({ reason: "entry file types/index.d.ts is not part of the TypeScript project" }),
    "unsupported-artifact-shape"
  );
  assert.equal(
    classifyArtifactApplicability({ reason: "different semantics across overlapping conditional-export branches" }),
    "unsupported-condition-environment"
  );
  assert.equal(
    classifyArtifactApplicability({ reason: "authenticated verifier-proved type-only leaf" }),
    "verifier-proved-type-only"
  );
});

test("the checked-in 418-row report produces orthogonal live ledgers", () => {
  const bytes = readFileSync(new URL("../benchmarks/ecosystem/report.json", import.meta.url));
  const report = JSON.parse(bytes.toString("utf8"));
  const ledger = buildPhase20Ledger(report, {
    reportSha256: createHash("sha256").update(bytes).digest("hex")
  });
  assert.equal(ledger.summary.rows, 418);
  // These counts are a live read of benchmarks/ecosystem/report.json and move
  // whenever that report is regenerated against changed proposal semantics.
  // Re-pin them only from a report whose movement is attributable; the current
  // values follow the producer-root, artifact-mechanics, and second-order
  // return-carry rounds landed 2026-09-01/02 (see docs/precision-backlog.md).
  // The return-carry slice adds exactly jsx-parser's authenticated receipt.
  assert.deepEqual(ledger.summary.proposalStates, {
    complete: 344,
    "fully-refused": 37,
    partial: 37
  });
  assert.deepEqual(ledger.summary.certificationStates, {
    "exact-refusal": 49,
    "not-attempted": 25,
    verified: 344
  });
  assert.deepEqual(ledger.summary.failureLedgers, {
    dependencyContractObligation: 29,
    exportKindUnresolved: 0,
    geolocationExportKindConflict: 0
  });
  assert.equal(ledger.summary.classifierCorrections, 0);
  assert.equal(ledger.summary.verifiedRows, 344);
  assert.deepEqual(
    ledger.rows.filter(row => row.certification.state === "verified").map(row => row.probeId),
    [
      "@corvu-next/accordion@0.1.5|solid2|only",
      "@corvu-next/calendar@0.1.5|solid2|only",
      "@corvu-next/dialog@0.1.5|solid2|only",
      "@corvu-next/disclosure@0.1.5|solid2|only",
      "@corvu-next/dismissible@0.1.5|solid2|only",
      "@corvu-next/drawer@0.1.5|solid2|only",
      "@corvu-next/focus-trap@0.1.5|solid2|only",
      "@corvu-next/list@0.1.5|solid2|only",
      "@corvu-next/otp-field@0.1.5|solid2|only",
      "@corvu-next/persistent@0.1.5|solid2|only",
      "@corvu-next/popover@0.1.5|solid2|only",
      "@corvu-next/presence@0.1.5|solid2|only",
      "@corvu-next/prevent-scroll@0.1.5|solid2|only",
      "@corvu-next/resizable@0.1.5|solid2|only",
      "@corvu-next/tooltip@0.1.5|solid2|only",
      "@corvu-next/transition-size@0.1.5|solid2|only",
      "@corvu-next/utils@0.1.5|solid2|only",
      "@corvu/accordion@0.2.5|solid1|only",
      "@corvu/calendar@0.1.2|solid1|only",
      "@corvu/dialog@0.2.4|solid1|only",
      "@corvu/disclosure@0.2.2|solid1|only",
      "@corvu/drawer@0.2.4|solid1|only",
      "@corvu/otp-field@0.1.4|solid1|only",
      "@corvu/popover@0.2.0|solid1|only",
      "@corvu/resizable@0.2.5|solid1|only",
      "@corvu/tooltip@0.2.2|solid1|only",
      "@corvu/utils@0.4.2|solid1|only",
      "@kobalte/utils@0.9.2|solid1|only",
      "@solid-devtools/extension-adapter@0.12.1|solid1|only",
      "@solid-devtools/frontend@0.15.4|solid1|only",
      "@solid-devtools/logger@0.9.11|solid1|only",
      "@solid-devtools/overlay@0.33.5|solid1|only",
      "@solid-devtools/transform@0.10.4|solid1|only",
      "@solid-primitives/a11y@1.0.0-next.3|solid2|floor",
      "@solid-primitives/a11y@1.0.0-next.3|solid2|head",
      "@solid-primitives/active-element@2.1.6|solid1|only",
      "@solid-primitives/active-element@3.0.0-next.2|solid2|floor",
      "@solid-primitives/active-element@3.0.0-next.2|solid2|head",
      "@solid-primitives/analytics@0.2.1|solid1|only",
      "@solid-primitives/analytics@2.0.0-next.2|solid2|floor",
      "@solid-primitives/analytics@2.0.0-next.2|solid2|head",
      "@solid-primitives/async@0.0.101-next.3|solid2|floor",
      "@solid-primitives/async@0.0.101-next.3|solid2|head",
      "@solid-primitives/audio@1.4.5|solid1|only",
      "@solid-primitives/audio@3.0.0-next.2|solid2|floor",
      "@solid-primitives/audio@3.0.0-next.2|solid2|head",
      "@solid-primitives/autofocus@0.1.5|solid1|only",
      "@solid-primitives/bounds@0.1.7|solid1|only",
      "@solid-primitives/bounds@1.0.0-next.2|solid2|floor",
      "@solid-primitives/bounds@1.0.0-next.2|solid2|head",
      "@solid-primitives/broadcast-channel@0.1.1|solid1|only",
      "@solid-primitives/broadcast-channel@1.0.0-next.2|solid2|floor",
      "@solid-primitives/broadcast-channel@1.0.0-next.2|solid2|head",
      "@solid-primitives/clipboard@1.6.6|solid1|only",
      "@solid-primitives/clipboard@2.0.0-next.17|solid2|floor",
      "@solid-primitives/clipboard@2.0.0-next.17|solid2|head",
      "@solid-primitives/connectivity@0.4.6|solid1|only",
      "@solid-primitives/connectivity@1.0.0-next.2|solid2|floor",
      "@solid-primitives/connectivity@1.0.0-next.2|solid2|head",
      "@solid-primitives/context@2.0.0-next.2|solid2|floor",
      "@solid-primitives/context@2.0.0-next.2|solid2|head",
      "@solid-primitives/controlled-props@0.1.4|solid1|only",
      "@solid-primitives/controlled-signal@1.0.0-next.3|solid2|floor",
      "@solid-primitives/controlled-signal@1.0.0-next.3|solid2|head",
      "@solid-primitives/cookies@0.0.3|solid1|only",
      "@solid-primitives/cookies@1.0.0-next.2|solid2|floor",
      "@solid-primitives/cookies@1.0.0-next.2|solid2|head",
      "@solid-primitives/cookies-store@1.1.11|solid1|only",
      "@solid-primitives/countdown@1.0.9|solid1|only",
      "@solid-primitives/cursor@0.1.4|solid1|only",
      "@solid-primitives/cursor@1.0.0-next.2|solid2|floor",
      "@solid-primitives/cursor@1.0.0-next.2|solid2|head",
      "@solid-primitives/date@2.1.8|solid1|only",
      "@solid-primitives/date@3.0.0-next.3|solid2|floor",
      "@solid-primitives/date@3.0.0-next.3|solid2|head",
      "@solid-primitives/date-difference@1.0.2|solid1|only",
      "@solid-primitives/debounce@1.3.0|solid1|only",
      "@solid-primitives/deep@0.3.7|solid1|only",
      "@solid-primitives/deep@1.0.0-next.3|solid2|floor",
      "@solid-primitives/deep@1.0.0-next.3|solid2|head",
      "@solid-primitives/destructure@0.2.4|solid1|only",
      "@solid-primitives/destructure@1.0.0-next.2|solid2|floor",
      "@solid-primitives/destructure@1.0.0-next.2|solid2|head",
      "@solid-primitives/devices@1.3.1|solid1|only",
      "@solid-primitives/devices@3.0.0-next.2|solid2|floor",
      "@solid-primitives/devices@3.0.0-next.2|solid2|head",
      "@solid-primitives/drag-drop@0.1.0-next.0|solid2|floor",
      "@solid-primitives/drag-drop@0.1.0-next.0|solid2|head",
      "@solid-primitives/event-bus@1.1.4|solid1|only",
      "@solid-primitives/event-bus@3.0.0-next.3|solid2|floor",
      "@solid-primitives/event-bus@3.0.0-next.3|solid2|head",
      "@solid-primitives/event-dispatcher@0.1.1|solid1|only",
      "@solid-primitives/event-dispatcher@1.0.0-next.2|solid2|floor",
      "@solid-primitives/event-dispatcher@1.0.0-next.2|solid2|head",
      "@solid-primitives/event-listener@2.4.6|solid1|only",
      "@solid-primitives/event-listener@3.0.0-next.3|solid2|floor",
      "@solid-primitives/event-listener@3.0.0-next.3|solid2|head",
      "@solid-primitives/event-props@0.3.1|solid1|only",
      "@solid-primitives/event-props@1.0.0-next.2|solid2|floor",
      "@solid-primitives/event-props@1.0.0-next.2|solid2|head",
      "@solid-primitives/fetch@2.5.2|solid1|only",
      "@solid-primitives/filesystem@1.3.4|solid1|only",
      "@solid-primitives/filesystem@3.0.0-next.3|solid2|floor",
      "@solid-primitives/filesystem@3.0.0-next.3|solid2|head",
      "@solid-primitives/flux-store@0.1.1|solid1|only",
      "@solid-primitives/focus@1.0.0-next.4|solid2|floor",
      "@solid-primitives/focus@1.0.0-next.4|solid2|head",
      "@solid-primitives/form@1.0.0-next.2|solid2|floor",
      "@solid-primitives/form@1.0.0-next.2|solid2|head",
      "@solid-primitives/fullscreen@1.3.5|solid1|only",
      "@solid-primitives/fullscreen@2.0.0-next.3|solid2|floor",
      "@solid-primitives/fullscreen@2.0.0-next.3|solid2|head",
      "@solid-primitives/geolocation@1.5.5|solid1|only",
      "@solid-primitives/geolocation@3.0.0-next.2|solid2|floor",
      "@solid-primitives/geolocation@3.0.0-next.2|solid2|head",
      "@solid-primitives/gestures@3.0.0-next.3|solid2|floor",
      "@solid-primitives/gestures@3.0.0-next.3|solid2|head",
      "@solid-primitives/graphql@3.0.0-next.0|solid1|only",
      "@solid-primitives/history@0.2.5|solid1|only",
      "@solid-primitives/history@1.0.0-next.3|solid2|floor",
      "@solid-primitives/history@1.0.0-next.3|solid2|head",
      "@solid-primitives/idle@0.2.3|solid1|only",
      "@solid-primitives/idle@1.0.0-next.3|solid2|floor",
      "@solid-primitives/idle@1.0.0-next.3|solid2|head",
      "@solid-primitives/immutable@2.0.0-next.0|solid1|only",
      "@solid-primitives/input-mask@0.3.1|solid1|only",
      "@solid-primitives/interaction@1.0.0-next.4|solid2|floor",
      "@solid-primitives/interaction@1.0.0-next.4|solid2|head",
      "@solid-primitives/intersection-observer@2.2.5|solid1|only",
      "@solid-primitives/jsx-parser@0.2.0|solid1|only",
      "@solid-primitives/jsx-tokenizer@1.1.4|solid1|only",
      "@solid-primitives/jsx-tokenizer@3.0.0-next.2|solid2|floor",
      "@solid-primitives/jsx-tokenizer@3.0.0-next.2|solid2|head",
      "@solid-primitives/keyboard@1.3.7|solid1|only",
      "@solid-primitives/keyboard@2.0.0-next.5|solid2|floor",
      "@solid-primitives/keyboard@2.0.0-next.5|solid2|head",
      "@solid-primitives/lifecycle@0.1.2|solid1|only",
      "@solid-primitives/lifecycle@1.0.0-next.2|solid2|floor",
      "@solid-primitives/lifecycle@1.0.0-next.2|solid2|head",
      "@solid-primitives/list@0.1.2|solid1|only",
      "@solid-primitives/list@1.0.0-next.2|solid2|floor",
      "@solid-primitives/list@1.0.0-next.2|solid2|head",
      "@solid-primitives/list-state@1.0.0-next.2|solid2|floor",
      "@solid-primitives/list-state@1.0.0-next.2|solid2|head",
      "@solid-primitives/map@0.7.4|solid1|only",
      "@solid-primitives/map@1.0.0-next.2|solid2|floor",
      "@solid-primitives/map@1.0.0-next.2|solid2|head",
      "@solid-primitives/masonry@0.1.4|solid1|only",
      "@solid-primitives/masonry@2.0.0-next.2|solid2|floor",
      "@solid-primitives/masonry@2.0.0-next.2|solid2|head",
      "@solid-primitives/match@0.0.100|solid1|only",
      "@solid-primitives/match@1.0.0-next.2|solid2|floor",
      "@solid-primitives/match@1.0.0-next.2|solid2|head",
      "@solid-primitives/media@2.3.6|solid1|only",
      "@solid-primitives/media@4.0.0-next.2|solid2|floor",
      "@solid-primitives/media@4.0.0-next.2|solid2|head",
      "@solid-primitives/mediastream@1.0.0-next.2|solid2|floor",
      "@solid-primitives/mediastream@1.0.0-next.2|solid2|head",
      "@solid-primitives/memo@1.5.1|solid1|only",
      "@solid-primitives/memo@2.0.0-next.2|solid2|floor",
      "@solid-primitives/memo@2.0.0-next.2|solid2|head",
      "@solid-primitives/mouse@2.1.7|solid1|only",
      "@solid-primitives/mouse@4.0.0-next.3|solid2|floor",
      "@solid-primitives/mouse@4.0.0-next.3|solid2|head",
      "@solid-primitives/mutable@1.1.1|solid1|only",
      "@solid-primitives/mutable@3.0.0-next.2|solid2|floor",
      "@solid-primitives/mutable@3.0.0-next.2|solid2|head",
      "@solid-primitives/mutation-observer@1.2.4|solid1|only",
      "@solid-primitives/mutation-observer@3.0.0-next.2|solid2|floor",
      "@solid-primitives/mutation-observer@3.0.0-next.2|solid2|head",
      "@solid-primitives/notification@1.0.0-next.3|solid2|floor",
      "@solid-primitives/notification@1.0.0-next.3|solid2|head",
      "@solid-primitives/orientation@1.0.0-next.2|solid2|floor",
      "@solid-primitives/orientation@1.0.0-next.2|solid2|head",
      "@solid-primitives/page-utilities@3.0.0-next.2|solid2|floor",
      "@solid-primitives/page-utilities@3.0.0-next.2|solid2|head",
      "@solid-primitives/page-visibility@2.1.6|solid1|only",
      "@solid-primitives/pagination@0.5.2|solid1|only",
      "@solid-primitives/pagination@1.0.0-next.6|solid2|floor",
      "@solid-primitives/pagination@1.0.0-next.6|solid2|head",
      "@solid-primitives/permission@1.3.2|solid1|only",
      "@solid-primitives/permission@2.0.0-next.2|solid2|floor",
      "@solid-primitives/permission@2.0.0-next.2|solid2|head",
      "@solid-primitives/platform@0.2.1|solid1|only",
      "@solid-primitives/platform@1.0.0-next.2|solid2|floor",
      "@solid-primitives/platform@1.0.0-next.2|solid2|head",
      "@solid-primitives/pointer@0.3.6|solid1|only",
      "@solid-primitives/pointer@1.0.0-next.2|solid2|floor",
      "@solid-primitives/pointer@1.0.0-next.2|solid2|head",
      "@solid-primitives/presence@0.1.4|solid1|only",
      "@solid-primitives/presence@1.0.0-next.2|solid2|floor",
      "@solid-primitives/presence@1.0.0-next.2|solid2|head",
      "@solid-primitives/promise@1.1.4|solid1|only",
      "@solid-primitives/promise@2.0.0-next.2|solid2|floor",
      "@solid-primitives/promise@2.0.0-next.2|solid2|head",
      "@solid-primitives/props@3.2.4|solid1|only",
      "@solid-primitives/props@4.0.0-next.3|solid2|floor",
      "@solid-primitives/props@4.0.0-next.3|solid2|head",
      "@solid-primitives/queue@1.0.0-next.3|solid2|floor",
      "@solid-primitives/queue@1.0.0-next.3|solid2|head",
      "@solid-primitives/raf@2.3.5|solid1|only",
      "@solid-primitives/raf@4.0.0-next.2|solid2|floor",
      "@solid-primitives/raf@4.0.0-next.2|solid2|head",
      "@solid-primitives/range@0.2.5|solid1|only",
      "@solid-primitives/range@1.0.0-next.3|solid2|floor",
      "@solid-primitives/range@1.0.0-next.3|solid2|head",
      "@solid-primitives/reducer@0.0.101|solid1|only",
      "@solid-primitives/refs@1.1.4|solid1|only",
      "@solid-primitives/refs@3.0.0-next.2|solid2|floor",
      "@solid-primitives/refs@3.0.0-next.2|solid2|head",
      "@solid-primitives/resize-observer@2.2.0|solid1|only",
      "@solid-primitives/resize-observer@4.0.0-next.3|solid2|floor",
      "@solid-primitives/resize-observer@4.0.0-next.3|solid2|head",
      "@solid-primitives/resource@0.4.3|solid1|only",
      "@solid-primitives/rootless@1.5.4|solid1|only",
      "@solid-primitives/rootless@2.0.0-next.2|solid2|floor",
      "@solid-primitives/rootless@2.0.0-next.2|solid2|head",
      "@solid-primitives/scheduled@1.5.3|solid1|only",
      "@solid-primitives/scheduled@2.0.0-next.2|solid2|floor",
      "@solid-primitives/scheduled@2.0.0-next.2|solid2|head",
      "@solid-primitives/script-loader@2.3.2|solid1|only",
      "@solid-primitives/script-loader@3.0.0-next.2|solid2|floor",
      "@solid-primitives/script-loader@3.0.0-next.2|solid2|head",
      "@solid-primitives/scroll@2.1.6|solid1|only",
      "@solid-primitives/scroll@3.0.0-next.4|solid2|floor",
      "@solid-primitives/scroll@3.0.0-next.4|solid2|head",
      "@solid-primitives/selection@0.1.3|solid1|only",
      "@solid-primitives/selection@1.0.0-next.2|solid2|floor",
      "@solid-primitives/selection@1.0.0-next.2|solid2|head",
      "@solid-primitives/sensors@1.0.0-next.3|solid2|floor",
      "@solid-primitives/sensors@1.0.0-next.3|solid2|head",
      "@solid-primitives/set@0.7.4|solid1|only",
      "@solid-primitives/set@1.0.0-next.2|solid2|floor",
      "@solid-primitives/set@1.0.0-next.2|solid2|head",
      "@solid-primitives/share@2.2.5|solid1|only",
      "@solid-primitives/share@4.0.0-next.4|solid2|floor",
      "@solid-primitives/share@4.0.0-next.4|solid2|head",
      "@solid-primitives/signal-builders@0.2.4|solid1|only",
      "@solid-primitives/signal-builders@1.0.0-next.4|solid2|floor",
      "@solid-primitives/signal-builders@1.0.0-next.4|solid2|head",
      "@solid-primitives/sortable@1.0.0-next.0|solid2|floor",
      "@solid-primitives/sortable@1.0.0-next.0|solid2|head",
      "@solid-primitives/spring@1.0.0-next.3|solid2|floor",
      "@solid-primitives/spring@1.0.0-next.3|solid2|head",
      "@solid-primitives/sse@0.0.103|solid1|only",
      "@solid-primitives/sse@1.0.0-next.2|solid2|floor",
      "@solid-primitives/sse@1.0.0-next.2|solid2|head",
      "@solid-primitives/start@0.0.4|solid1|only",
      "@solid-primitives/state-machine@0.1.1|solid1|only",
      "@solid-primitives/state-machine@1.0.0-next.2|solid2|floor",
      "@solid-primitives/state-machine@1.0.0-next.2|solid2|head",
      "@solid-primitives/static-store@0.1.4|solid1|only",
      "@solid-primitives/static-store@1.0.0-next.2|solid2|floor",
      "@solid-primitives/static-store@1.0.0-next.2|solid2|head",
      "@solid-primitives/storage@4.4.0|solid1|only",
      "@solid-primitives/storage@5.0.0-next.4|solid2|floor",
      "@solid-primitives/storage@5.0.0-next.4|solid2|head",
      "@solid-primitives/stream@0.7.4|solid1|only",
      "@solid-primitives/styles@0.1.4|solid1|only",
      "@solid-primitives/styles@1.0.0-next.2|solid2|floor",
      "@solid-primitives/styles@1.0.0-next.2|solid2|head",
      "@solid-primitives/throttle@1.2.0|solid1|only",
      "@solid-primitives/timer@1.4.4|solid1|only",
      "@solid-primitives/transition-group@1.1.2|solid1|only",
      "@solid-primitives/transition-group@2.0.0-next.2|solid2|floor",
      "@solid-primitives/transition-group@2.0.0-next.2|solid2|head",
      "@solid-primitives/trigger@1.2.4|solid1|only",
      "@solid-primitives/trigger@3.0.0-next.2|solid2|floor",
      "@solid-primitives/trigger@3.0.0-next.2|solid2|head",
      "@solid-primitives/tween@1.4.1|solid1|only",
      "@solid-primitives/tween@2.0.0-next.2|solid2|floor",
      "@solid-primitives/tween@2.0.0-next.2|solid2|head",
      "@solid-primitives/upload@0.1.5|solid1|only",
      "@solid-primitives/upload@1.0.0-next.4|solid2|floor",
      "@solid-primitives/upload@1.0.0-next.4|solid2|head",
      "@solid-primitives/url@0.2.0-next.2|solid2|floor",
      "@solid-primitives/url@0.2.0-next.2|solid2|head",
      "@solid-primitives/utils@6.4.1|solid1|only",
      "@solid-primitives/vibrate@1.0.0-next.2|solid2|floor",
      "@solid-primitives/vibrate@1.0.0-next.2|solid2|head",
      "@solid-primitives/video@1.0.0-next.3|solid2|floor",
      "@solid-primitives/video@1.0.0-next.3|solid2|head",
      "@solid-primitives/virtual@0.2.5|solid1|only",
      "@solid-primitives/visibility-observer@2.0.1|solid1|only",
      "@solid-primitives/websocket@1.4.0|solid1|only",
      "@solid-primitives/websocket@2.0.0-next.3|solid2|floor",
      "@solid-primitives/websocket@2.0.0-next.3|solid2|head",
      "@solid-primitives/workers@2.0.1-next.1|solid2|floor",
      "@solid-primitives/workers@2.0.1-next.1|solid2|head",
      "@solidjs/html@2.0.0-rc.3|solid2|only",
      "@solidjs/meta@0.29.4|solid1|only",
      "@solidjs/meta@1.0.0-next.2|solid2|floor",
      "@solidjs/meta@1.0.0-next.2|solid2|head",
      "@solidjs/router@1.0.0|solid1|only",
      "@solidjs/start@2.0.3|solid1|only",
      "@solidjs/start-devtools@1.0.0-next.4|solid2|floor",
      "@solidjs/start-devtools@1.0.0-next.4|solid2|head",
      "@tanstack/ai-devtools-core@0.5.8|solid1|only",
      "@tanstack/charts@0.15.0|solid1|only",
      "@tanstack/devtools@0.14.2|solid1|only",
      "@tanstack/devtools-a11y@0.2.2|solid1|only",
      "@tanstack/devtools-ui@0.7.1|solid1|only",
      "@tanstack/devtools-utils@0.7.0|solid1|only",
      "@tanstack/form-devtools@1.0.0-alpha.2|solid1|only",
      "@tanstack/hotkeys-devtools@0.9.0|solid1|only",
      "@tanstack/pacer-devtools@1.4.0|solid1|only",
      "@tanstack/solid-ai-devtools@0.2.71|solid1|only",
      "@tanstack/solid-charts@0.15.0|solid1|only",
      "@tanstack/solid-devtools@0.8.12|solid1|only",
      "@tanstack/solid-form-devtools@1.0.0-alpha.2|solid1|only",
      "@tanstack/solid-hotkeys@0.10.0|solid1|only",
      "@tanstack/solid-hotkeys-devtools@0.7.0|solid1|only",
      "@tanstack/solid-pacer@0.22.0|solid1|only",
      "@tanstack/solid-pacer-devtools@0.14.0|solid1|only",
      "@tanstack/solid-query-devtools@5.102.5|solid1|only",
      "@tanstack/solid-query-devtools@6.0.0-rc.0|solid2|floor",
      "@tanstack/solid-query-devtools@6.0.0-rc.0|solid2|head",
      "@tanstack/solid-router@1.170.30|solid1|only",
      "@tanstack/solid-router@2.0.0-rc.2|solid2|floor",
      "@tanstack/solid-router@2.0.0-rc.2|solid2|head",
      "@tanstack/solid-router-devtools@1.167.1|solid1|only",
      "@tanstack/solid-router-devtools@2.0.0-rc.2|solid2|floor",
      "@tanstack/solid-router-devtools@2.0.0-rc.2|solid2|head",
      "@tanstack/solid-router-ssr-query@1.167.2-pre.0|solid1|only",
      "@tanstack/solid-router-ssr-query@2.0.0-rc.2|solid2|floor",
      "@tanstack/solid-router-ssr-query@2.0.0-rc.2|solid2|head",
      "@tanstack/solid-start@1.168.47|solid1|only",
      "@tanstack/solid-start@2.0.0-rc.2|solid2|floor",
      "@tanstack/solid-start@2.0.0-rc.2|solid2|head",
      "@tanstack/solid-start-client@1.168.29|solid1|only",
      "@tanstack/solid-start-client@2.0.0-rc.2|solid2|floor",
      "@tanstack/solid-start-client@2.0.0-rc.2|solid2|head",
      "@tanstack/solid-start-config@1.120.20|solid1|only",
      "@tanstack/solid-table@9.1.2|solid1|only",
      "@tanstack/solid-table-devtools@9.2.0|solid1|only",
      "@tanstack/solid-virtual@3.13.37|solid1|only",
      "@tanstack/table-devtools@9.2.0|solid1|only",
      "corvu@0.7.2|solid1|only",
      "motion-solidjs@0.7.0-beta.4|solid2|floor",
      "motion-solidjs@0.7.0-beta.4|solid2|head",
      "solid-js@2.0.0-rc.3|solid2|only",
      "solid-recharts@1.0.1|solid1|only",
      "solid-recharts@2.0.0-beta.1|solid2|floor",
      "solid-recharts@2.0.0-beta.1|solid2|head"
    ]
  );
  assert.equal(ledger.summary.incompleteRefusalCensusRows, 0);
  assert.equal(ledger.summary.incompleteAcceptedCaseIdentityRows, 0);
  assert.equal(ledger.summary.externalEdgesWithoutResolvedVersion, 1);
  assert.equal(ledger.summary.dependencyPlannedRows, 51);
  // Two rows, both an explicit `resource-refusal` at the planner's maxNodes=512
  // budget rather than a plan that silently stopped short:
  // `@tanstack/ai-solid-ui@0.7.20|solid1|only`, and — newly —
  // `@kobalte/solidbase@0.6.13|solid1|only`, whose 94 `./default-theme/*`
  // asset entrypoints are now recorded `non-module-target` inapplicable
  // instead of refused, leaving 66 accepted cases as plan roots and a graph
  // that exceeds the node budget.
  assert.equal(ledger.summary.incompleteDependencyPlanRows, 2);
  // The `@solid-primitives/source` case (a private namespaced condition
  // selecting an unpublished `src/index.ts`) is now recorded
  // `unpublished-conditional-target` inapplicable rather than refused, so the
  // row's only remaining case is accepted: the proposal is complete, the row
  // certifies, and its sole blocker membership is certification authority.
  const geolocation = ledger.rows.find(
    row => row.probeId === "@solid-primitives/geolocation@1.5.5|solid1|only"
  );
  assert.equal(geolocation.measurement.observedClass, "success");
  assert.equal(geolocation.measurement.classifierCorrected, false);
  assert.deepEqual(geolocation.nextOwner, { slice: 6, blocker: "certification-authority" });
});

test("mixed applicability is derived from cases while blocker memberships remain overlapping", () => {
  const result = {
    schemaVersion: 1,
    finishedAt: "2026-08-30T00:00:00.000Z",
    results: [{
      probeId: "pkg@1.0.0|solid1|only",
      status: "official",
      package: "pkg",
      version: "1.0.0",
      family: "fixture",
      solidTarget: "solid1",
      probeKind: "only",
      outcome: "partial-success",
      class: "partial-success",
      detail: {},
      exitStatus: 0,
      timedOut: false,
      stdout: "generated unaccepted stable contract proposal for pkg@1.0.0 at /tmp/p; 1 artifact case(s) refused and omitted; proof verification must issue its receipt",
      stderr: "",
      installedVersions: {},
      contractContent: {
        artifactCasesTotal: 1,
        artifactCases: [{ entrypoint: ".", caseIndex: 0, artifact: { path: "./index.js" } }],
        artifactCaseRefusals: [{
          entrypoint: ".",
          conditions: ["source"],
          stage: "artifact-case",
          applicability: "unsupported-condition-environment",
          reason: "resolved target <package-root>/src/index.ts is not a file"
        }]
      }
    }]
  };
  const [row] = buildPhase20Ledger(result).rows;
  assert.equal(row.applicability.aggregate, "mixed");
  assert.deepEqual(row.applicability.counts, {
    "runtime-module": 1,
    "unsupported-condition-environment": 1
  });
  assert.ok(row.blockerMemberships.includes("artifact-applicability"));
  assert.ok(!row.blockerMemberships.includes("upstream-artifact-or-manual-triage"));
});
