import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";

// A stop row (open-rows.mjs, R-stop-named-U) clears on a named U in its statement. That U can be raised by a holder the generator adds, not by the
// site: remove the holder and the same site may be certified while Chrome fires (the coordinator's audit, ADR 0815). So every stop row with a holder
// must keep a finding with the holder removed. Every source file of the program is copied and transformed (coordinator, Part 17: the 43 multi-file
// rows were unchecked): the holder may sit in any of them, and a file the transform leaves alone goes in unchanged. A program with no App.tsx (nothing
// to mount) is reported unchecked, with a count, never as passing.

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
 * The stop rows with a holder, sorted: `checked` programs (each with every source file, the holder removed), `unmatched` (no file matched: fails the
 * check), `unchecked` (no App.tsx: counted, never passing). `rows`: `{ id, grid, variant }`. `read(row)`: `{ files }`, every source file's text by name.
 */
export function stopPlan(rows, read) {
  const plan = { checked: [], unmatched: [], unchecked: [] };
  for (const row of rows) {
    if (!row.variant || !HOLDER_FREE[row.variant]) continue;
    const { files } = read(row);
    if (typeof files?.["App.tsx"] !== "string") { plan.unchecked.push(row.id); continue; }
    const freed = Object.fromEntries(Object.entries(files).map(([name, text]) => [name, holderFree(row.variant, text)]));
    if (Object.values(freed).every((text) => text === null)) { plan.unmatched.push(row.id); continue; }
    plan.checked.push({ ...row, files: Object.fromEntries(Object.entries(files).map(([name, text]) => [name, freed[name] ?? text])) });
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

/** A source file of a program: what the checker reads beside App.tsx (any JS or TS module; App.run.txt and the oracle's JSON are not). */
export const isSourceFile = (name) => /\.[cm]?[jt]sx?$/.test(name);

/** Every source file of the case folder by name, from disk, so no file of a multi-file program is left out of the check. */
export function readStopSource(folder) {
  return { files: Object.fromEntries(readdirSync(folder).filter(isSourceFile).sort().map((name) => [name, readFileSync(join(folder, name), "utf8")])) };
}
