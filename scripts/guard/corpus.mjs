// node scripts/guard/corpus.mjs check | record [--apps-root <dir>] [--checker <bin>] [--typefacts <bin>] [--jobs N] [--build std|dev]: the corpus findings of a
// branch held to main's (ADR 1012). Needs the frozen corpus (scripts/corpus/rc21-manifest.json, its apps installed) and a release checker; no CI job has either.
//
// A violation that leaves the corpus, or turns uncertifiable, was a proved result (ADR 0995's regress.mjs holds the fixtures to that; the corpus is where a
// real project loses one), and a (project, file, rule) that held no uncertifiable row and holds one now is a proved-clean place that stopped being proved.
// Neither was a failure anywhere: the corpus delta of an ADR is measured by whoever writes it, and a count rising in a project's total says nothing of which
// site lost its proof. `check` sweeps the branch (release checker, the standard build and the development build, the two runtime builds of the corpus
// manifest's configuration) and compares with the committed baseline of THIS machine:
//   - a VIOLATION of the baseline that is not a violation now (same project, file, rule id and byte span) is a lost proof: `became uncertifiable` when the
//     (project, file, rule) holds more uncertifiable rows than before, else `gone`;
//   - a (project, file, rule) with uncertifiable rows now and none at the baseline is a place that was proved clean and no longer is;
//   - each needs a NEW row of scripts/guard/corpus-changes.json (a row the base lacks; append-only against the base): { project, rule, kind: "lost-violation" |
//     "newly-uncertifiable", build: "std" | "dev" | "both", count, adr, why }, `count` the most it names (rows of a group add up), the ADR in docs/decisions.md.
// Other movement (more rows where some stood, a violation added, a row gone) is printed, never a failure.
//
// THE BASELINE (scripts/guard/corpus/<fingerprint>.json.gz). Findings differ with the corpus install (ADR 0974: +1,176 rows between two Macs on the same commit), so
// a baseline names where it was built: the platform, the architecture and a hash of the installed packages of every app (the names and versions under each
// app's node_modules), and `check` refuses a baseline built elsewhere rather than report the moves of another install. It also names the checker it was built
// from (the git trees of rust/ and apps/solid-typefacts at main's tip when it was recorded), and `check` refuses one older than the merge base's checker: main
// moves with every ADR, and a stale baseline would report the earlier ADR's moves as this branch's. `record` writes it, and only from a tree whose checker is
// the merge base's (a main checkout or a branch with no engine change), because a baseline is main's findings. Aggregate U counts and exact Vs make a baseline
// of both builds 4.4 MB of JSON (212 violations and 22,250 (project, file, rule) places a build), so it is stored gzipped (deterministically: no mtime), under
// .githooks/pre-commit's 5 MB by a wide margin.
//
// Exit 1: a lost proof or a proved-clean place lost, unnamed. Exit 2: nothing could be compared (no corpus, no baseline for this machine, a stale one, no base).
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { gunzipSync, gzipSync } from "node:zlib";

import { baseCandidates, git, helpOf, repo } from "./base-revision.mjs";
import { readReportFile } from "../lib/corpus-report.mjs";

export const BASELINES = "scripts/guard/corpus";
export const CHANGES = "scripts/guard/corpus-changes.json";
const BUILDS = { std: "production", dev: "development" };
const same = (a, b) => JSON.stringify(a) === JSON.stringify(b);
const text = (value) => typeof value === "string" && value.trim() !== "";
const adrNumber = (adr) => /^(?:ADR )?(\d{4})$/.exec(adr ?? "")?.[1];

/**
 * The digest of a sweep's results: the violations exactly (`project\tfile\trule-id\tstart-end`) and the uncertifiable rows counted per
 * (project, file, rule id) (`project\tfile\tid`). Paths are relative to each project's root.
 */
export function digestResults(results) {
  const violations = [];
  const uncertifiable = {};
  for (const result of results) {
    const project = `${result.app}/${result.project}`;
    for (const finding of result.findings ?? []) {
      const path = finding.primaryLocation?.path ?? "";
      const file = path.startsWith(`${result.root}/`) ? path.slice(result.root.length + 1) : path;
      if (finding.kind === "violation") violations.push(`${project}\t${file}\t${finding.id}\t${finding.primaryLocation?.startByte}-${finding.primaryLocation?.endByte}`);
      else if (finding.kind === "uncertifiable") {
        const key = `${project}\t${file}\t${finding.id}`;
        uncertifiable[key] = (uncertifiable[key] ?? 0) + 1;
      }
    }
  }
  return { violations: violations.sort(), uncertifiable: Object.fromEntries(Object.entries(uncertifiable).sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0))) };
}

