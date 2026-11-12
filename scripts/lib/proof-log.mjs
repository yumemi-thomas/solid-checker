// The proof log's reader, fold and checks (ADR 0933).
//
// The checker writes one JSON line per fact about a site when SOLID_CHECKER_IR_PROOF_LOG names a file or a
// directory (rust/crates/solid-reactive-ir/src/proof_log.rs); the kinds are documented there. This module reads a log, folds its
// lines by file, and checks what the fold shows:
//
//   C1  a site of the independent universe (proof_universe.rs) that ends in nothing and that the classifier never decided: no row,
//       no census obligation, no compiler discard, no named silence (a `silent` line), no decision.
//   C2  the key check, a false-clean candidate. Either (a) a row that projection reports but no finding stands at, or a finding
//       that left without a typed pass naming why; or (b) a site with a reporting classifier decision and no row, no finding and
//       no named silence (audit candidate S1: `setter_callback_role` answered a non-reporting role after the classifier said the
//       read reports, and the producer returned with no row).
//   C3  a silent row, or a site with no row, whose silence no tagged, non-default arm of the classifier gave: no decision at the
//       site (a producer set the role), a decision that said another role (an override outside the classifier), an untagged path,
//       or a self-declared default arm (silent for want of a proof). Also a named end (`rule: "end"`) that rests on no premise
//       naming code: a fact kind and its span; a label is not a fact (`label-only-end`); or a premise the log itself can confirm
//       and does not (`premise-unconfirmed`: a setter's guard that throws first has its SC2001 violation in the final list).
//
// A site has one `site` line per Chrome code it may raise (batch 11, slice 4: `code`, and `by`/`credited` for the ending that stands at its span), and the checks above are
// asked of each (site, code): an ending ends a code only if it answers that code, a named silence ends the codes its `codes` list (a silence line with none ends every code, as
// a log from before the field does), and the classifier's decisions are about the table's first code (`SITE_CODE`), so a line of another code (SC2009 at a setter call) has no
// decision to be explained by: it is ended by a row, a silence naming it, or it is a C1 hit.
//
// C2, a label-only end and an unconfirmed premise are never accepted as gaps; C1 and C3 are, in fixtures/proof-log/gaps.json, each with a
// stated reason.
import { closeSync, existsSync, openSync, readdirSync, readSync, statSync } from "node:fs";
import { join } from "node:path";

export const CHECKS = ["C1", "C2", "C3"];

/** The decision kinds that answer for a universe site of each table. */
const DECISION_KIND = { read: "read", write: "write", action: "action" };

/** The Chrome runtime code each table of the site universe (rust/crates/solid-reactive-ir/src/proof_universe.rs) answers first: the code the classifier's decisions are about. */
export const SITE_CODE = {
  read: "STRICT_READ_UNTRACKED",
  write: "REACTIVE_WRITE_IN_OWNED_SCOPE",
  action: "ACTION_CALLED_IN_OWNED_SCOPE",
  "store-write": "STORE_WRITE_OUTSIDE_SETTER",
  "on-flush": "ON_FLUSH_IN_OWNED_SCOPE",
};

/**
 * The premise kinds whose claim the log itself confirms, each `(premise, verdicts by span) => boolean`. A setter whose write guard
 * throws first (ADR 0932) has the SC2001 violation of its call in the final list, at the span the premise names, in the end's file.
 */
const PREMISE_CONFIRMED = {
  SetterGuardThrowsFirst: (premise, verdicts) =>
    premise.in === undefined && (verdicts.get(`${premise.at[0]}:${premise.at[1]}`) ?? []).some((v) => v.code === "SC2001" && v.kind === "violation"),
};

const logFiles = (path) =>
  statSync(path).isDirectory()
    ? readdirSync(path).filter((name) => name.endsWith(".jsonl")).sort().map((name) => join(path, name))
    : [path];

/**
 * A line with its file path and program resolved: a checker writes each path once per process, as a
 * `{"k":"path","q":pid,"p":id,"f":path}` line ahead of the lines that name it by `q` and `p`. A line naming a path no earlier line
 * gave is malformed. Path lines are read, not yielded.
 */
export function resolvedLine(line, paths, counter) {
  if (line.k === "path") {
    paths.set(`${line.q}:${line.p}`, line.g === undefined ? { f: line.f } : { f: line.f, g: line.g });
    return null;
  }
  if (line.f !== undefined || line.p === undefined) return line;
  const named = paths.get(`${line.q}:${line.p}`);
  if (named === undefined) {
    counter.malformed += 1;
    return null;
  }
  const { q, p, ...rest } = line;
  return { ...rest, ...named };
}

