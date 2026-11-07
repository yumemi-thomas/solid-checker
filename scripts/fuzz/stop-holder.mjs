import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";

// A stop row (open-rows.mjs, R-stop-named-U) clears on a named U in its statement. That U can be raised by a holder the generator adds, not by the
// site: remove the holder and the same site may be certified while Chrome fires (the coordinator's audit, ADR 0815). So every stop row with a holder
// must keep a finding with the holder removed. A row a file other than App.tsx holds is reported unchecked, with a count, never as passing.

/** The variants whose rows may stop, and each one's holder-free form: the site the holder wraps, written directly. */
export const HOLDER_FREE = {
  "installed-set-reflect": [/\(\(\) => \{ const t: any = \{\}; Reflect\.defineProperty\(t, "a", \{ set: (.+?) \}\); t\.a = 7; \}\)\(\)/g, "$1(7)"],
  "installed-set-alias": [/\(\(\) => \{ const t: any = \{\}; const dp = Object\.defineProperty; dp\(t, "a", \{ set: (.+?) \}\); t\.a = 7; \}\)\(\)/g, "$1(7)"],
  "store-proto-merge": [/merge\(\(\{ __proto__: (.+?) \}\) as any, \{ w: 2 \}\)\.v/g, "$1.v"],
  "store-proto-literal": [/\(\(\{ __proto__: (.+?) \}\) as any\)\.v/g, "$1.v"],
  "store-proto-string": [/\(\(\{ "__proto__": (.+?) \}\) as any\)\.v/g, "$1.v"],
  "store-proto-member": [/\(\(\) => \{ const t: any = \{ a: 1 \}; t\.__proto__ = (.+?); return t\.v; \}\)\(\)/g, "$1.v"],
  "proto-key-handed": [/\(\(\) => \{ const t: any = \{ a: 1 \}; const put = \(k: string, v: unknown\) => \{ t\[k\] = v; \}; put\("__proto__", (.+?)\); t\.x = 7; \}\)\(\)/g, "$1.x = 7"],
  "proto-member": [/\(\(\) => \{ const t: any = \{ a: 1 \}; t\.__proto__ = (.+?); t\.x = 7; \}\)\(\)/g, "$1.x = 7"],
  "proto-literal": [/\(\(\{ __proto__: (.+?) \}\) as any\)\.x = 7/g, "$1.x = 7"],
  "proto-method-this": [/\(\(\) => \{ const t: any = \{ install\(\) \{ Object\.setPrototypeOf\(this, (.+?)\); \} \}; t\.install\(\); t\.x = 7; \}\)\(\)/g, "$1.x = 7"],
};
export const STOP_VARIANTS = Object.keys(HOLDER_FREE);

/** `text` with `variant`'s holder removed, or null when nothing matched: the generator changed, and the check must not pass quietly. */
export function holderFree(variant, text) {
  const [pattern, replacement] = HOLDER_FREE[variant];
  const next = text.replace(pattern, replacement);
  return next === text ? null : next;
}

/**
 * The stop rows with a holder, sorted: `checked` programs, `unmatched` (fails the check), `unchecked` (a file beside App.tsx: counted, never passing).
 * `rows`: `{ id, grid, variant }`. `read(row)`: `{ text, others }`, the App.tsx text and how many other source files the program has.
 */
export function stopPlan(rows, read) {
  const plan = { checked: [], unmatched: [], unchecked: [] };
  for (const row of rows) {
    if (!row.variant || !HOLDER_FREE[row.variant]) continue;
    const { text, others } = read(row);
    if (others > 0) { plan.unchecked.push(row.id); continue; }
    const free = holderFree(row.variant, text);
    if (free === null) plan.unmatched.push(row.id);
    else plan.checked.push({ ...row, text: free });
  }
  return plan;
}

/** The problems of a plan, given the checker's findings by id (null when the run failed). No finding at all is a certified program: the U was the holder's. */
export function stopProblems(plan, findings) {
  return [
    ...plan.unmatched.map((id) => `${id}: the holder-free transform matched nothing`),
    ...plan.checked.filter(({ id }) => findings.get(id) === null || findings.get(id) === undefined).map(({ id }) => `${id}: the checker gave no result`),
    ...plan.checked.filter(({ id }) => findings.get(id)?.length === 0).map(({ id }) => `${id}: certified with the holder removed, so its U was the holder's`),
  ];
}

/** Discover the program files from disk so a multi-file case cannot pass as a single-file check. */
export function readStopSource(folder) {
  return { text: readFileSync(join(folder, "App.tsx"), "utf8"), others: readdirSync(folder).filter((name) => /\.[cm]?[jt]sx?$/.test(name) && name !== "App.tsx").length };
}
