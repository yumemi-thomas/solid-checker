import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "vitest";

import { findSessions, parseArguments, recordEnvironment, replayArguments } from "./typefacts-differential.mjs";
import {
  absenceFor,
  applyAllowList,
  buildReport,
  cellsOf,
  compareCells,
  diffTranscripts,
  failureReason,
  globMatches,
  normalizeTranscript,
  parseAllowList,
  summarize,
  summarizeCoverage
} from "./lib/typefacts-differential.mjs";

const UNAVAILABLE =
  "type facts backend: async_functions_at is not available in the tsrs backend yet (migration slice S4)";
const handshake = { buildId: "dev", protocol: 81, schemaHash: "sha256:x" };
const loc = (start, end, path = "/p/App.tsx") => ({ path, startByte: start, endByte: end });

const transcript = (...exchanges) => ({
  handshake,
  exchanges: exchanges.map((response, index) => ({
    index,
    operation: response.__op ?? "analyze",
    requestId: index + 1,
    generation: 1,
    response: (({ __op, ...rest }) => rest)(response)
  }))
});

const table = (overrides = {}) => ({
  schema: 22,
  sources: [{ path: "/p/App.tsx", sha256: "sha256:aa" }],
  files: [{ path: "/p/App.tsx" }],
  entities: [],
  symbols: [],
  ...overrides
});

const analyze = (tableValue, extra = {}) => ({ ok: true, generation: 1, stateToken: "tok", table: tableValue, ...extra });

const categories = (results) => {
  const counts = {};
  for (const result of results) counts[result.category] = (counts[result.category] ?? 0) + 1;
  return counts;
};

test("bundled:/// and bundled:/ spell the same library path", () => {
  const go = normalizeTranscript(
    transcript({ ok: true, generation: 1, modules: [{ path: "/p/App.tsx" }], __op: "modules" })
  );
  const a = transcript({
    ok: true,
    generation: 1,
    __op: "symbols",
    symbolEvidence: [{ id: "symbol:h:a", declarations: [{ name: "x", kind: "var", location: loc(1, 2, "bundled:///libs/lib.es5.d.ts") }] }]
  });
  const b = transcript({
    ok: true,
    generation: 1,
    __op: "symbols",
    symbolEvidence: [{ id: "symbol:h:b", declarations: [{ name: "x", kind: "var", location: loc(9, 9, "bundled:/libs/lib.es5.d.ts") }] }]
  });
  // lib-only symbols vanish in both, whatever their id, spelling or offsets
  assert.deepEqual(normalizeTranscript(a).exchanges[0].response.symbolEvidence, []);
  assert.deepEqual(normalizeTranscript(b).exchanges[0].response.symbolEvidence, []);
  assert.equal(go.exchanges.length, 1);
});

test("lib modules are dropped, project modules are kept", () => {
  const normalized = normalizeTranscript(
    transcript({
      ok: true,
      generation: 1,
      __op: "modules",
      modules: [{ path: "/p/App.tsx" }, { path: "bundled:///libs/lib.dom.d.ts" }, { path: "bundled:/libs/lib.es5.d.ts" }]
    })
  );
  assert.deepEqual(normalized.exchanges[0].response.modules, [{ path: "/p/App.tsx" }]);
});

test("a mixed symbol keeps its project declarations and loses the library ones", () => {
  const normalized = normalizeTranscript(
    transcript(
      analyze(
        table({
          symbols: [
            {
              id: "symbol:h:m",
              declarations: [
                { name: "Array", kind: "interface", location: loc(1, 5, "bundled:/libs/lib.es5.d.ts") },
                { name: "Array", kind: "interface", location: loc(10, 15) }
              ]
            }
          ]
        })
      )
    )
  );
  const [symbol] = normalized.exchanges[0].response.table.symbols;
  assert.equal(symbol.declarations.length, 1);
  assert.deepEqual(symbol.declarations[0].location, loc(10, 15));
});