/** Every line of a proof log file or directory, parsed and resolved, read in 8 MiB chunks. */
export function* proofLogLines(path, counter = { malformed: 0 }) {
  const chunk = Buffer.alloc(8 << 20);
  for (const file of logFiles(path)) {
    const fd = openSync(file, "r");
    const paths = new Map();
    const parsed = (text) => {
      let line;
      try {
        line = JSON.parse(text);
      } catch {
        counter.malformed += 1;
        return null;
      }
      return resolvedLine(line, paths, counter);
    };
    let rest = "";
    try {
      for (let read; (read = readSync(fd, chunk, 0, chunk.length, null)) > 0; ) {
        const parts = (rest + chunk.toString("utf8", 0, read)).split("\n");
        rest = parts.pop();
        for (const text of parts) {
          if (!text) continue;
          const line = parsed(text);
          if (line) yield line;
        }
      }
      if (rest) {
        const line = parsed(rest);
        if (line) yield line;
      }
    } finally {
      closeSync(fd);
    }
  }
}

/** Every line of a log, parsed into an array (a fixture's log; a grid folds `proofLogLines`). */
export function readProofLog(path) {
  if (!existsSync(path)) return { lines: [], malformed: 0, missing: true };
  const counter = { malformed: 0 };
  const lines = [...proofLogLines(path, counter)];
  return { lines, malformed: counter.malformed, missing: false };
}

const TABLES = ["decision", "owner", "pending", "row", "finding", "verdict", "drop", "reclass", "attributed", "site", "silent"];

/** The lines of one file, by kind. */
function tablesOf() {
  return Object.fromEntries(TABLES.map((kind) => [kind, []]));
}

/** `lines` grouped by the file they are about and by kind: `Map<path, { decision: [], row: [], ... }>`. */
export function foldFiles(lines) {
  const files = new Map();
  for (const line of lines) {
    if (line.f === undefined || !TABLES.includes(line.k)) continue;
    if (!files.has(line.f)) files.set(line.f, tablesOf());
    files.get(line.f)[line.k].push(line);
  }
  return files;
}

const spanKey = (line) => `${line.s}:${line.e}`;

function groupBy(list, key) {
  const groups = new Map();
  for (const item of list) {
    const k = key(item);
    if (!groups.has(k)) groups.set(k, []);
    groups.get(k).push(item);
  }
  return groups;
}

/** Whether a decision says the site reports (a read, write or action rule speaks there). */
const reports = (decision) => decision.silent === false;

/**
 * Every hit the checks find in the lines of one program (a fixture's log). A hit: `{ check, kind, file, s, e, table, detail }`.
 * `detail` names what the fold saw (roles, arms, passes) so a reader can judge the entry without the log.
 */
