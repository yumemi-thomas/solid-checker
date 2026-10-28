#!/usr/bin/env node
// Runs the pinned shapes of the packages profile (packages-seeds.mjs) through the fast path and prints, per shape, what the runtime did
// against what the shape is known to do, and what the checker said. Exit 1 when the runtime contradicts a pinned shape (the harness is wrong).
//   node scripts/fuzz/packages-check.mjs --out <fresh dir> [--checker-bin ...] [--typefacts ...]
import { mkdirSync } from "node:fs";
import { resolve } from "node:path";
import { buildCase } from "./emit.mjs";
import { SEEDS } from "./packages-seeds.mjs";
import { generateProgram } from "./generate.mjs";
import { evaluate } from "./pipeline.mjs";
import { attachDaemon, configFrom, parseArgs } from "./run.mjs";
import { requireTypeChecker, resolveTsc } from "./typecheck.mjs";
import { runtimeEvents } from "./classify.mjs";

const THROWS = ["REACTIVE_WRITE_IN_OWNED_SCOPE", "ACTION_CALLED_IN_OWNED_SCOPE"];
const options = parseArgs(process.argv.slice(2));
const out = resolve(options.out);
try { requireTypeChecker(resolveTsc(options.tsc)); } catch (error) { console.error(`fuzz: ${error.message}`); process.exit(2); }
mkdirSync(out, { recursive: true });
const config = attachDaemon(configFrom(options, out), options);
try {
  const entries = SEEDS.map((seed, index) => {
    const program = generateProgram(200 + index, { profile: "packages", packagePlan: seed.plan });
    return { seed, program, built: buildCase(program, { id: `seed-${index + 1}` }) };
  });
  const results = await evaluate(entries, config);
  let bad = 0;
  for (const { seed, built } of entries) {
    const value = results.get(built.id);
    const site = built.sites[0];
    const events = value.runtime ? runtimeEvents(value.runtime) : [];
    const at = events.filter((event) => event.positions.some((position) => position.line === site.line));
    const codes = [...new Set(at.map((event) => event.code))];
    const throws = codes.some((code) => THROWS.includes(code));
    const siteHits = value.runtime?.hits?.[site.hit] ?? null;
    // A write that throws still ran; one that never ran proves nothing about the shape.
    const ok = value.typecheck?.ok && throws === seed.throws && (seed.ran === false ? !(siteHits > 0) : siteHits > 0);
    if (!ok) bad += 1;
    const checker = (value.checker?.findings ?? []).map((f) => `${f.rule}:${f.kind}@${f.line}`).join(" ") || "silent";
    console.log(`${ok ? "ok " : "BAD"} ${seed.name}: expected ${seed.throws ? "throw" : "legal"}, runtime ${throws ? "throw" : "legal"} [${codes.join(",") || "-"}] site hits ${siteHits} typecheck ${value.typecheck?.ok} | checker ${checker} -> ${value.classification.verdict}`);
  }
  process.exitCode = bad ? 1 : 0;
} finally {
  await config.fast?.close();
}
