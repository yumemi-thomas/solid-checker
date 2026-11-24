// R7: totals, moves and families from the focus runs and the blocker scan.
//   node analyze.mjs <res dir: <build>-<grid>.jsonl> <scan.jsonl> <old build> <new build>
import { readFileSync } from "node:fs";
const [res, scanFile, OLD, NEW] = process.argv.slice(2);
const G = ["mx-base", "mx-src", "mx-combo", "lg-base", "lg-src", "lg-combo", "mx-recall", "lg-recall", "mx-pairs", "lg-pairs", "mx-census"];
const read = (f) => new Map(readFileSync(f, "utf8").trim().split("\n").filter(Boolean).map((l) => JSON.parse(l)).map((r) => [r.id, r]));
const totals = { [OLD]: {}, [NEW]: {} }, moves = {}, bad = [];
const BAD = new Set(["false-clean", "false-positive", "wrong-rule", "misplaced", "unplaced", "halted-claim", "stacked-claim", "unjudged-claim", "harness", "not-legal"]);
for (const g of G) {
  const o = read(`${res}/${OLD}-${g}.jsonl`), n = read(`${res}/${NEW}-${g}.jsonl`);
  const side = g.startsWith("mx") ? "misuse" : "legal";
  for (const [id, r] of n) {
    const a = o.get(id)?.outcome ?? "missing", b = r.outcome;
    totals[OLD][`${side} ${a}`] = (totals[OLD][`${side} ${a}`] ?? 0) + 1;
    totals[NEW][`${side} ${b}`] = (totals[NEW][`${side} ${b}`] ?? 0) + 1;
    if (a !== b) moves[`${side} ${a} -> ${b}`] = (moves[`${side} ${a} -> ${b}`] ?? 0) + 1;
    if (BAD.has(b)) bad.push(`${id} ${b}`);
  }
}
console.log("totals", JSON.stringify(totals, null, 1));
console.log("moves", JSON.stringify(moves));
console.log("false or unjudged results on", NEW, bad.length, bad.slice(0, 50));
const rows = readFileSync(scanFile, "utf8").trim().split("\n").slice(1).map((l) => JSON.parse(l)).filter((r) => !r.error);
const fam = new Map();
for (const r of rows) {
  const at = Object.entries(r.u?.kinds ?? {}).filter(([, [a]]) => a > 0).sort((x, y) => y[1][0] - x[1][0]);
  const key = `${r.side} ${r.code} ${at[0]?.[0] ?? "(none at the site)"}`;
  const f = fam.get(key) ?? fam.set(key, { programs: 0, rows: 0, ctx: {}, also: {} }).get(key);
  f.programs += 1;
  for (const [kind, [a]] of at) { f.rows += a; if (kind !== at[0][0]) f.also[kind] = (f.also[kind] ?? 0) + a; }
  f.ctx[r.context] = (f.ctx[r.context] ?? 0) + 1;
}
console.log("families by at-site rows (side, code, main at-site blocker):");
for (const [k, f] of [...fam].sort((a, b) => b[1].rows - a[1].rows).slice(0, 15)) {
  const ctx = Object.entries(f.ctx).sort((a, b) => b[1] - a[1]);
  const also = Object.entries(f.also).sort((a, b) => b[1] - a[1]).slice(0, 2).map(([x, n]) => `${x} ${n}`).join(", ");
  console.log(`${f.rows}\t${f.programs}\t${k}\t${ctx.length} contexts: ${ctx.slice(0, 4).map(([c, n]) => `${c} ${n}`).join(", ")}\talso at the site: ${also}`);
}
