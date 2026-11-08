import { readdirSync, readFileSync, statSync } from "node:fs";
import { dirname, join, normalize } from "node:path";

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

/** The C1 component App.tsx exports, mounted as App by the check (`mountAsApp`); null when the rename matches nothing. */
export function mountAsApp(text) {
  const next = text.replace("export function C1()", "export function App()");
  return next === text ? null : next;
}

/** Source extensions a local import may name, tried in this order (TypeScript's resolution of a relative specifier, with index files). */
const RESOLVE = ["", ".ts", ".tsx", ".js", ".jsx", ".mts", ".cts", ".mjs", ".cjs", "/index.ts", "/index.tsx", "/index.js", "/index.jsx"];
/** The local specifiers (`./x`, `../x`) a file imports, re-exports or loads, by text. */
const localSpecifiers = (text) => [...text.matchAll(/(?:\bfrom\s*|\bimport\s*\(?\s*|\brequire\s*\(\s*)["'](\.{1,2}\/[^"']*)["']/g)].map((match) => match[1]);
/** Each local import of `files` that names no file of the program: `name -> specifier`. */
export function unresolvedImports(files) {
  const names = new Set(Object.keys(files));
  return Object.entries(files).flatMap(([name, text]) => localSpecifiers(text)
    .filter((specifier) => !RESOLVE.some((suffix) => names.has(normalize(join(dirname(name), specifier + suffix)).replace(/^\.\//, ""))))
    .map((specifier) => `${name} -> ${specifier}`));
}

/**
 * The stop rows with a holder, sorted (review of 88481a542, all four points):
 *   checked    every source file, the holder removed; the file holding the row's site (`row.site.file`) among the transformed ones;
 *   unmatched  no file matched, or the site's file was not transformed: fails;
 *   failed     `{ id, why }`: App.tsx's C1 not found for the mount, a source file in a subdirectory, or a local import naming no file of
 *              the program: fails;
 *   unchecked  no App.tsx: counted, blocking, never passing.
 * `rows`: `{ id, grid, variant, site: { file, line } }`. `read(row)`: `{ files, nested }`, every source file's text by name and the source
 * files found in subdirectories.
 */
export function stopPlan(rows, read) {
  const plan = { checked: [], unmatched: [], failed: [], unchecked: [] };
  for (const row of rows) {
    if (!row.variant || !HOLDER_FREE[row.variant]) continue;
    const { files, nested = [] } = read(row);
    if (typeof files?.["App.tsx"] !== "string") { plan.unchecked.push(row.id); continue; }
    if (nested.length) { plan.failed.push({ id: row.id, why: `source files in a subdirectory: ${nested.join(", ")}` }); continue; }
    const freed = Object.fromEntries(Object.entries(files).map(([name, text]) => [name, holderFree(row.variant, text)]));
    const siteFile = row.site?.file ?? "App.tsx";
    if (Object.values(freed).every((text) => text === null) || freed[siteFile] === null || freed[siteFile] === undefined) { plan.unmatched.push(row.id); continue; }
    const out = Object.fromEntries(Object.entries(files).map(([name, text]) => [name, freed[name] ?? text]));
    const mounted = mountAsApp(out["App.tsx"]);
    if (mounted === null) { plan.failed.push({ id: row.id, why: "App.tsx has no `export function C1()` to mount as App" }); continue; }
    const unresolved = unresolvedImports(out);
    if (unresolved.length) { plan.failed.push({ id: row.id, why: `local imports naming no file of the program: ${unresolved.join(", ")}` }); continue; }
    plan.checked.push({ ...row, files: { ...out, "App.tsx": mounted } });
  }
  return plan;
}

/** A checker finding's file name and start line (its JSON `primaryLocation`). */
const placed = (finding) => ({ file: String(finding.primaryLocation?.path ?? finding.file ?? "").replaceAll("\\", "/").split("/").pop(), line: finding.primaryLocation?.line ?? finding.line });

/**
 * The problems of a plan, given the checker's findings by id (null when the run failed). A pass needs a finding at the row's own site: the file and
 * line of its original U (the transforms are single-line, so no line moves). None there is a gap: the U was the holder's, or sat elsewhere.
 */
export function stopProblems(plan, findings) {
  const atSite = ({ id, site }) => (findings.get(id) ?? []).some((finding) => { const at = placed(finding); return at.file === site.file && at.line === site.line; });
  return [
    ...plan.unmatched.map((id) => `${id}: the holder-free transform matched nothing in the site's file`),
    ...plan.failed.map(({ id, why }) => `${id}: ${why}`),
    ...plan.checked.filter(({ id }) => findings.get(id) === null || findings.get(id) === undefined).map(({ id }) => `${id}: the checker gave no result`),
    ...plan.checked.filter(({ id }) => findings.get(id)?.length === 0).map(({ id }) => `${id}: certified with the holder removed, so its U was the holder's`),
    ...plan.checked.filter((row) => findings.get(row.id)?.length > 0 && !atSite(row)).map(({ id, site }) => `${id}: no finding at its site ${site.file}:${site.line} with the holder removed`),
  ];
}

/** A source file of a program: what the checker reads beside App.tsx (any JS or TS module; App.run.txt and the oracle's JSON are not). */
export const isSourceFile = (name) => /\.[cm]?[jt]sx?$/.test(name);

/** Every source file of the case folder by name, from disk, and every source file in a subdirectory (`nested`), which the check refuses. */
export function readStopSource(folder) {
  const nested = [];
  const walk = (dir, prefix) => { for (const name of readdirSync(dir)) { const path = join(dir, name); if (statSync(path).isDirectory()) walk(path, `${prefix}${name}/`); else if (prefix && isSourceFile(name)) nested.push(`${prefix}${name}`); } };
  for (const name of readdirSync(folder)) if (statSync(join(folder, name)).isDirectory()) walk(join(folder, name), `${name}/`);
  return { files: Object.fromEntries(readdirSync(folder).filter((name) => isSourceFile(name) && statSync(join(folder, name)).isFile()).sort().map((name) => [name, readFileSync(join(folder, name), "utf8")])), nested: nested.sort() };
}