test("an entity that references a library symbol compares by its name", () => {
  const side = (id) =>
    normalizeTranscript(
      transcript(
        analyze(
          table({
            entities: [{ location: loc(3, 4), symbol: id }],
            symbols: [{ id, declarations: [{ name: "Promise", kind: "interface", location: loc(1, 2, "bundled:/libs/lib.es5.d.ts") }] }]
          })
        )
      )
    );
  const results = diffTranscripts(side("symbol:h:go"), side("symbol:h:rust"));
  assert.deepEqual(categories(results), { equal: results.length });
});

test("non-durable ids are renamed from content, whatever order a producer visited them in", () => {
  const symbols = (first, second) => [
    { id: first, declarations: [{ name: "a", kind: "var", location: loc(10, 11) }] },
    { id: second, declarations: [{ name: "b", kind: "var", location: loc(20, 21) }] }
  ];
  const entities = (a, b) => [
    { location: loc(30, 31), symbol: a },
    { location: loc(40, 41), symbol: b },
    { location: loc(50, 51), symbol: a }
  ];
  const go = normalizeTranscript(
    transcript(analyze(table({ symbols: symbols("symbol:1:1", "symbol:1:2"), entities: entities("symbol:1:1", "symbol:1:2") })))
  );
  const rust = normalizeTranscript(
    transcript(analyze(table({ symbols: symbols("symbol:1:9", "symbol:1:3"), entities: entities("symbol:1:9", "symbol:1:3") })))
  );
  const results = diffTranscripts(go, rust);
  assert.deepEqual(categories(results), { equal: results.length });
});

test("a non-durable id that two entities share but a producer splits is a difference", () => {
  const go = normalizeTranscript(
    transcript(analyze(table({ entities: [{ location: loc(1, 2), symbol: "symbol:1:1" }, { location: loc(3, 4), symbol: "symbol:1:1" }] })))
  );
  const rust = normalizeTranscript(
    transcript(analyze(table({ entities: [{ location: loc(1, 2), symbol: "symbol:1:1" }, { location: loc(3, 4), symbol: "symbol:1:2" }] })))
  );
  const counts = categories(diffTranscripts(go, rust));
  assert.equal(counts["value-differs"], 1);
});

test("state tokens, source arena paths and nothing else are normalised away", () => {
  const go = normalizeTranscript(transcript(analyze(table(), { stateToken: "17" })));
  const rust = normalizeTranscript(transcript(analyze(table(), { stateToken: "abc" })));
  const results = diffTranscripts(go, rust);
  assert.deepEqual(categories(results), { equal: results.length });
  const arena = normalizeTranscript(transcript({ ok: true, generation: 1, __op: "sources", sourceArena: "/tmp/x", sources: [] }));
  assert.equal(arena.exchanges[0].response.sourceArena, undefined);
});

test("a changed column value is reported against that column only", () => {
  const entity = (callability) => ({ location: loc(1, 2), symbol: "symbol:h:s", callability });
  const go = normalizeTranscript(transcript(analyze(table({ entities: [entity("callable")] }))));
  const rust = normalizeTranscript(transcript(analyze(table({ entities: [entity("nonCallable")] }))));
  const results = diffTranscripts(go, rust, { project: "fixtures/x", session: "s0" });
  const different = results.filter((result) => result.category === "value-differs");
  assert.equal(different.length, 1);
  assert.equal(different[0].family, "table.entities");
  assert.equal(different[0].column, "callability");
  assert.equal(different[0].go, "callable");
  assert.equal(different[0].rust, "nonCallable");
  assert.deepEqual(different[0].where, loc(1, 2));
  assert.equal(different[0].project, "fixtures/x");
});

test("rows and columns present on one side only are told apart", () => {
  const go = normalizeTranscript(
    transcript(
      analyze(
        table({
          entities: [
            { location: loc(1, 2), symbol: "symbol:h:a", callability: "callable" },
            { location: loc(5, 6), symbol: "symbol:h:b" }
          ]
        })
      )
    )
  );
  const rust = normalizeTranscript(
    transcript(
      analyze(
        table({
          entities: [
            { location: loc(1, 2), symbol: "symbol:h:a" },
            { location: loc(7, 8), symbol: "symbol:h:c" }
          ]
        })
      )
    )
  );
  const counts = categories(diffTranscripts(go, rust));
  assert.equal(counts["column-missing-in-rust"], 1); // callability on loc(1,2)
  assert.equal(counts["row-missing-in-rust"], 1); // loc(5,6)
  assert.equal(counts["row-extra-in-rust"], 1); // loc(7,8)
});