export function proofHits(lines) {
  const hits = [];
  for (const [file, t] of foldFiles(lines)) {
    const findings = groupBy(t.finding, spanKey);
    const verdicts = groupBy(t.verdict, spanKey);
    const drops = groupBy([...t.drop, ...t.reclass], spanKey);
    const silents = groupBy(t.silent, spanKey);
    const decisionsAt = groupBy(t.decision, (decision) => `${decision.t}@${decision.s}`);
    // A row may sit at any alternate span of its site (a composite callee, a chain link): find the site by each.
    const siteAt = new Map();
    for (const site of t.site) {
      siteAt.set(spanKey(site), site);
      for (const [s, e] of site.alt ?? []) if (!siteAt.has(`${s}:${e}`)) siteAt.set(`${s}:${e}`, site);
    }
    // The decisions of a `kind` that answer for a site: those that start where it does or where an alternate span of it does (a
    // parenthesized callee starts one byte after the call it is the callee of).
    const decisionsFor = (kind, site) => {
      const starts = new Set([site.s, ...(site.alt ?? []).map(([start]) => start)]);
      return [...starts].flatMap((start) => decisionsAt.get(`${kind}@${start}`) ?? []);
    };
    const hit = (check, kind, at, table, detail) => hits.push({ check, kind, file, s: at.s, e: at.e, table, detail });

    // C2(c) (ADR 0958): a finding the dependency-source drop discarded stands again at a project site: its `attributed` line, written
    // beside the `drop` by the one function that drops it (`package_drop::hand_off`), lists at least one. A drop with none is a package
    // finding that left the report with no project row: a false clean (a style rule's finding is dropped under its own named pass).
    const attributedAt = groupBy(t.attributed, spanKey);
    for (const drop of t.drop) {
      if (drop.pass !== "DependencySource") continue;
      const stands = (attributedAt.get(spanKey(drop)) ?? []).some((line) => Array.isArray(line.to) && line.to.length > 0);
      if (!stands) hit("C2", "package-drop-unattributed", drop, "drop", { code: drop.code, pass: drop.pass });
    }

    // C2(a): a row projection reports has a finding, and a finding ends as a verdict or in a typed drop; a drop alone is a
    // finding the projection itself removed (ADR 0659's client-runtime rules).
    for (const row of t.row) {
      if (!row.reported) continue;
      // A read reported at its origin (ADR 0930) has its finding there; the row says where (`fa`).
      const key = row.fa ? `${row.fa[0]}:${row.fa[1]}` : spanKey(row);
      if (!findings.has(key)) {
        if (!drops.has(key)) hit("C2", "reported-row-no-finding", row, row.t, { role: row.role, why: row.why });
      } else if (!verdicts.has(key) && !drops.has(key)) {
        hit("C2", "finding-lost", row, row.t, { role: row.role, codes: findings.get(key).map((finding) => finding.code) });
      }
    }

    // A site that ends in nothing: no row, no census obligation, no compiler discard. Its account is the classifier's answer
    // (a tagged, non-default arm that said silent), a named silence, or nothing at all.
    for (const site of t.site) {
      // A named silence at the site, or at any span a row for it may sit at (`alt`), is its account when it names the site's code (a line with no `codes` names every code).
      const spans = [spanKey(site), ...(site.alt ?? []).map(([s, e]) => `${s}:${e}`)];
      const silencedFor = (key) => (silents.get(key) ?? []).some((silent) => silent.codes === undefined || site.code === undefined || silent.codes.includes(site.code));
      if (site.end !== "none" || spans.some(silencedFor)) continue;
      // The classifier's decisions are about the table's first code: another code of the site has none.
      const kind = DECISION_KIND[site.t];
      const decisions = kind && (site.code === undefined || site.code === SITE_CODE[site.t]) ? decisionsFor(kind, site) : [];
      const speaking = decisions.filter(reports);
      const detail = {
        via: site.via,
        source: site.source,
        roles: [...new Set(decisions.map((decision) => decision.role))],
        arms: [...new Set(decisions.map((decision) => decision.arm))],
      };
      if (speaking.length > 0) {
        // The classifier said this site reports and nothing ends it (audit candidate S1). A props member read whose prop every
        // caller passes statically ends in a named `props-static` end; one with none is as unexplained as any other.
        hit("C2", "reporting-decision-no-row", site, site.t, detail);
      } else if (decisions.length === 0) {
        hit("C1", "no-decision", site, site.t, detail);
      } else if (!decisions.some((decision) => decision.arm !== null && decision.default !== true)) {
        hit("C3", decisions.some((decision) => decision.default === true) ? "default-arm" : "untagged", site, site.t, { ...detail, rowless: true });
      }
    }

    // A producer's named end rests on premises that name code, each a fact kind with the span of the code the fact is about. The
    // writer drops an end that has none (an unnamed end is then a hit above); one in a log is malformed, never a gap.
    for (const end of t.silent) {
      if (end.rule !== "end") continue;
      const premises = end.premises ?? [];
      if (premises.length === 0 || premises.some((premise) => !Array.isArray(premise.at))) {
        hit("C3", "label-only-end", end, "end", { id: end.proof, premises });
        continue;
      }
      for (const premise of premises) {
        if (PREMISE_CONFIRMED[premise.kind]?.(premise, verdicts) === false) {
          hit("C3", "premise-unconfirmed", end, "end", { id: end.proof, kind: premise.kind, at: premise.at });
        }
      }
    }

    // C3: a silent row's role must come from a tagged, non-default arm of the classifier.
    for (const row of t.row) {
      // A silent end the producer named at the row (an override over the classifier's answer) is the account.
      if (row.reported || row.why !== "role-does-not-report" || !DECISION_KIND[row.t] || silents.has(spanKey(row))) continue;
      const decisions = decisionsFor(row.t, siteAt.get(spanKey(row)) ?? row);
      const same = decisions.filter((decision) => decision.role === row.role);
      const explained = same.some((decision) => decision.arm !== null && decision.default !== true);
      if (explained) continue;
      const detail = { role: row.role, decisionRoles: [...new Set(decisions.map((decision) => decision.role))] };
      if (decisions.length === 0) hit("C3", "no-decision", row, row.t, detail);
      else if (same.length === 0) hit("C3", "override", row, row.t, detail);
      else if (same.some((decision) => decision.default === true)) {
        hit("C3", "default-arm", row, row.t, { ...detail, arm: same.find((decision) => decision.default === true).arm });
      } else hit("C3", "untagged", row, row.t, detail);
    }
  }
  return hits;
}

