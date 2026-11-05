import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { gzipSync } from "node:zlib";
import { describe, expect, it } from "vitest";
import { AUDIT_GRIDS, ORACLE_AXES, ORACLE_PART_FLOOR, ORACLE_PER_STRATUM, ORACLE_RUNNERS, judge, markdown, oracleJobs, oracleParts, parseArgs, parsePart, partIds, pickOracle, readShards, recallModeOf } from "./audit.mjs";
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

  // `source` and `mount` refine the old strata, so every code, context and outcome keeps a program (ADR 0641).
  it("takes focus.mjs's sample: one program per code, context, source, mount and outcome", () => {
    expect(ORACLE_PER_STRATUM).toBe("1");
    expect(ORACLE_AXES).toBe("code,context,source,mount,outcome");
    expect(AUDIT_GRIDS.toSorted()).toEqual(["mx-base", "mx-src", "mx-combo", "lg-base", "lg-src", "lg-combo", "mx-census", "mx-recall", "lg-recall", "mx-pairs", "lg-pairs"].toSorted());
  });
});

// A run directory as the workflow lays it out: shards/<name>/<grid>/ and oracle/<grid>/, oracle/manifests/<grid>.json. `parts` splits a
// grid's oracle as the workflow does (its manifest records `parts`, its results are oracle/<grid>/part-<i>/); `dropPart` leaves one out.
function run({ count = 2, drop = null, oracleOutcome = null, behaviour = false, problem = false, recall = "full", mixed = false, parts = {}, dropPart = null } = {}) {
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
  mkdirSync(join(root, "oracle", "manifests"), { recursive: true });
  for (const grid of AUDIT_GRIDS) {
    const all = [`${grid}-0`, `${grid}-1`, ...(parts[grid] ? [`${grid}-2`, `${grid}-3`] : [])];
    writeFileSync(join(root, "oracle", "manifests", `${grid}.json`), JSON.stringify({ grid, ids: all, ...(parts[grid] ? { parts: parts[grid] } : {}) }));
    const slices = parts[grid] ? Array.from({ length: parts[grid] }, (_, part) => [join(root, "oracle", grid, `part-${part}`), partIds(all, part, parts[grid])]) : [[join(root, "oracle", grid), all]];
    slices.forEach(([dir, ids], part) => {
      if (dropPart && dropPart.grid === grid && dropPart.part === part) return;
      mkdirSync(dir, { recursive: true });
      const moved = behaviour && grid === "lg-src" && ids.includes(`${grid}-0`) ? [{ id: `${grid}-0`, diffs: [{ kind: "moved", detail: "x" }] }] : [];
      writeFileSync(join(dir, "runtime-diff.jsonl"), ids.map((id) => JSON.stringify({ id, diffs: [] })).join("\n") + "\n");
      writeFileSync(join(dir, "runtime-diff.json"), JSON.stringify({ summary: { compared: ids.length, behaviour: moved.length, inconclusive: 0, byKind: moved.length ? { moved: 1 } : {}, perStratum: "all" }, differing: moved }));
      writeFileSync(join(dir, "programs.jsonl"), ids.map((id) => JSON.stringify({ id, outcome: oracleOutcome && grid === "mx-recall" ? oracleOutcome : "agree" })).join("\n") + "\n");
    });
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
  it("runs the oracle in pick's jobs, each part on its own warm cache, and files each part where report reads it", () => {
    expect(workflow).toContain("matrix: ${{ fromJSON(needs.pick.outputs.jobs) }}");
    expect(workflow).toContain('echo "jobs=$(cat "$RUNNER_TEMP/manifests/jobs.json")" >> "$GITHUB_OUTPUT"');
    expect(workflow).toContain('--part "${{ matrix.part }}/${{ matrix.parts }}"');
    expect(workflow).toMatch(/--part[^\n]*\n[^\n]*\n\s+-- --cache-dir "\$HOME\/fuzz-cache"/);
    expect(workflow).toContain("STATE: warm-oracle-${{ matrix.grid }}-${{ matrix.part }}of${{ matrix.parts }}-${{ needs.build.outputs.state-key }}");
    expect(workflow).toContain("name: oracle-${{ matrix.grid }}-part${{ matrix.part }}");
    expect(workflow).toContain('mv "$d" "$RUNNER_TEMP/oracle/${name%-part*}/part-${name##*-part}"');
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

  it("puts a split grid's oracle parts together, and needs a look at a part with no result", () => {
    const split = run({ parts: { "mx-recall": 2, "lg-src": 3 }, behaviour: true, oracleOutcome: "recall-gap" });
    const result = judge({ shards: join(split, "shards"), oracle: join(split, "oracle") });
    expect(result.grids["mx-recall"].oracle).toMatchObject({ compared: 4, programs: 4, behaviour: 0, inconclusive: 0, perStratum: "all" });
    expect(result.grids["lg-src"].oracle).toMatchObject({ compared: 4, programs: 4, behaviour: 1, byKind: { moved: 1 } });
    // Every part's rows are read: the differing program of lg-src's part 0, and the outcomes of both mx-recall parts (ids 0 to 3).
    expect(result.attention).toEqual([
      expect.stringMatching(/^lg-src: oracle differs lg-src-0 moved/),
      ...[0, 2, 1, 3].map((i) => `mx-recall: mx-recall-${i} is agree in its shard, recall-gap in the oracle grid`),
    ]);
    const gap = run({ parts: { "mx-recall": 2 }, dropPart: { grid: "mx-recall", part: 1 } });
    expect(judge({ shards: join(gap, "shards"), oracle: join(gap, "oracle") }).attention).toEqual(["mx-recall: no oracle result for part 1/2 (2 programs)", "mx-recall: 2 oracle programs not compared (mx-recall-1, mx-recall-3)"]);
  });

  it("splits a manifest's ids into parts that take each id once and keep the rank order's spread", () => {
    const ids = Array.from({ length: 911 }, (_, i) => `p${i}`);
    for (const parts of [1, 2, 4]) {
      const slices = Array.from({ length: parts }, (_, part) => partIds(ids, part, parts));
      expect(slices.flat().sort()).toEqual([...ids].sort());
      expect(Math.max(...slices.map((s) => s.length)) - Math.min(...slices.map((s) => s.length))).toBeLessThanOrEqual(1);
    }
    expect(partIds(ids, 1, 4).slice(0, 3)).toEqual(["p1", "p5", "p9"]);
  });

  it("splits the samples over the run's runners, the largest part first, never below the floor", () => {
    expect([ORACLE_RUNNERS, ORACLE_PART_FLOOR]).toEqual([20, 200]);
    // Run 41775917761's samples (a full-recall night): mx-census's 2,294 programs were one job of 90 minutes.
    const full = { "mx-base": 106, "mx-src": 96, "mx-combo": 151, "lg-base": 54, "lg-src": 38, "lg-combo": 100, "mx-recall": 1231, "lg-recall": 1316, "mx-pairs": 198, "lg-pairs": 100, "mx-census": 2294 };
    expect(Object.keys(full).toSorted()).toEqual(AUDIT_GRIDS.toSorted());
    const parts = oracleParts(full);
    expect(Object.fromEntries(Object.entries(parts).filter(([, n]) => n > 1))).toEqual({ "mx-recall": 3, "lg-recall": 3, "mx-census": 6 });
    expect(Object.values(parts).reduce((a, b) => a + b)).toBe(ORACLE_RUNNERS);
    expect(Math.max(...Object.keys(full).map((grid) => Math.ceil(full[grid] / parts[grid])))).toBe(439);
    // Run 41754339595's (lean, before the census): runners left over, every part at most the floor.
    const lean = { "mx-base": 106, "mx-src": 96, "mx-combo": 149, "lg-base": 53, "lg-src": 36, "lg-combo": 98, "mx-recall": 1025, "lg-recall": 950 };
    expect(oracleParts(lean)).toMatchObject({ "mx-recall": 6, "lg-recall": 5, "mx-combo": 1 });
    // Small samples are never split; an empty one is one job; more grids than runners each keep one; a tie goes to the first grid.
    expect(oracleParts({ a: 200, b: 0 })).toEqual({ a: 1, b: 1 });
    expect(oracleParts({})).toEqual({});
    const many = Object.fromEntries(Array.from({ length: 25 }, (_, i) => [`g${i}`, 5000]));
    expect(Object.values(oracleParts(many)).every((n) => n === 1)).toBe(true);
    expect(oracleParts({ a: 1000, b: 1000, ...Object.fromEntries(Array.from({ length: 17 }, (_, i) => [`s${i}`, 1])) })).toMatchObject({ a: 2, b: 1 });
    // The matrix: every grid and part once, and every program in exactly one job.
    const manifests = AUDIT_GRIDS.map((grid) => ({ grid, ids: Array.from({ length: full[grid] }, (_, i) => `${grid}-${i}`), parts: parts[grid] }));
    const jobs = oracleJobs(manifests).include;
    expect(jobs.length).toBe(ORACLE_RUNNERS);
    expect(new Set(jobs.map((job) => `${job.grid}#${job.part}/${job.parts}`)).size).toBe(jobs.length);
    const taken = jobs.flatMap((job) => partIds(manifests.find((m) => m.grid === job.grid).ids, job.part, job.parts));
    expect(taken.toSorted()).toEqual(manifests.flatMap((m) => m.ids).toSorted());
    expect(jobs.filter((job) => job.grid === "mx-census")).toEqual([0, 1, 2, 3, 4, 5].map((part) => ({ grid: "mx-census", part, parts: 6 })));
  });

  it("writes the parts into pick's manifests and its matrix", () => {
    const root = run({ count: 2 });
    const size = { "mx-census": 850, "lg-src": 0 };
    for (const grid of AUDIT_GRIDS) {
      const candidates = Array.from({ length: size[grid] ?? 5 }, (_, i) => ({ id: `${grid}-c${i}`, stratum: `s${i}`, rank: fnv(`${grid}-c${i}`) }));
      for (let index = 0; index < 2; index += 1) {
        const mine = candidates.filter((candidate) => shardOf(candidate.id, 2) === index);
        writeFileSync(join(root, "shards", `shard-${index}`, grid, "oracle-candidates.jsonl"), mine.map((row) => JSON.stringify(row)).join("\n") + "\n");
      }
    }
    const out = join(root, "manifests");
    const result = spawnSync(process.execPath, [new URL("./audit.mjs", import.meta.url).pathname, "pick", "--shards", join(root, "shards"), "--out", out], { encoding: "utf8" });
    expect(result.status, result.stderr).toBe(0);
    const manifest = (grid) => JSON.parse(readFileSync(join(out, `${grid}.json`), "utf8"));
    // 850 census programs and ten 5-program grids: the census splits until a part is at most the floor (5 of 170), runners to spare.
    expect(manifest("mx-census")).toMatchObject({ parts: 5 });
    expect(manifest("mx-census").ids.length).toBe(850);
    expect(manifest("lg-src")).toMatchObject({ parts: 1, ids: [] });
    expect(manifest("mx-base")).toMatchObject({ parts: 1 });
    const jobs = JSON.parse(readFileSync(join(out, "jobs.json"), "utf8")).include;
    expect(jobs.length).toBe(AUDIT_GRIDS.length + 4);
    for (const grid of AUDIT_GRIDS) expect(jobs.filter((job) => job.grid === grid).map((job) => job.part)).toEqual(Array.from({ length: manifest(grid).parts }, (_, part) => part));
  });

  it("reads its phases' arguments", () => {
    expect(parseArgs(["shard", "--shard", "0/4", "--recall", "full", "--out", "o", "--checker-bin", "c", "--typefacts", "t"]).options.recall).toBe("full");
    expect(() => parseArgs(["shard", "--shard", "0/4", "--recall", "half", "--out", "o", "--checker-bin", "c", "--typefacts", "t"])).toThrow(/--recall is lean or full/);
    expect(parseArgs(["shard", "--shard", "0/4", "--out", "o", "--checker-bin", "c", "--typefacts", "t", "--", "--cache-dir", "x"])).toEqual({ phase: "shard", options: { shard: "0/4", out: "o", checkerBin: "c", typefacts: "t" }, extra: ["--cache-dir", "x"] });
    expect(() => parseArgs(["pick", "--shards", "s"])).toThrow(/--out is required/);
    expect(() => parseArgs(["oracle", "--grid", "nope", "--manifest", "m", "--out", "o", "--checker-bin", "c", "--typefacts", "t"])).toThrow(/no grid nope/);
    expect(() => parseArgs(["dance"])).toThrow(/shard\|pick\|oracle\|report/);
    const oracle = ["oracle", "--grid", "mx-recall", "--manifest", "m", "--out", "o", "--checker-bin", "c", "--typefacts", "t"];
    expect(parseArgs([...oracle, "--part", "3/4"]).options.part).toBe("3/4");
    for (const bad of ["4/4", "1", "-1/2", "a/b", "0/0"]) expect(() => parseArgs([...oracle, "--part", bad])).toThrow(/--part is i\/N/);
    expect(parsePart(undefined)).toEqual({ part: 0, parts: 1 });
    expect(parsePart("1/2")).toEqual({ part: 1, parts: 2 });
  });
});