/**
 * The moves of a build's digest from the baseline's: `{ lost, cleanLost, added, rose, fell }`. `lost`: `[{ project, file, id, start, kind }]` (kind
 * `became-uncertifiable` or `gone`), `cleanLost`: `[{ project, file, id, count }]` (no uncertifiable row at the baseline), the others counts.
 */
export function movesOf(base, head) {
  const heldNow = new Set(head.violations);
  const lost = [];
  for (const key of base.violations) {
    if (heldNow.has(key)) continue;
    const [project, file, id, span] = key.split("\t");
    const rose = (head.uncertifiable[`${project}\t${file}\t${id}`] ?? 0) > (base.uncertifiable[`${project}\t${file}\t${id}`] ?? 0);
    lost.push({ project, file, id, start: span, kind: rose ? "became-uncertifiable" : "gone" });
  }
  const heldBefore = new Set(base.violations);
  const cleanLost = [];
  let rose = 0, fell = 0;
  for (const [key, count] of Object.entries(head.uncertifiable)) {
    const was = base.uncertifiable[key] ?? 0;
    if (!was) { const [project, file, id] = key.split("\t"); cleanLost.push({ project, file, id, count }); } else if (count > was) rose += count - was;
  }
  for (const [key, was] of Object.entries(base.uncertifiable)) if ((head.uncertifiable[key] ?? 0) < was) fell += was - (head.uncertifiable[key] ?? 0);
  return { lost, cleanLost, added: head.violations.filter((key) => !heldBefore.has(key)).length, rose, fell };
}

/**
 * Whether `rows` (the NEW rows of corpus-changes.json) name the moves of both builds: `{ unnamed: [{ build, kind, project, id, missing, example }] }`. A group is
 * (build, kind, project, rule id); a row names a group when its project and rule (id) match and its build is that one or `both`; rows add up, and a row's
 * count is spent once across the groups it names.
 */
export function unnamedMoves({ std, dev }, rows) {
  const spent = rows.map(() => 0);
  const unnamed = [];
  const groups = [];
  for (const [build, moves] of [["std", std], ["dev", dev]]) {
    const by = new Map();
    for (const item of moves.lost) { const key = `lost-violation\t${item.project}\t${item.id}`; by.set(key, [...(by.get(key) ?? []), item]); }
    for (const item of moves.cleanLost) { const key = `newly-uncertifiable\t${item.project}\t${item.id}`; by.set(key, [...(by.get(key) ?? []), item]); }
    for (const [key, items] of by) { const [kind, project, id] = key.split("\t"); groups.push({ build, kind, project, id, items }); }
  }
  for (const group of groups) {
    let missing = group.items.length;
    rows.forEach((row, index) => {
      if (!missing || row.kind !== group.kind || row.project !== group.project || (row.rule ?? row.id) !== group.id || (row.build !== group.build && row.build !== "both")) return;
      const take = Math.min(row.count - spent[index], missing);
      if (take > 0) { spent[index] += take; missing -= take; }
    });
    if (missing) unnamed.push({ build: group.build, kind: group.kind, project: group.project, id: group.id, missing, example: group.items[0].file });
  }
  return { unnamed };
}

/** Problems of corpus-changes.json against its base: append-only; a new row needs the shape, an ADR and a why. */
export function changesProblems({ head, base, decisions }) {
  const problems = [];
  for (const old of base.rows ?? []) if (!(head.rows ?? []).some((row) => same(row, old))) problems.push(`corpus-changes.json is append-only: a row of the base is gone or changed (${old.project} ${old.rule ?? old.id} ${old.adr ?? ""})`);
  for (const row of head.rows ?? []) {
    if ((base.rows ?? []).some((old) => same(old, row))) continue;
    const name = `${row.project ?? "?"} ${row.rule ?? row.id ?? "?"} ${row.adr ?? "no ADR"}`;
    if (!text(row.project) || !text(row.rule ?? row.id)) problems.push(`corpus-changes.json: new row ${name} needs a project and a rule id (SCnnnn)`);
    if (!["lost-violation", "newly-uncertifiable"].includes(row.kind)) problems.push(`corpus-changes.json: new row ${name}: kind is lost-violation or newly-uncertifiable`);
    if (!["std", "dev", "both"].includes(row.build)) problems.push(`corpus-changes.json: new row ${name}: build is std, dev or both`);
    if (!Number.isInteger(row.count) || row.count < 1) problems.push(`corpus-changes.json: new row ${name}: count must be a positive integer`);
    const number = adrNumber(row.adr);
    if (!number || !decisions.includes(`<a id="adr-${number}"></a>`)) problems.push(`corpus-changes.json: new row ${name}: ${row.adr ?? "no adr"} is not an ADR in docs/decisions.md`);
    if (!text(row.why)) problems.push(`corpus-changes.json: new row ${name} says no why`);
  }
  return problems;
}