test("arrays compare in order", () => {
  const file = (calls) => ({ path: "/p/App.tsx", calls });
  const go = normalizeTranscript(transcript(analyze(table({ files: [file([1, 2])] }))));
  const rust = normalizeTranscript(transcript(analyze(table({ files: [file([2, 1])] }))));
  assert.equal(categories(diffTranscripts(go, rust))["value-differs"], 1);
});

test("an unavailable family expands to the cells Go answered and is its own category", () => {
  const go = normalizeTranscript(
    transcript(analyze(table({ entities: [{ location: loc(1, 2), symbol: "symbol:h:a", callability: "callable" }] })))
  );
  const rust = {
    handshake,
    exchanges: [{ index: 0, operation: "analyze", response: { ok: false, generation: 1, error: { code: "analysis-failed", message: UNAVAILABLE } } }]
  };
  const results = diffTranscripts(go, rust);
  assert.ok(results.length > 5);
  assert.ok(results.some((r) => r.family === "table.entities" && r.column === "callability" && r.category === "rust-unavailable"));
  assert.equal(results.filter((r) => r.category === "rust-unavailable").length, results.length - 1); // handshake equal
});

test("any other Rust error is rust-error, except a state error that follows an unavailable answer", () => {
  const goResponse = { ok: true, generation: 1 };
  assert.equal(absenceFor(goResponse, { ok: false, error: { code: "analysis-failed", message: "boom" } }, false).category, "rust-error");
  assert.equal(absenceFor(goResponse, { ok: false, error: { code: "state-mismatch", message: "x" } }, false).category, "rust-error");
  assert.equal(absenceFor(goResponse, { ok: false, error: { code: "state-mismatch", message: "x" } }, true).category, "rust-unavailable");
  assert.equal(absenceFor(goResponse, null, false).category, "rust-error");
  assert.equal(absenceFor(goResponse, { ok: true }, false), null);
});

test("when Go refuses, Rust must refuse the same way", () => {
  const error = { code: "update-failed", message: "no" };
  const go = transcript({ ok: false, generation: 1, error, __op: "update" });
  const same = transcript({ ok: false, generation: 1, error, __op: "update" });
  const other = transcript({ ok: false, generation: 1, error: { ...error, message: "different" }, __op: "update" });
  assert.equal(categories(diffTranscripts(go, same)).equal, 2);
  assert.equal(categories(diffTranscripts(go, other))["value-differs"], 1);
});

test("an exchange Go never answered (a cancel) is skipped", () => {
  const go = transcript({ __op: "cancel" });
  go.exchanges[0].response = null;
  assert.deepEqual(categories(diffTranscripts(go, transcript({ __op: "cancel" }))), { equal: 1 });
});

test("update affected sets compare as sets of rows", () => {
  const go = transcript({ ok: true, generation: 2, affected: ["/p/a.ts", "/p/b.ts"], __op: "update" });
  const rust = transcript({ ok: true, generation: 2, affected: ["/p/b.ts", "/p/c.ts"], __op: "update" });
  const counts = categories(diffTranscripts(go, rust));
  assert.equal(counts["row-missing-in-rust"], 1);
  assert.equal(counts["row-extra-in-rust"], 1);
});

test("module graph rows key by path and specifier span", () => {
  const response = (resolution) => ({
    ok: true,
    generation: 1,
    __op: "modules",
    modules: [{ path: "/p/App.tsx", format: "esm" }],
    moduleImports: [{ specifier: loc(1, 9), text: "solid-js", resolution }]
  });
  const counts = categories(diffTranscripts(transcript(response("relative")), transcript(response("nodeModules"))));
  assert.equal(counts["value-differs"], 1);
});

