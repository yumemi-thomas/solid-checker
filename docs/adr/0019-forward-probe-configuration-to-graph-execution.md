# 0019 — Forward the requested probe configuration to graph execution

Status: accepted and implemented; measured outcomes in docs/2026-09-04-published-js-probe-unlock.md
Date: 2026-09-04

The first 12-recipe Kobalte JS graph attempt still reports all 174 graph
closure candidates withheld for no recipe. Exact claim IDs match. Inspection
shows that the ordinary execution path forwards probeHarnessRequest(options),
but executePreparedPublishedGraphs drops it. Native graph certification already
accepts the same compiled-pin-validated harness configuration and performs
recipe gating; the adapter never supplies it.

Forward the same all-or-none harness root, Node path and recipe-corpus path to
both single-graph and deduplicated graph-case-set requests. Carry the corpus
into graph-node planning inputs as well. Missing configuration preserves the
existing unknown closure result; configured missing or mismatched pins must
refuse through existing native validation. Do not construct receipts, declare
sandbox policy or mark gates complete in JavaScript.

This restores the user's explicit recipe request. The failed diagnostic is
not a completed veto and must not be counted as certification of a creates
closure. Native gates, census and sandbox scheme 6 remain unchanged. Test both
request shapes and rerun the exact 12 recipes before promoting any of them.