/** The baseline a branch can be held to: `{ ok, problem }` (a baseline built elsewhere, or from a checker older than the base's, is refused). */
export function baselineUsable({ baseline, fingerprint, trees }) {
  if (!baseline) return { ok: false, problem: `no corpus baseline for this machine (${fingerprint}): record one from a main checkout with \`node scripts/guard/corpus.mjs record\`; another machine's baseline is not used (the findings differ with the corpus install, ADR 0974)` };
  if (baseline.fingerprint !== fingerprint) return { ok: false, problem: `the baseline was built on ${baseline.fingerprint}, this machine is ${fingerprint}: refused (the findings differ with the corpus install)` };
  if (!same(baseline.checker, trees)) return { ok: false, problem: `the baseline was recorded from a checker (${baseline.checker?.rust?.slice(0, 10)}) other than the merge base's (${trees.rust?.slice(0, 10)}): main moved since; record it again from a main checkout` };
  return { ok: true, problem: null };
}

/** The fingerprint of this machine's corpus install: platform, architecture, and a hash of every app's installed package names and versions. */
export function installFingerprint(manifest, appsRoot, { platform = process.platform, arch = process.arch } = {}) {
  const hash = createHash("sha256");
  for (const app of Object.keys(manifest.apps).sort()) {
    const lines = [];
    const modules = join(appsRoot, app, manifest.apps[app].installDir ?? ".", "node_modules");
    const lock = join(modules, ".package-lock.json");
    if (existsSync(lock)) {
      for (const [path, entry] of Object.entries(JSON.parse(readFileSync(lock, "utf8")).packages ?? {})) lines.push(`${path}@${entry.version ?? "?"}`);
    } else if (existsSync(modules)) {
      for (const name of readdirSync(modules).filter((entry) => !entry.startsWith("."))) {
        const dirs = name.startsWith("@") ? readdirSync(join(modules, name)).map((sub) => `${name}/${sub}`) : [name];
        for (const dir of dirs) {
          const pkg = join(modules, dir, "package.json");
          lines.push(`${dir}@${existsSync(pkg) ? (JSON.parse(readFileSync(pkg, "utf8")).version ?? "?") : "?"}`);
        }
      }
    } else lines.push("no node_modules");
    hash.update(`${app}\0${lines.sort().join("\n")}\0`);
  }
  return `${platform}-${arch}-${hash.digest("hex").slice(0, 12)}`;
}

/** The git trees of the engine at `revision`: what a baseline's findings depend on. */
export const engineTrees = (revision) => ({ rust: git("rev-parse", `${revision}:rust`), typefacts: git("rev-parse", `${revision}:apps/solid-typefacts`) });

function sweep({ build, manifestPath, appsRoot, checker, typefacts, jobs, out }) {
  const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
  manifest.configuration = { ...manifest.configuration, runtimeBuild: BUILDS[build] };
  const dir = mkdtempSync(join(tmpdir(), `corpus-guard-${build}-`));
  const file = join(dir, "manifest.json");
  writeFileSync(file, `${JSON.stringify(manifest)}\n`);
  const report = join(out, `${build}.json`);
  const run = spawnSync(process.execPath, [join(repo, "scripts/corpus-sweep.mjs"), "--manifest", file, "--apps-root", appsRoot, "--checker", checker, "--typefacts", typefacts, "--jobs", String(jobs), "--out", report], { stdio: ["ignore", "inherit", "inherit"] });
  if (run.status !== 0) throw new Error(`corpus-sweep (${build}) exited ${run.status}`);
  const results = [];
  readReportFile(report, { onResult: (result) => results.push(result) });
  return digestResults(results);
}

function parseArgs(argv) {
  const options = { _: [] };
  for (let index = 0; index < argv.length; index += 1) {
    if (argv[index].startsWith("--")) { options[argv[index].slice(2)] = argv[index + 1]; index += 1; } else options._.push(argv[index]);
  }
  return options;
}

const read = (file) => readFileSync(join(repo, file), "utf8");