test("cells are keyed by location for entities and by id for symbols", () => {
  const cells = cellsOf({
    ok: true,
    generation: 1,
    table: table({ entities: [{ location: loc(1, 2), symbol: "s" }], symbols: [{ id: "symbol:h:q", aliasTarget: "" }] })
  });
  assert.ok(cells.some((c) => c.family === "table.entities" && c.key === "/p/App.tsx:1-2" && c.column === "symbol"));
  assert.ok(cells.some((c) => c.family === "table.symbols" && c.key === "symbol:h:q"));
});

test("compareCells with no Rust cells marks each Go cell with the given category", () => {
  const go = cellsOf({ ok: true, generation: 3 });
  const results = compareCells(go, null, { category: "rust-error" });
  assert.deepEqual(results.map((r) => r.category), ["rust-error", "rust-error"]);
});

// --- allow-list -----------------------------------------------------------------

const entry = (extra = {}) => ({
  id: "lib-version",
  family: "table.entities",
  column: "libraryTypes",
  category: "value-differs",
  justification: "The two producers pin different TypeScript lib versions, so lib type names differ.",
  ...extra
});

test("an allow-list entry needs a justification, known fields and a failing category", () => {
  const parse = (entries) => parseAllowList(JSON.stringify({ version: 1, entries }));
  assert.equal(parse([entry()]).length, 1);
  assert.throws(() => parse([entry({ justification: "" })]), /justification is required/);
  assert.throws(() => parse([entry({ justification: "too short" })]), /justification is required/);
  assert.throws(() => parse([entry({ because: "x" })]), /unknown field/);
  assert.throws(() => parse([entry({ category: "equal" })]), /cannot be allowed/);
  assert.throws(() => parse([entry({ category: "rust-unavailable" })]), /cannot be allowed/);
  assert.throws(() => parse([entry(), entry()]), /duplicate id/);
  assert.throws(() => parseAllowList("{}"), /expected/);
});

test("allow-list entries match narrowly and are counted", () => {
  const go = normalizeTranscript(transcript(analyze(table({ entities: [{ location: loc(1, 2), symbol: "symbol:h:s", libraryTypes: ["Map"] }] }))));
  const rust = normalizeTranscript(transcript(analyze(table({ entities: [{ location: loc(1, 2), symbol: "symbol:h:s", libraryTypes: ["Set"] }] }))));
  const unmatched = diffTranscripts(go, rust, { project: "fixtures/a" });
  const used = applyAllowList(unmatched, [entry({ id: "other-column", column: "callability" }), entry({ project: "fixtures/b" })]);
  assert.equal(categories(unmatched)["value-differs"], 1);
  assert.deepEqual([...used.values()], [0, 0]);

  const matched = diffTranscripts(go, rust, { project: "fixtures/a" });
  const counted = applyAllowList(matched, [entry({ project: "fixtures/*", go: ["Map"], rust: ["Set"] })]);
  assert.equal(categories(matched).allowed, 1);
  assert.equal(counted.get("lib-version"), 1);
  assert.equal(matched.find((r) => r.category === "allowed").wasCategory, "value-differs");
});

test("globs treat only * as a wildcard", () => {
  assert.ok(globMatches("fixtures/*/x", "fixtures/a/x"));
  assert.ok(!globMatches("fixtures/a.b", "fixtures/aXb"));
  assert.ok(!globMatches("fixtures/a", "fixtures/ab"));
});

// --- report ---------------------------------------------------------------------

test("the report counts per family and column and keeps the first examples per category", () => {
  const entities = (callability) => [1, 2, 3, 4, 5].map((n) => ({ location: loc(n, n + 1), symbol: "symbol:h:s", callability }));
  const go = normalizeTranscript(transcript(analyze(table({ entities: entities("callable") }))));
  const rust = normalizeTranscript(transcript(analyze(table({ entities: entities("nonCallable") }))));
  const results = diffTranscripts(go, rust, { project: "fixtures/a", session: "s" });
  const report = buildReport(results, { limit: 2, sessions: 1 });
  assert.equal(report.families["table.entities"].callability["value-differs"], 5);
  assert.equal(report.families["table.entities"].symbol.equal, 5);
  assert.equal(report.examples.filter((e) => e.category === "value-differs").length, 2);
  const example = report.examples.find((e) => e.category === "value-differs");
  assert.equal(example.project, "fixtures/a");
  assert.deepEqual(example.span, [1, 2]);
  assert.equal(example.go, "callable");
  assert.equal(report.failing, 5);
  assert.match(summarize(report), /value-differs\s+5/);
});

