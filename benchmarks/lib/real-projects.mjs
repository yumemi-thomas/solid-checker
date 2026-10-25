// The pure half of the real-project edit benchmark (ADR 0423): the manifest's
// shape, the edits, and the paired statistics and gate over measured rows.
// benchmarks/real-projects.mjs does the timing; scripts/real-projects.test.mjs
// tests this file.

export const EDIT_KINDS = ["code", "mid", "body", "import"];
// Head over base, the median of per-repetition ratios.
export const RATIO_LIMIT = 1.25;
// Absolute head ceilings in ms, keyed "project:kind" with "*" as the default.
// Reported, not enforced, until calibrated from runner measurements.
export const CEILINGS = { "app-game:code": 1000, "*": 2500 };

const SHA = /^[0-9a-f]{40}$/;
const DIGEST = /^sha256:[0-9a-f]{64}$/;

/** Every problem with `manifest`, empty when it is usable. */
export function manifestErrors(manifest) {
  const errors = [];
  if (manifest?.format !== 1) errors.push("format must be 1");
  const projects = manifest?.projects;
  if (!Array.isArray(projects) || projects.length === 0) return [...errors, "projects must be a non-empty array"];
  const names = new Set();
  for (const [index, project] of projects.entries()) {
    const at = `projects[${index}]${project?.name ? ` (${project.name})` : ""}`;
    for (const key of ["name", "repo", "commit", "patch", "install", "lockfile", "lockfileDigest", "tsconfig", "editFile"]) {
      if (typeof project?.[key] !== "string" || project[key] === "") errors.push(`${at}: ${key} must be a non-empty string`);
    }
    if (typeof project?.name === "string") {
      if (!/^[a-z0-9-]+$/.test(project.name)) errors.push(`${at}: name must be lowercase letters, digits and dashes`);
      if (names.has(project.name)) errors.push(`${at}: duplicate name`);
      names.add(project.name);
    }
    if (typeof project?.commit === "string" && !SHA.test(project.commit)) errors.push(`${at}: commit must be a full 40-hex sha`);
    if (typeof project?.lockfileDigest === "string" && !DIGEST.test(project.lockfileDigest)) errors.push(`${at}: lockfileDigest must be sha256:<64 hex>`);
    if (typeof project?.repo === "string" && !project.repo.startsWith("https://")) errors.push(`${at}: repo must be an https URL`);
    for (const key of ["patch", "tsconfig", "editFile", "lockfile"]) {
      const value = project?.[key];
      if (typeof value === "string" && (value.startsWith("/") || value.split("/").includes(".."))) errors.push(`${at}: ${key} must be a relative path inside the project`);
    }
    const kinds = project?.kinds;
    if (!Array.isArray(kinds) || kinds.length === 0) errors.push(`${at}: kinds must be a non-empty array`);
    else {
      for (const kind of kinds) if (!EDIT_KINDS.includes(kind)) errors.push(`${at}: unknown edit kind ${kind}`);
      if (new Set(kinds).size !== kinds.length) errors.push(`${at}: duplicate edit kind`);
      if (kinds.includes("body") && typeof project.bodyAnchor !== "string") errors.push(`${at}: a body edit needs bodyAnchor`);
    }
    if (project?.bodyAnchor !== undefined && !kinds?.includes?.("body")) errors.push(`${at}: bodyAnchor without a body edit`);
  }
  return errors;
}

/** `original` with edit `kind` number `seq` applied; every edit is a new, distinct declaration. */
export function editText(original, kind, seq, bodyAnchor) {
  const crlf = original.includes("\r\n");
  const nl = crlf ? "\r\n" : "\n";
  switch (kind) {
    case "code":
      return `${original}${original.endsWith("\n") ? "" : nl}export const __q${seq} = ${seq};${nl}`;
    case "import":
      return `import { createSignal as __s${seq} } from "solid-js";${nl}${original}`;
    case "mid": {
      // Before the first blank line past the imports' usual first lines.
      const lines = original.split("\n");
      const at = lines.findIndex((line, index) => index > 3 && line.trim() === "");
      if (at < 0) throw new Error("no blank line for a mid edit");
      lines.splice(at, 0, `export const __m${seq} = ${seq};${crlf ? "\r" : ""}`);
      return lines.join("\n");
    }
    case "body": {
      const at = original.indexOf(`${bodyAnchor}${nl}`);
      if (!bodyAnchor || at < 0 || original.indexOf(`${bodyAnchor}${nl}`, at + 1) >= 0) {
        throw new Error(`body anchor ${JSON.stringify(bodyAnchor)} must open exactly one line`);
      }
      const end = at + bodyAnchor.length + nl.length;
      return `${original.slice(0, end)}  const __b${seq} = ${seq};${nl}${original.slice(end)}`;
    }
    default:
      throw new Error(`unknown edit kind ${kind}`);
  }
}

