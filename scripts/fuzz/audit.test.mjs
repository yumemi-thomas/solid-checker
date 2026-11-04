import { mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { gzipSync } from "node:zlib";
import { describe, expect, it } from "vitest";
import { AUDIT_GRIDS, ORACLE_AXES, ORACLE_PER_STRATUM, judge, markdown, parseArgs, pickOracle, readShards, recallModeOf } from "./audit.mjs";
import { fnv } from "./rng.mjs";
import { parseAxes, sampleStrata, stratumOf } from "./runtime-diff.mjs";
import { shardOf } from "./shard.mjs";

// Grid rows across codes, contexts and outcomes; every fifth has no fast record (the differential leaves it out).
const rows = (n = 900) => Array.from({ length: n }, (_, i) => ({ id: `mx-${i}`, code: ["A", "B", "C"][i % 3], context: ["memo", "body"][i % 2], outcome: i % 7 ? "agree" : "recall-gap" }));
const hasRecord = (row) => Number(row.id.slice(3)) % 5 !== 0;
const candidates = (list, perStratum, axes) => sampleStrata(list, perStratum, hasRecord, axes).map((row) => ({ id: row.id, stratum: stratumOf(row, axes), rank: fnv(String(row.id)) }));

describe("the audit's oracle sample", () => {
  it("is the whole grid's sample, picked from every shard's candidates", () => {
    const axes = parseAxes(ORACLE_AXES);
    for (const perStratum of [1, 3]) for (const count of [1, 4, 40]) {
      const all = rows();
      const whole = sampleStrata(all, perStratum, hasRecord, axes).map((row) => row.id);
      const shards = Array.from({ length: count }, (_, index) => candidates(all.filter((row) => shardOf(row.id, count) === index), perStratum, axes));
      expect(pickOracle(shards, perStratum)).toEqual(whole);
    }
  });

  it("takes focus.mjs's sample: one program per code, context and outcome", () => {
    expect(ORACLE_PER_STRATUM).toBe("1");
    expect(ORACLE_AXES).toBe("code,context,outcome");
    expect(AUDIT_GRIDS.toSorted()).toEqual(["mx-base", "mx-src", "mx-combo", "lg-base", "lg-src", "lg-combo", "mx-census", "mx-recall", "lg-recall"].toSorted());
  });
});

// A run directory as the workflow lays it out: shards/<name>/<grid>/ and oracle/<grid>/, oracle/manifests/<grid>.json.
function run({ count = 2, drop = null, oracleOutcome = null, behaviour = false, problem = false, recall = "full", mixed = false } = {}) {
  const root = mkdtempSync(join(tmpdir(), "fuzz-audit-"));
  const all = Object.fromEntries(AUDIT_GRIDS.map((grid) => [grid, Array.from({ length: 10 }, (_, i) => ({ id: `${grid}-${i}`, outcome: problem && grid === "mx-base" && i === 3 ? "false-clean" : "agree" }))]));
  for (let index = 0; index < count; index += 1) {
    if (drop === index) continue;
    for (const grid of AUDIT_GRIDS) {
      const dir = join(root, "shards", `shard-${index}`, grid);
      mkdirSync(dir, { recursive: true });
      const mine = all[grid].filter((row) => shardOf(row.id, count) === index);
      writeFileSync(join(dir, "programs.jsonl"), mine.map((row) => JSON.stringify(row)).join("\n") + "\n");
      writeFileSync(join(dir, grid.startsWith("mx") ? "matrix.json" : "legal.json"), JSON.stringify({ gridShard: { index, count, kept: mine.length, of: 10 }, selected: mine.length, ...(grid.endsWith("-recall") && (recall === "lean") !== (mixed && index === 1) ? { recallLean: true } : {}) }));
    }
  }
  for (const grid of AUDIT_GRIDS) {
    const dir = join(root, "oracle", grid);
    mkdirSync(join(root, "oracle", "manifests"), { recursive: true });
    mkdirSync(dir, { recursive: true });
    const ids = [`${grid}-0`, `${grid}-1`];
    writeFileSync(join(root, "oracle", "manifests", `${grid}.json`), JSON.stringify({ grid, ids }));
    writeFileSync(join(dir, "runtime-diff.jsonl"), ids.map((id) => JSON.stringify({ id, diffs: [] })).join("\n") + "\n");
    writeFileSync(join(dir, "runtime-diff.json"), JSON.stringify({ summary: { compared: 2, behaviour: behaviour ? 1 : 0, inconclusive: 0 }, differing: behaviour && grid === "lg-src" ? [{ id: ids[0], diffs: [{ kind: "moved", detail: "x" }] }] : [] }));
    writeFileSync(join(dir, "programs.jsonl"), ids.map((id) => JSON.stringify({ id, outcome: oracleOutcome && grid === "mx-recall" ? oracleOutcome : "agree" })).join("\n") + "\n");
  }
  return root;
}

describe("the workflow's lean and full audits", () => {
  const workflow = readFileSync(new URL("../../.github/workflows/fuzz-audit.yml", import.meta.url), "utf8");
  it("runs lean Monday to Saturday and full on Sunday, and a manual run chooses", () => {
    expect(workflow).toContain('cron: "0 16 * * 1-6"');
    expect(workflow).toContain('cron: "0 16 * * 0"');
    // The plan step reads the Sunday schedule as full, the same string the cron declares.
    expect(workflow).toContain('[ "${{ github.event.schedule }}" = "0 16 * * 0" ]');
    expect(workflow).toMatch(/recall:\n\s+description:[^\n]*\n\s+type: choice\n\s+options: \[lean, full\]\n\s+default: lean/);
  });
  it("gives the shards the mode, and files and fetches the report under it (the legacy audit-report is a full one)", () => {
    expect(workflow).toContain('--recall "${{ needs.build.outputs.recall }}"');
    expect(workflow).toContain("name: audit-report-${{ needs.build.outputs.recall }}");
    expect(workflow).toContain("name: ${{ needs.build.outputs.earlier-rows-name }}");
    expect(workflow).toContain('elif [ "$RECALL" = full ] && has "$id" audit-report;');
  });
});

describe("the audit's verdict", () => {
  it("passes a whole run with nothing to look at, and counts every program once", () => {
    const root = run();
    const result = judge({ shards: join(root, "shards"), oracle: join(root, "oracle") });
    expect(result.attention).toEqual([]);
    expect(result.count).toBe(2);
    for (const grid of AUDIT_GRIDS) expect(result.grids[grid]).toMatchObject({ programs: 10, planned: 10, outcomes: { agree: 10 }, oracle: { compared: 2, programs: 2 } });
    expect(markdown(result)).toMatch(/Nothing needs a look/);
  });

  it("needs a look at a missing shard, a problem row, an oracle behaviour difference, and an outcome the oracle grid disagrees with", () => {
    const missing = judge({ shards: join(run({ drop: 1 }), "shards"), oracle: join(run(), "oracle") });
    expect(missing.attention.some((item) => /shard 1\/2 missing/.test(item))).toBe(true);
    expect(judge({ shards: join(run({ problem: true }), "shards"), oracle: join(run(), "oracle") }).attention).toEqual([expect.stringMatching(/^mx-base: false-clean mx-base-3/)]);
    const behaviour = run({ behaviour: true });
    expect(judge({ shards: join(behaviour, "shards"), oracle: join(behaviour, "oracle") }).attention).toEqual([expect.stringMatching(/^lg-src: oracle differs lg-src-0 moved/)]);
    const differs = run({ oracleOutcome: "recall-gap" });
    expect(judge({ shards: join(differs, "shards"), oracle: join(differs, "oracle") }).attention).toEqual([
      "mx-recall: mx-recall-0 is agree in its shard, recall-gap in the oracle grid",
      "mx-recall: mx-recall-1 is agree in its shard, recall-gap in the oracle grid",
    ]);
  });

  it("refuses shards of different runs and counts the moves from an earlier report", () => {
    const root = run();
    writeFileSync(join(root, "shards", "shard-1", "mx-base", "matrix.json"), JSON.stringify({ gridShard: { index: 1, count: 3, kept: 5, of: 10 } }));
    expect(readShards(join(root, "shards")).attention.some((item) => /shard of 3/.test(item))).toBe(true);
    const fresh = run();
    const against = join(fresh, "before");
    mkdirSync(against, { recursive: true });
    for (const grid of AUDIT_GRIDS) writeFileSync(join(against, `${grid}.jsonl.gz`), gzipSync(Array.from({ length: 10 }, (_, i) => JSON.stringify({ id: `${grid}-${i}`, outcome: grid === "lg-base" && i < 2 ? "recall-gap" : "agree" })).join("\n")));
    const result = judge({ shards: join(fresh, "shards"), oracle: join(fresh, "oracle"), against });
    expect(result.moves.moved).toBe(2);
    expect(result.moves.kinds).toEqual({ "lg-base: recall-gap -> agree": 2 });
  });

  it("is lean or full by what its shards' recall grids recorded, and a report from before the flag is full", () => {
    const shardsOf = (root) => readShards(join(root, "shards")).shards;
    expect(recallModeOf(shardsOf(run()))).toBe("full");
    expect(recallModeOf(shardsOf(run({ recall: "lean" })))).toBe("lean");
    expect(recallModeOf(shardsOf(run({ recall: "lean", mixed: true })))).toBe("mixed");
    const lean = run({ recall: "lean" });
    expect(judge({ shards: join(lean, "shards"), oracle: join(lean, "oracle") })).toMatchObject({ recall: "lean", attention: [] });
    expect(markdown(judge({ shards: join(lean, "shards"), oracle: join(lean, "oracle") }))).toMatch(/lean recall grids/);
    const mixed = run({ recall: "lean", mixed: true });
    expect(judge({ shards: join(mixed, "shards"), oracle: join(mixed, "oracle") }).attention).toEqual(["the shards' recall grids are of both lean and full modes"]);
  });

  it("compares a lean night with the latest lean audit and a full one with a full one: the recall grids only like with like, the base grids always", () => {
    // An earlier audit as the artifact lays it out: <dir>/audit.json beside <dir>/rows/<grid>.jsonl.gz (the earlier run's rows all agree).
    const earlier = (mode) => {
      const dir = mkdtempSync(join(tmpdir(), "fuzz-audit-earlier-"));
      mkdirSync(join(dir, "rows"));
      if (mode) writeFileSync(join(dir, "audit.json"), JSON.stringify({ recall: mode }));
      for (const grid of AUDIT_GRIDS) writeFileSync(join(dir, "rows", `${grid}.jsonl.gz`), gzipSync(Array.from({ length: 10 }, (_, i) => JSON.stringify({ id: `${grid}-${i}`, outcome: grid === "lg-base" && i < 2 ? "recall-gap" : "agree" })).join("\n")));
      return join(dir, "rows");
    };
    const judged = (recall, mode) => { const root = run({ recall }); return judge({ shards: join(root, "shards"), oracle: join(root, "oracle"), against: earlier(mode) }); };
    // Same mode: every grid compared (the lg-base moves are the earlier rows').
    expect(judged("lean", "lean").moves.kinds).toEqual({ "lg-base: recall-gap -> agree": 2 });
    expect(judged("full", "full").moves.kinds).toEqual({ "lg-base: recall-gap -> agree": 2 });
    // An earlier audit from before the flag (no audit.json, or none with `recall`) is a full one.
    expect(judged("full", null).moves.kinds).toEqual({ "lg-base: recall-gap -> agree": 2 });
    // Another mode: the two recall grids are not compared (and say so), the base grids still are.
    for (const [now, was] of [["lean", "full"], ["lean", null], ["full", "lean"]]) {
      expect(judged(now, was).moves.kinds).toEqual({
        "lg-base: recall-gap -> agree": 2,
        [`mx-recall: not compared (the earlier audit was ${was ?? "full"}, this one is ${now})`]: 1,
        [`lg-recall: not compared (the earlier audit was ${was ?? "full"}, this one is ${now})`]: 1,
      });
      expect(judged(now, was).moves.moved).toBe(2);
    }
  });

  it("reads its phases' arguments", () => {
    expect(parseArgs(["shard", "--shard", "0/4", "--recall", "full", "--out", "o", "--checker-bin", "c", "--typefacts", "t"]).options.recall).toBe("full");
    expect(() => parseArgs(["shard", "--shard", "0/4", "--recall", "half", "--out", "o", "--checker-bin", "c", "--typefacts", "t"])).toThrow(/--recall is lean or full/);
    expect(parseArgs(["shard", "--shard", "0/4", "--out", "o", "--checker-bin", "c", "--typefacts", "t", "--", "--cache-dir", "x"])).toEqual({ phase: "shard", options: { shard: "0/4", out: "o", checkerBin: "c", typefacts: "t" }, extra: ["--cache-dir", "x"] });
    expect(() => parseArgs(["pick", "--shards", "s"])).toThrow(/--out is required/);
    expect(() => parseArgs(["oracle", "--grid", "nope", "--manifest", "m", "--out", "o", "--checker-bin", "c", "--typefacts", "t"])).toThrow(/no grid nope/);
    expect(() => parseArgs(["dance"])).toThrow(/shard\|pick\|oracle\|report/);
  });
});
