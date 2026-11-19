// ADR 1114: the proof gate over fuzz programs. A grid run with `focus.mjs --proof-log` gives each one-shot checker process its own
// proof log (`SOLID_CHECKER_IR_PROOF_LOG=<dir>`, proof_log.rs: one `proof-<pid>.jsonl` per process, which is one program); this folds
// each with the gate's own checks (scripts/lib/proof-log.mjs `proofHits`: C1 an unended site, C2 a reporting decision or row with
// nothing ending it, C3 a silence no tagged arm gave, C4 a finding the report dropped), keeps only the hits, each with its program,
// and deletes the log it read, so a grid's logs never sit on the disk together.
//
//   node scripts/fuzz/proof-fold.mjs <log dir> [--out <hits.jsonl>] [--keep]
import { readdirSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { hitKey, proofHits, readProofLog } from "../lib/proof-log.mjs";

/** The program a log is about: the name the executor gave its check, else the directory under `cases/` or an executor slot its first project file sits in. */
export function programOf(lines) {
  // The executor names each check's program (exec/src/main.rs, `SOLID_CHECKER_IR_PROOF_LOG_PROGRAM`): the path lines' `g`.
  const named = lines.find((line) => line.k === "path" && typeof line.g === "string")?.g;
  if (named) return named;
  for (const line of lines) {
    if (line.k !== "path" || typeof line.f !== "string") continue;
    const match = /\/(?:(?:chrome-)?cases|slots\/\d+)\/([^/]+)\//.exec(line.f);
    if (match) return match[1];
  }
  return null;
}

/** Folds every log of `dir` (removing each unless `keep`), and answers its hits and how many logs and programs it read. */
export function foldProofDir(dir, { keep = false } = {}) {
  const hits = [];
  let logs = 0, malformed = 0;
  const programs = new Set();
  let names = [];
  try {
    names = readdirSync(dir).filter((name) => name.endsWith(".jsonl"));
  } catch {
    return { hits, logs, programs: 0, malformed };
  }
  for (const name of names) {
    const path = join(dir, name);
    const log = readProofLog(path);
    logs += 1;
    malformed += log.malformed;
    const program = programOf(log.lines) ?? name;
    programs.add(program);
    for (const hit of proofHits(log.lines)) {
      const file = typeof hit.file === "string" ? hit.file.replace(/^.*\/(?:(?:chrome-)?cases|slots\/\d+)\/[^/]+\//, "") : "";
      hits.push({ program, key: hitKey(program, file, hit), ...hit });
    }
    if (!keep) rmSync(path, { force: true });
  }
  if (!keep) rmSync(dir, { recursive: true, force: true });
  return { hits, logs, programs: programs.size, malformed };
}

/** Counts of hits by check. */
export function hitCounts(hits) {
  const counts = {};
  for (const hit of hits) counts[hit.check ?? "?"] = (counts[hit.check ?? "?"] ?? 0) + 1;
  return counts;
}

if (import.meta.url === `file://${process.argv[1]}`) {
  const [dir, ...rest] = process.argv.slice(2);
  if (!dir) throw new Error("proof-fold.mjs <log dir> [--out <hits.jsonl>] [--keep]");
  const out = rest.includes("--out") ? rest[rest.indexOf("--out") + 1] : null;
  const folded = foldProofDir(dir, { keep: rest.includes("--keep") });
  if (out) writeFileSync(out, folded.hits.map((hit) => JSON.stringify(hit)).join("\n") + (folded.hits.length ? "\n" : ""));
  console.log(`proof-fold: ${folded.logs} logs, ${folded.programs} programs, ${folded.hits.length} hits ${JSON.stringify(hitCounts(folded.hits))}${folded.malformed ? `, ${folded.malformed} malformed lines` : ""}`);
}