export function median(values) {
  if (values.length === 0) return NaN;
  const sorted = [...values].sort((a, b) => a - b);
  const middle = sorted.length >> 1;
  return sorted.length % 2 ? sorted[middle] : (sorted[middle - 1] + sorted[middle]) / 2;
}

/** Two-sided sign test p-value for `positive` against `negative` paired differences. */
export function signP(positive, negative) {
  const n = positive + negative;
  if (n === 0) return 1;
  const k = Math.min(positive, negative);
  let choose = 1;
  let tail = 0;
  for (let i = 0; i <= k; i++) {
    if (i > 0) choose = (choose * (n - i + 1)) / i;
    tail += choose;
  }
  return Math.min(1, (2 * tail) / 2 ** n);
}

export function ceilingFor(project, kind, ceilings = CEILINGS) {
  return ceilings[`${project}:${kind}`] ?? ceilings["*"];
}

/**
 * One line per (project, kind) from timed rows `{project, kind, rep, arm: "base"|"head", ms, hash, retained}`.
 * A pair is one repetition's base and head. The ratio is gated only on rows
 * whose answers agree and whose base answered from its retained daemon; a head
 * answer that did not come from its retained daemon fails the row outright.
 */
export function summarize(rows, { ratioLimit = RATIO_LIMIT, ceilings = CEILINGS, enforceCeilings = false } = {}) {
  const groups = new Map();
  for (const row of rows) {
    const key = `${row.project}\0${row.kind}`;
    if (!groups.has(key)) groups.set(key, { project: row.project, kind: row.kind, reps: new Map() });
    const reps = groups.get(key).reps;
    if (!reps.has(row.rep)) reps.set(row.rep, {});
    reps.get(row.rep)[row.arm] = row;
  }
  const lines = [];
  for (const { project, kind, reps } of groups.values()) {
    const pairs = [...reps.values()].filter(pair => pair.base && pair.head);
    const base = median(pairs.map(pair => pair.base.ms));
    const head = median(pairs.map(pair => pair.head.ms));
    const ratio = median(pairs.map(pair => pair.head.ms / pair.base.ms));
    const slower = pairs.filter(pair => pair.head.ms > pair.base.ms).length;
    const faster = pairs.filter(pair => pair.head.ms < pair.base.ms).length;
    const answersEqual = pairs.every(pair => pair.head.hash === pair.base.hash);
    const headRetained = pairs.every(pair => pair.head.retained);
    const baseRetained = pairs.every(pair => pair.base.retained);
    const ceiling = ceilingFor(project, kind, ceilings);
    const notes = [];
    let fails = false;
    if (pairs.length === 0) {
      notes.push("no complete pair");
      fails = true;
    }
    if (!headRetained) {
      notes.push("head answered without its retained daemon");
      fails = true;
    }
    const ratioGated = answersEqual && baseRetained;
    if (!answersEqual) notes.push("answers differ; ratio not gated");
    if (!baseRetained) notes.push("base answered without its retained daemon; ratio not gated");
    if (ratioGated && ratio > ratioLimit) {
      notes.push(`ratio above ${ratioLimit}`);
      fails = true;
    }
    if (head > ceiling) {
      notes.push(`head above ${ceiling} ms${enforceCeilings ? "" : " (reported)"}`);
      if (enforceCeilings) fails = true;
    }
    lines.push({ project, kind, n: pairs.length, base, head, ratio, slower, faster, signP: signP(slower, faster), answersEqual, headRetained, baseRetained, ratioGated, ceiling, fails, notes });
  }
  return lines;
}

const fixed = (value, digits) => (Number.isFinite(value) ? value.toFixed(digits) : "-");

export function markdownTable(lines, { ratioLimit = RATIO_LIMIT } = {}) {
  const out = [
    `Paired median ratio head/base gate: ${ratioLimit}. CLI wall time, retained daemon, full report.`,
    "",
    "| project | edit | n | base ms | head ms | head/base | head slower | sign p | answers equal | ceiling ms | verdict |",
    "|---|---|---|---|---|---|---|---|---|---|---|",
  ];
  for (const line of lines) {
    const verdict = line.fails ? `FAIL: ${line.notes.join("; ")}` : line.notes.length ? `ok (${line.notes.join("; ")})` : "ok";
    out.push(`| ${line.project} | ${line.kind} | ${line.n} | ${fixed(line.base, 0)} | ${fixed(line.head, 0)} | ${fixed(line.ratio, 3)} | ${line.slower}/${line.n} | ${fixed(line.signP, 3)} | ${line.answersEqual ? "yes" : "NO"} | ${line.ceiling} | ${verdict} |`);
  }
  return `${out.join("\n")}\n`;
}