/** Counts of what the log holds, for the gate's summary line. */
export function proofStats(lines) {
  const stats = { sites: 0, siteEnds: {}, siteCodes: {}, rows: 0, reportedRows: 0, findings: 0, verdicts: 0, drops: {}, silents: 0, proofs: {}, decisions: 0 };
  for (const line of lines) {
    switch (line.k) {
      case "site":
        stats.sites += 1;
        stats.siteEnds[line.end] = (stats.siteEnds[line.end] ?? 0) + 1;
        if (line.code !== undefined) stats.siteCodes[line.code] = (stats.siteCodes[line.code] ?? 0) + 1;
        break;
      case "row":
        stats.rows += 1;
        if (line.reported) stats.reportedRows += 1;
        break;
      case "finding":
        stats.findings += 1;
        break;
      case "verdict":
        stats.verdicts += 1;
        break;
      case "drop":
      case "reclass":
        stats.drops[`${line.k}:${line.pass}`] = (stats.drops[`${line.k}:${line.pass}`] ?? 0) + 1;
        break;
      case "silent":
        stats.silents += 1;
        stats.proofs[line.proof] = (stats.proofs[line.proof] ?? 0) + 1;
        break;
      case "decision":
        stats.decisions += 1;
        break;
      default:
    }
  }
  return stats;
}

/** A hit's identity in the gap file: fixture, file, byte span, check and kind. Byte offsets, as the coverage snapshots use. */
export function hitKey(fixture, relativeFile, hit) {
  return `${fixture}|${relativeFile}|${hit.s}-${hit.e}|${hit.check}/${hit.kind}`;
}

/** A hit no gap entry may accept: a false-clean candidate, and a named end that is a label rather than a fact, or a claim the log refutes. */
export const neverAccepted = (check, kind) => check === "C2" || kind === "label-only-end" || kind === "premise-unconfirmed";

/**
 * The candidate entries a run still has a hit for (`kept`) and those it has none for (`removed`). The branch that fixes a site
 * removes its entry in the same commit; this does it from a run, and only ever removes.
 */
export function pruneCandidates(candidates, hitsByKey) {
  const kept = {};
  const removed = [];
  for (const [key, entry] of Object.entries(candidates)) {
    if (hitsByKey.has(key)) kept[key] = entry;
    else removed.push(key);
  }
  return { kept, removed };
}

/** Reasons that state no fact. A gap's `why` must say what makes the silence sound or why the site is outside the claim. */
export const EMPTY_REASONS = /^(|unreviewed|todo|tbd|known|harmless|ok|fine|n\/a|none|noise|expected|legacy)\.?$/i;

/**
 * The problems of a gap file against a run's hits, `[{ kind, key, message }]`:
 * - `new`: a hit no entry accepts (a C2 hit is `candidate` or `new`, never accepted);
 * - `stale`: an entry no hit matches (the file only shrinks);
 * - `invalid`: an entry that is not `{ check, why }` with a stated reason, or accepts a C2.
 * `candidates` lists hits under review by their key, each with the owner who fixes it; a listed hit is `candidate`, which the gate
 * reports and does not fail, and a listed key no hit matches is `stale`.
 */
export function gapProblems(hitsByKey, gaps, candidates = {}) {
  const problems = [];
  for (const [key, entry] of Object.entries(gaps)) {
    if (!entry || typeof entry !== "object" || typeof entry.check !== "string") {
      problems.push({ kind: "invalid", key, message: "an entry is { check, why }" });
      continue;
    }
    const hitKind = key.slice(key.lastIndexOf("/") + 1);
    if (neverAccepted(entry.check, hitKind)) {
      problems.push({ kind: "invalid", key, message: "a C2 hit is a false-clean candidate, a label-only end is no fact and an unconfirmed premise is refuted by the log: never an accepted gap" });
    }
    if (typeof entry.why !== "string" || EMPTY_REASONS.test(entry.why.trim()) || entry.why.trim().length < 20) {
      problems.push({ kind: "invalid", key, message: "why must state the fact that makes the silence sound (not empty, 'known' or 'harmless')" });
    }
    if (!hitsByKey.has(key)) problems.push({ kind: "stale", key, message: "no hit matches this entry any more: remove it" });
  }
  for (const [key, hit] of hitsByKey) {
    if (key in gaps) continue;
    if (key in candidates) {
      problems.push({ kind: "candidate", key, message: `under review, not accepted (${candidates[key].owner ?? "no owner"})` });
    } else {
      problems.push({ kind: "new", key, message: `${hit.check}/${hit.kind}` });
    }
  }
  for (const key of Object.keys(candidates)) {
    if (!hitsByKey.has(key)) problems.push({ kind: "stale", key, message: "candidate no longer a hit: remove it from the candidates file" });
  }
  return problems;
}