test("unavailable cells are not failing, and unused allow-list entries are listed", () => {
  const report = buildReport(
    [{ family: "response", column: "ok", key: "", category: "rust-unavailable", operation: "analyze" }],
    { allowUsed: new Map([["a", 0], ["b", 2]]) }
  );
  assert.equal(report.failing, 0);
  assert.equal(report.unavailable, 1);
  assert.deepEqual(report.allowList.unused, ["a"]);
});

test("end-to-end summary groups checker failures by message", () => {
  assert.equal(
    failureReason({ stderr: "solid-checker: note: x\nsolid-checker-rust: TypeFacts session error: foo at /tmp/a/b 12345\n" }),
    "solid-checker-rust: TypeFacts session error: foo at <path> <n>"
  );
  const summary = summarizeCoverage({
    projects: 3,
    states: { "checker-failed": 2, unchanged: 1 },
    moved: [
      { state: "checker-failed", failure: { stderr: "error: boom /x" } },
      { state: "checker-failed", failure: { stderr: "error: boom /y" } },
      { state: "unchanged" }
    ]
  });
  assert.deepEqual(summary.checkerFailureReasons, { "error: boom <path>": 2 });
});

// --- command line, recording and replay -----------------------------------------

test("arguments split into verb, options and the command after --", () => {
  const parsed = parseArguments(["record", "--out", "d", "--gate", "coverage", "--", "x", "--y"]);
  assert.deepEqual(parsed.positional, ["record"]);
  assert.equal(parsed.options.out, "d");
  assert.deepEqual(parsed.rest, ["x", "--y"]);
  assert.equal(parseArguments(["compare", "d", "--allow-unavailable"]).options["allow-unavailable"], true);
  assert.throws(() => parseArguments(["compare", "--report"]), /needs a value/);
});

test("recording routes the producer through the proxy and disables result caches", () => {
  const env = recordEnvironment({ out: "x", label: "coverage", target: "/bin/go", base: { KEEP: "1" } });
  assert.match(env.SOLID_TYPEFACTS_BIN, /typefacts-transcript-proxy\.mjs$/);
  assert.equal(env.TYPEFACTS_TRANSCRIPT_TARGET, "/bin/go");
  assert.equal(env.SOLID_CHECKER_GATE_CACHE, "0");
  assert.equal(env.SOLID_CHECKER_DAEMON, "0");
  assert.equal(env.KEEP, "1");
});

test("replay argv swaps the producer and the checker's deleted temporary config", () => {
  const argv = ["/bin/go", "-inline-sources", "-project-config=/tmp/gone.json", "-schema=1", "-project", "/p/tsconfig.json"];
  assert.deepEqual(replayArguments(argv, "/bin/rs", "/s", { hasConfigCopy: true }), [
    "/bin/rs",
    "-inline-sources",
    "-project-config=/s/project-config.bin",
    "-schema=1",
    "-project",
    "/p/tsconfig.json"
  ]);
  const spaced = ["/bin/go", "-project-config", "/tmp/gone.json", "-project", "/p/tsconfig.json"];
  assert.deepEqual(replayArguments(spaced, "/bin/rs", "/s", { hasConfigCopy: true }).slice(1, 3), [
    "-project-config",
    "/s/project-config.bin"
  ]);
  assert.deepEqual(replayArguments(argv, "/bin/rs", "/s", { hasConfigCopy: false }).slice(2, 3), [
    "-project-config=/tmp/gone.json"
  ]);
});

test("sessions are found at any depth by their requests.bin", () => {
  const root = mkdtempSync(join(tmpdir(), "differential-"));
  try {
    for (const path of ["coverage/000000", "coverage/000001", "000002"]) {
      mkdirSync(join(root, path), { recursive: true });
      writeFileSync(join(root, path, "requests.bin"), "");
    }
    assert.equal(findSessions(root).length, 3);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
