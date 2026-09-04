import assert from "node:assert/strict";
import { describe, test } from "vitest";

import {
  parseArguments,
  rankDialectAuditYield,
  renderRanking,
  reportRows
} from "./dialect-audit-yield.mjs";

function row({
  probeId,
  measured = true,
  blockers = [],
  declinedClosures = 0,
  byKind = {}
}) {
  return {
    probeId,
    package: probeId?.split("@")[0] ?? "",
    contractContent: measured
      ? {
          measured: true,
          declinedClosures,
          declinedClosuresByKind: byKind,
          dialectSilentBlockers: blockers
        }
      : { measured: false, note: "unparsable" }
  };
}

function synthesizedReport() {
  return {
    schemaVersion: 1,
    results: [
      row({
        probeId: "a@1.0.0|solid2|only",
        declinedClosures: 7,
        byKind: { "dialect-silent": 5, "unresolved-callee": 2 },
        blockers: [
          { package: "solid-js", export: "createEffect", blockedExports: 4 },
          { package: "@solidjs/signals", export: "createContext", blockedExports: 1 }
        ]
      }),
      row({
        probeId: "b@2.0.0|solid2|floor",
        declinedClosures: 3,
        byKind: { "dialect-silent": 3 },
        blockers: [{ package: "solid-js", export: "createEffect", blockedExports: 3 }]
      }),
      row({
        probeId: "c@1.0.0|solid1|only",
        declinedClosures: 2,
        byKind: { "dialect-silent": 1, "refusing-callee-fixpoint": 1 },
        // An unresolved package is kept as its own identity: it is a different
        // audit question from the same spelling in a named archive.
        blockers: [{ package: "", export: "createRenderEffect", blockedExports: 1 }]
      }),
      // A row whose contract could not be measured contributes nothing and is
      // not counted as measured.
      row({ probeId: "d@1.0.0|solid2|head", measured: false }),
      // A report written before the decline records existed: measured, but
      // with no `dialectSilentBlockers` array at all.
      {
        probeId: "e@1.0.0|solid2|only",
        package: "e",
        contractContent: { measured: true, exportsTotal: 3 }
      }
    ],
    supplemental: {
      probeCount: 1,
      results: [
        row({
          probeId: "fork@0.1.0|solid2|only",
          declinedClosures: 1,
          byKind: { "dialect-silent": 1 },
          blockers: [{ package: "solid-js", export: "createEffect", blockedExports: 1 }]
        })
      ]
    }
  };
}

describe("dialect audit yield ranking", () => {
  test("collects rows from the corpus and the supplemental section", () => {
    const rows = reportRows(synthesizedReport());
    assert.equal(rows.length, 6);
    assert.equal(rows.filter(entry => entry.supplemental).length, 1);
    assert.equal(reportRows(synthesizedReport(), { includeSupplemental: false }).length, 5);
    // A report with neither section is not an error; it ranks nothing.
    assert.deepEqual(reportRows({}), []);
    assert.deepEqual(reportRows(null), []);
  });

  test("ranks by distinct consumer exports blocked, keeping the row count beside it", () => {
    const result = rankDialectAuditYield(synthesizedReport());
    assert.deepEqual(
      result.ranking.map(entry => [entry.package, entry.export, entry.blockedExports, entry.rows]),
      [
        ["solid-js", "createEffect", 8, 3],
        ["", "createRenderEffect", 1, 1],
        ["@solidjs/signals", "createContext", 1, 1]
      ]
    );
    assert.equal(result.ranking[0].supplementalRows, 1);
    assert.deepEqual(result.ranking[0].probeIds, [
      "a@1.0.0|solid2|only",
      "b@2.0.0|solid2|floor",
      "fork@0.1.0|solid2|only"
    ]);
    assert.equal(result.blockedExportsTotal, 10);
  });

  test("names the rows that carry no records instead of counting them as zero", () => {
    const result = rankDialectAuditYield(synthesizedReport());
    assert.equal(result.rowsInReport, 6);
    // `d` is unmeasured (not counted); `e` is measured with no records.
    assert.equal(result.rowsMeasured, 5);
    assert.equal(result.rowsWithoutRecords, 1);
    assert.equal(result.rowsWithBlockers, 4);
  });

  test("aggregates the decline kinds, so the silent primitives are read next to the rest", () => {
    const result = rankDialectAuditYield(synthesizedReport());
    assert.equal(result.declinedClosures, 13);
    assert.deepEqual(result.declinedClosuresByKind, {
      "dialect-silent": 10,
      "refusing-callee-fixpoint": 1,
      "unresolved-callee": 2
    });
  });

  test("dropping the supplemental rows drops only their contribution", () => {
    const result = rankDialectAuditYield(synthesizedReport(), { includeSupplemental: false });
    assert.deepEqual(
      result.ranking.map(entry => [entry.export, entry.blockedExports, entry.rows]),
      [
        ["createEffect", 7, 2],
        ["createRenderEffect", 1, 1],
        ["createContext", 1, 1]
      ]
    );
  });

  test("renders a table with the unresolved package named as such", () => {
    const rendered = renderRanking(rankDialectAuditYield(synthesizedReport()));
    assert.match(rendered, /5 measured row\(s\) of 6/);
    assert.match(rendered, /1 carry no decline records at all/);
    assert.match(rendered, /13 declined closure proposal\(s\)/);
    assert.match(rendered, /solid-js\s+createEffect\s+8\s+3/);
    assert.match(rendered, /\(unresolved\)\s+createRenderEffect/);
    assert.match(rendered, /10 consumer export\(s\) blocked in total/);
  });

  test("an empty ranking says so rather than printing an empty table", () => {
    const rendered = renderRanking(rankDialectAuditYield({ results: [] }));
    assert.match(rendered, /No dialect-silent blocker recorded/);
  });

  test("--limit truncates the table and says how much it dropped", () => {
    const rendered = renderRanking(rankDialectAuditYield(synthesizedReport()), { limit: 1 });
    assert.match(rendered, /createEffect/);
    assert.doesNotMatch(rendered, /createContext/);
    assert.match(rendered, /\.\.\. 2 more/);
  });

  test("arguments are parsed exactly, and an unknown one refuses", () => {
    assert.deepEqual(parseArguments([]), {
      report: "benchmarks/ecosystem/report.json",
      json: false,
      limit: Number.POSITIVE_INFINITY,
      includeSupplemental: true
    });
    const parsed = parseArguments(["--json", "--report", "/tmp/r.json", "--limit", "5", "--official"]);
    assert.equal(parsed.json, true);
    assert.equal(parsed.report, "/tmp/r.json");
    assert.equal(parsed.limit, 5);
    assert.equal(parsed.includeSupplemental, false);
    assert.throws(() => parseArguments(["--nope"]), /unknown argument/);
    assert.throws(() => parseArguments(["--report"]), /--report needs a path/);
  });
});
