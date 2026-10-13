import assert from "node:assert/strict";
import { describe, test } from "vitest";

import {
  parseArguments,
  rankDialectAuditYield,
  renderRanking,
  renderShapeRanking,
  reportRows
} from "./dialect-audit-yield.mjs";

function row({
  probeId,
  measured = true,
  blockers = [],
  declinedClosures = 0,
  byKind = {},
  shapes = null
}) {
  return {
    probeId,
    package: probeId?.split("@")[0] ?? "",
    contractContent: measured
      ? {
          measured: true,
          declinedClosures,
          declinedClosuresByKind: byKind,
          dialectSilentBlockers: blockers,
          // Additive: a row written before the shapes carries no array at all,
          // which is not the same measurement as a row of zero shapes.
          ...(shapes === null ? {} : { unresolvedCalleeShapes: shapes })
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
        ],
        shapes: [
          {
            shape: "parameter-rooted",
            blockedExports: 2,
            records: 9,
            spellings: [
              { spelling: "read", blockedExports: 2, records: 7 },
              { spelling: "write", blockedExports: 1, records: 2 }
            ]
          },
          {
            shape: "undeclared-identifier",
            blockedExports: 1,
            records: 1,
            spellings: [{ spelling: "requestAnimationFrame", blockedExports: 1, records: 1 }]
          }
        ]
      }),
      row({
        probeId: "b@2.0.0|solid2|floor",
        declinedClosures: 3,
        byKind: { "dialect-silent": 3 },
        blockers: [{ package: "solid-js", export: "createEffect", blockedExports: 3 }],
        shapes: [
          {
            shape: "parameter-rooted",
            blockedExports: 3,
            records: 4,
            spellings: [{ spelling: "read", blockedExports: 3, records: 4 }]
          },
          {
            // A shape whose spelling is deliberately empty -- a computed member
            // whose receiver is not a plain identifier names nothing.
            shape: "computed-member",
            blockedExports: 1,
            records: 2,
            spellings: [{ spelling: "", blockedExports: 1, records: 2 }]
          }
        ]
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

  test("ranks the unresolved-callee shapes and their concrete spellings", () => {
    const result = rankDialectAuditYield(synthesizedReport());
    assert.deepEqual(
      result.shapeRanking.map(entry => [
        entry.shape,
        entry.blockedExports,
        entry.records,
        entry.rows
      ]),
      [
        // 2 + 3 exports across the two rows that named it, 9 + 4 call sites.
        ["parameter-rooted", 5, 13, 2],
        ["computed-member", 1, 2, 1],
        ["undeclared-identifier", 1, 1, 1]
      ]
    );
    // Spellings aggregate across rows the same way, and stay ranked by the
    // exports they block rather than by call sites.
    assert.deepEqual(
      result.shapeRanking[0].spellings.map(entry => [
        entry.spelling,
        entry.blockedExports,
        entry.records,
        entry.rows
      ]),
      [
        ["read", 5, 11, 2],
        ["write", 1, 2, 1]
      ]
    );
  });

  test("renders the shape table with its spellings, and names an empty spelling", () => {
    const rendered = renderShapeRanking(rankDialectAuditYield(synthesizedReport()));
    assert.match(rendered, /parameter-rooted\s+5\s+13\s+2/);
    assert.match(rendered, /read 5\/11\s+write 1\/2/);
    assert.match(rendered, /\(none\) 1\/2/);
    assert.match(rendered, /never a claim about what the callee does/);
  });

  test("a report written before the shapes ranks none rather than reporting zero", () => {
    const report = synthesizedReport();
    for (const entry of report.results) delete entry.contractContent.unresolvedCalleeShapes;
    for (const entry of report.supplemental.results) {
      delete entry.contractContent.unresolvedCalleeShapes;
    }
    const result = rankDialectAuditYield(report);
    assert.deepEqual(result.shapeRanking, []);
    assert.match(renderShapeRanking(result), /No unresolved-callee shape recorded/);
    // The dialect-silent half is unaffected, which is what "additive" means.
    assert.equal(result.blockedExportsTotal, 10);
  });

  test("the shape table is printed under the primitive table, not instead of it", () => {
    const rendered = renderRanking(rankDialectAuditYield(synthesizedReport()));
    assert.ok(
      rendered.indexOf("consumer export(s) blocked in total") <
        rendered.indexOf("Unresolved-callee shapes")
    );
    assert.match(rendered, /parameter-rooted/);
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