function main(argv) {
  if (argv.includes("--help")) return console.log(helpOf(import.meta.url));
  const options = parseArgs(argv);
  const [command] = options._;
  if (!["check", "record"].includes(command)) throw new Error("usage: corpus.mjs check|record [--apps-root <dir>] [--checker <bin>] [--typefacts <bin>] [--jobs N]");
  const refuse = (message) => { console.error(message); process.exitCode = 2; };
  const { revisions, problem } = baseCandidates();
  if (problem) return refuse(problem);
  const base = revisions[0];
  const manifestPath = join(repo, "scripts/corpus/rc21-manifest.json");
  const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
  const appsRoot = resolve(options["apps-root"] ?? join(repo, "../solid-checker-rc21-frozen/apps"));
  if (!existsSync(appsRoot)) return refuse(`no corpus at ${appsRoot}: the corpus guard needs the frozen corpus installed (--apps-root)`);
  const checker = resolve(options.checker ?? join(repo, "rust/target/release/solid-checker-rust"));
  const typefacts = resolve(options.typefacts ?? join(repo, "bin/solid-typefacts"));
  if (!existsSync(checker)) return refuse(`no release checker at ${checker}: make build-checker-release`);
  const fingerprint = installFingerprint(manifest, appsRoot);
  const trees = engineTrees(base);
  const file = join(repo, BASELINES, `${fingerprint}.json.gz`);
  const jobs = Number(options.jobs ?? 4);
  const out = mkdtempSync(join(tmpdir(), "corpus-guard-"));
  const only = options.build ? [options.build] : ["std", "dev"];
  if (command === "record") {
    const here = engineTrees("HEAD");
    if (!same(here, trees)) return refuse("record only from a tree whose checker is the merge base's (a main checkout): a baseline is main's findings, and this branch changes rust/ or apps/solid-typefacts");
    const builds = {};
    for (const build of only) builds[build] = sweep({ build, manifestPath, appsRoot, checker, typefacts, jobs, out });
    mkdirSync(join(repo, BASELINES), { recursive: true });
    writeFileSync(file, gzipSync(`${JSON.stringify({ schema: 1, fingerprint, platform: process.platform, arch: process.arch, checker: trees, builtFrom: base, builds })}\n`, { level: 9 }));
    console.log(`corpus baseline ${fingerprint}: ${only.map((build) => `${build} ${builds[build].violations.length} violations, ${Object.keys(builds[build].uncertifiable).length} (project, file, rule) places`).join("; ")}`);
    return;
  }
  const baseline = existsSync(file) ? JSON.parse(gunzipSync(readFileSync(file)).toString("utf8")) : null;
  const usable = baselineUsable({ baseline, fingerprint, trees });
  if (!usable.ok) return refuse(usable.problem);
  const moves = {};
  for (const build of only) moves[build] = movesOf(baseline.builds[build], sweep({ build, manifestPath, appsRoot, checker, typefacts, jobs, out }));
  const baseChanges = git("show", `${base}:${CHANGES}`);
  const headChanges = existsSync(join(repo, CHANGES)) ? JSON.parse(read(CHANGES)) : { rows: [] };
  const problems = changesProblems({ head: headChanges, base: baseChanges ? JSON.parse(baseChanges) : { rows: [] }, decisions: read("docs/decisions.md") });
  const fresh = (headChanges.rows ?? []).filter((row) => !(baseChanges ? JSON.parse(baseChanges).rows ?? [] : []).some((old) => same(old, row)));
  const { unnamed } = unnamedMoves({ std: moves.std ?? { lost: [], cleanLost: [] }, dev: moves.dev ?? { lost: [], cleanLost: [] } }, fresh);
  for (const build of only) {
    const m = moves[build];
    console.log(`corpus ${build}: ${m.lost.length} violation(s) lost (${m.lost.filter((x) => x.kind === "became-uncertifiable").length} became uncertifiable), ${m.cleanLost.length} place(s) newly uncertifiable, ${m.added} violation(s) added, uncertifiable rows +${m.rose} where some stood and -${m.fell}`);
  }
  for (const item of unnamed) problems.push(`corpus ${item.build}: ${item.missing} ${item.kind === "lost-violation" ? "lost violation(s)" : "newly uncertifiable place(s)"} of ${item.id} in ${item.project} (e.g. ${item.example}) named by no new row of ${CHANGES}`);
  for (const line of problems) console.error(line);
  process.exitCode = problems.length ? 1 : 0;
}

if (import.meta.main ?? process.argv[1] === fileURLToPath(import.meta.url)) main(process.argv.slice(2));
