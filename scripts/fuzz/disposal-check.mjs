#!/usr/bin/env node
// Runs the pinned disposal shapes (disposal-seeds.mjs) through the fast path and prints, per shape, what the runtime did against
// what the shape is known to do, and what the checker said. Exit 1 when the runtime contradicts a pinned shape (the harness is wrong).
//   node scripts/fuzz/disposal-check.mjs --out <fresh dir> [--checker-bin ...] [--typefacts ...]
import { mkdirSync } from "node:fs";
import { resolve } from "node:path";
import { buildCase } from "./emit.mjs";
import { cleanupEvidence } from "./disposal.mjs";
import { SEEDS } from "./disposal-seeds.mjs";
import { generateProgram } from "./generate.mjs";
import { evaluate } from "./pipeline.mjs";
import { attachDaemon, configFrom, parseArgs } from "./run.mjs";
import { runtimeEvents } from "./classify.mjs";

const options = parseArgs(process.argv.slice(2));
const out = resolve(options.out);
mkdirSync(out, { recursive: true });
const config = attachDaemon(configFrom(options, out), options);
try {
  const entries = SEEDS.map((seed, index) => {
    const program = generateProgram(100 + index, { profile: "disposal", disposalPlan: seed.plan });
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
    const throws = codes.some((code) => ["REACTIVE_WRITE_IN_OWNED_SCOPE", "ACTION_CALLED_IN_OWNED_SCOPE"].includes(code));
    const siteHits = value.runtime?.hits?.[site.hit] ?? null;
    const cleanup = value.runtime ? cleanupEvidence(value.program, built.ledger, value.runtime.hits) : null;
    const ok = value.typecheck?.ok && throws === seed.throws && siteHits > 0;
    if (!ok) bad += 1;
    console.log(`${ok ? "ok " : "BAD"} ${seed.name}: expected ${seed.throws ? "throw" : "legal"}, runtime ${throws ? "throw" : "legal"} [${codes.join(",") || "-"}] site hits ${siteHits} cleanup entered ${cleanup?.entered ?? "-"} typecheck ${value.typecheck?.ok} | checker ${(value.checker?.findings ?? []).map((f) => `${f.rule}:${f.kind}@${f.line}`).join(" ") || "silent"} -> ${value.classification.verdict}`);
  }
  process.exitCode = bad ? 1 : 0;
} finally {
  await config.fast?.close();
}
