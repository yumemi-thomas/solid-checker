// The base revision every guard ratchet compares against (ADR 0995).
//
// A ratchet that compares with "the base" is only as strong as that base: with none, a comparison that finds nothing to compare
// passes, and a shallow clone, a missing remote or a mistyped ref turns every "a fall needs a row" check into a silent no-op. So the
// base is resolved here, once, and no resolution is a FAILURE, never a pass:
//   COUNT_BASE=<rev>   the base, named explicitly (a branch, a tag, a commit: whatever `git rev-parse` resolves; a rev it does not
//                      resolve is a failure too, not a fall back);
//   otherwise          the merge base of HEAD with origin/main (CI checks out with fetch-depth 0 so it exists), else, while the flip
//                      runs, with origin/claude/flip-census.
// With neither, the guard fails and says how to name one. On a push to main the merge base is HEAD itself, and every comparison is trivially
// equal; on a pull request's merge ref it is main's tip.
import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

export const repo = resolve(dirname(fileURLToPath(import.meta.url)), "../..");

export function git(...args) {
  const run = spawnSync("git", args, { cwd: repo, encoding: "utf8", maxBuffer: 1 << 28 });
  return run.status === 0 ? run.stdout.trim() : null;
}

const HOW = "set COUNT_BASE=<rev> to name one explicitly, or fetch the full history (CI: actions/checkout with fetch-depth: 0) so the merge base with origin/main exists";

/**
 * The candidate base revisions, in order: `{ revisions, problem }`. `problem` is a sentence when none resolves; `revisions` may hold several
 * (the guard that reads a file at the base takes the first that has it).
 */
export function baseCandidates(env = process.env, run = git) {
  if (env.COUNT_BASE) {
    const revision = run("rev-parse", "--verify", `${env.COUNT_BASE}^{commit}`);
    return revision ? { revisions: [revision], problem: null } : { revisions: [], problem: `COUNT_BASE=${env.COUNT_BASE} names no commit this repository has: ${HOW}` };
  }
  const revisions = ["origin/main", "origin/claude/flip-census"].map((ref) => run("merge-base", "HEAD", ref)).filter(Boolean);
  return revisions.length ? { revisions, problem: null } : { revisions: [], problem: `no base revision: neither origin/main nor origin/claude/flip-census has a merge base with HEAD (a shallow clone or no remote): ${HOW}` };
}

/** The first candidate whose `file` exists at it: `{ revision, text, problem }`; `problem` says why not. */
export function baseFile(file, env = process.env, run = git) {
  const { revisions, problem } = baseCandidates(env, run);
  if (problem) return { revision: null, text: null, problem };
  for (const revision of revisions) {
    const text = run("show", `${revision}:${file}`);
    if (text !== null) return { revision, text, problem: null };
  }
  return { revision: revisions[0], text: null, problem: `${file} does not exist at the base ${revisions.join(", ")}: the comparison has nothing to hold the change to (name a base that has it with COUNT_BASE=<rev>)` };
}

/** The leading `//` comment block of the script at `url`, for its `--help`: how an intended change is recorded (which file, which row, which ADR). */
export function helpOf(url) {
  const lines = readFileSync(fileURLToPath(url), "utf8").split("\n");
  if (lines[0].startsWith("#!")) lines.shift();
  const end = lines.findIndex((line) => !line.startsWith("//"));
  return lines.slice(0, end < 0 ? lines.length : end).map((line) => line.replace(/^\/\/ ?/, "")).join("\n");
}

/**
 * The files of `paths` as they are at `revision`, written to a new temporary directory: `{ dir, problem }` (the caller removes `dir`). A
 * lint that scans a source root can then scan the base's with the same code.
 */
export function exportBase(revision, paths) {
  const dir = mkdtempSync(join(tmpdir(), "guard-base-"));
  const archive = spawnSync("git", ["archive", revision, "--", ...paths], { cwd: repo, maxBuffer: 1 << 30 });
  if (archive.status !== 0) return { dir, problem: `git archive ${revision} failed: ${String(archive.stderr).trim().slice(0, 200)}` };
  const extract = spawnSync("tar", ["-x", "-C", dir], { input: archive.stdout, maxBuffer: 1 << 30 });
  return { dir, problem: extract.status === 0 ? null : `tar failed: ${String(extract.stderr).trim().slice(0, 200)}` };
}
