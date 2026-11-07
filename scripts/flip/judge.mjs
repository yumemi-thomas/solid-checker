// The witness judge, pure: `judgeWitness(w, out)` for one row of witnesses.json or host-witnesses.json and the checker's JSON report.
// check.mjs does the I/O; guard.test.mjs runs this on in-memory rows. Rules per kind are documented at the top of check.mjs.
import { codeOfThrown, codeRules, ruleHasRuntimeCounterpart } from "../fuzz/codes.mjs";
import { judgePattern } from "../fuzz/patterns.mjs";

import { isNamedBlocker } from "../lib/blockers.mjs";

export { isNamedBlocker };

/** Every code the recorded run showed: its diagnostics, and each uncaught error's code (`<phase>: [CODE] …` or a known message). */
export function recordedCodes(runtime) {
  const thrown = (runtime?.errors ?? []).map((error) => {
    const text = String(error).replace(/^[a-z?]+: /, "");
    return codeOfThrown(text) ?? /^\[([A-Z_]+)\]/.exec(text)?.[1] ?? null;
  });
  return [...new Set([...(runtime?.diagnostics ?? []), ...thrown.filter(Boolean)])];
}

/** `{ ok, why }`: whether the checker's report `out` keeps witness `w`. */
export function judgeWitness(w, out) {
  const findings = out.findings ?? [];
  const now = findings.map((f) => `${f.id}/${f.kind[0]}@${f.primaryLocation?.line}`);
  const shown = now.join(" ") || "no findings";
  // ADR 0680: an uncertifiable with no named blocker is a silent gap in the row itself, whatever the witness asks of it.
  const unnamed = findings.filter((f) => f.kind === "uncertifiable" && !isNamedBlocker(f.analysisContext));
  if (unnamed.length) return { ok: false, why: `blocker: ${unnamed.map((f) => `${f.id}@${f.analysisContext ?? ""}`).join(" ")}` };
  if (w.host) {
    const seen = [...(w.runtime.client ?? []), ...(w.runtime.server ?? [])];
    const claimed = new Set(seen.flatMap((code) => codeRules[code] ?? []));
    const unearned = findings.filter((f) => f.kind === "violation" && ruleHasRuntimeCounterpart(f.rule) && !claimed.has(f.rule));
    // One rule of a code covers it: STRICT_READ_UNTRACKED maps to four, and a program shows one of them.
    const missed = seen.filter((code) => (codeRules[code] ?? []).length && !findings.some((f) => codeRules[code].includes(f.rule)));
    if (!unearned.length && !missed.length) return { ok: true };
    return { ok: false, why: `host: ${w.open ? `open (${w.open}) ` : ""}unearned ${unearned.map((f) => f.id).join(" ") || "none"}, missed ${missed.join(" ") || "none"}; ${shown}` };
  }
  // Whatever the kind: where the recorded run fired a code some rule claims, a finding must cover it (the pattern lane's judge).
  // A `keep` or `regress` row has no other check that the program does not go back to clean, and an SC9012 `clean` row passed on any
  // finding at all (endstage-report.md 1a, rule 4).
  const codes = recordedCodes(w.runtime);
  const verdict = judgePattern({ codes }, findings);
  // A contradicted violation decides the pattern judge's outcome first; its missed codes are asked without it.
  const missed = verdict.outcome === "false-positive" ? judgePattern({ codes }, findings.filter((f) => !verdict.rules.includes(f.rule))) : verdict;
  if (codes.length && missed.outcome === "false-clean") return { ok: false, why: `Chrome fired ${codes.join(" ")}, no finding covers it: ${out.status} ${shown}` };
  const own = findings.filter((f) => f.id === w.rule || (w.rule === "SC2004" && f.id === "SC2005"));
  const was = w.before?.findings ?? [];
  let ok;
  // `closedBy`: the census closes the row, under SC9012. A row of its own needs that finding, or a proved violation: an unrelated
  // uncertifiable elsewhere in the program must not pass a witness (`endstage-report.md` 1a).
  // `closedAt`: the line and blocker of the row that covers the runtime site, read by hand; a `closedBy` or SC9012 row without it fails.
  // An SC9012 row passes only at its `closedAt` too (review item 4a): an SC9012 elsewhere says nothing about the site. `closedAt.id`
  // names a proved violation at the site instead of an SC9012 row.
  const closedAt = (id) => Boolean(w.closedAt) && findings.some((f) => f.primaryLocation?.line === w.closedAt.line && (w.closedAt.id ? f.id === w.closedAt.id && f.kind === "violation" : f.id === id && (f.analysisContext ?? "").split(":")[0] === w.closedAt.blocker));
  if (w.kind === "clean") ok = w.closedBy ? closedAt(w.closedBy) : w.rule === "SC9012" ? closedAt("SC9012") : own.length > 0;
  else if (w.kind === "pref") ok = own.some((f) => f.kind === (w.proved ? "violation" : "uncertifiable"));
  else if (w.kind === "warn") ok = !own.some((f) => f.kind === "violation");
  else if (w.kind === "keep") ok = was.filter((f) => f.startsWith(w.rule)).every((f) => now.includes(f));
  else ok = out.status !== "certified";
  return ok ? { ok } : { ok, why: `${w.kind}: ${out.status} ${shown}` };
}

/**
 * Rows may not vanish (endstage-report.md 1a, rule 7): every row at `base` stays with every field the same, both ways, or `retired`
 * lists `{ id, adr, why }` (and, for a row still there, `fields`: each changed field) and `decisions` (docs/decisions.md's text) has
 * that ADR's anchor. Returns the failures.
 */
export function vanishedRows(base, rows, retired, decisions) {
  const out = [];
  const reasons = new Map(retired.map((entry) => [entry.id, entry]));
  for (const [id, was] of Object.entries(base)) {
    const now = rows[id];
    // Every field, both ways (review item 8): `rule`, `closedBy`, `closedAt`, `before` and `proved` decide how a row is judged.
    const keys = [...new Set([...Object.keys(was), ...Object.keys(now ?? {})])];
    const changed = now ? keys.filter((key) => JSON.stringify(now[key]) !== JSON.stringify(was[key])) : null;
    if (changed && !changed.length) continue;
    const reason = reasons.get(id);
    // A retired row for a row still there names the fields it excuses; any other changed field fails.
    if (changed && reason) {
      const named = Array.isArray(reason.fields) ? reason.fields : [];
      const unnamed = changed.filter((key) => !named.includes(key));
      if (unnamed.length) {
        out.push(`${id}: ${unnamed.join(", ")} changed, not named by its retired row's fields`);
        continue;
      }
    }
    const anchor = /^ADR (\d{4})$/.exec(reason?.adr ?? "")?.[1];
    if (!reason?.why || !anchor || !decisions.includes(`<a id="adr-${anchor}"></a>`)) out.push(`${id}: ${now ? "changed" : "removed"} with no retired row naming an ADR in docs/decisions.md`);
  }
  return out;
}
