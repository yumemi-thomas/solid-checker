import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { buildCase } from "./emit.mjs";
import { GRIDS } from "./focus.mjs";
import { generateProgram } from "./generate.mjs";
import { generateLegal, legalCells, legalId, legalShapes, pairLegalCells, recallLegalShapes } from "./legal.mjs";
import { cellList, censusCells, censusShapes, combined, generateShape, gridShapes, mountsOption, pairShapes, recallShapes, shapeId } from "./matrix.mjs";
import { sourcesOption } from "./misuse.mjs";
import { CALLS, TEMPLATES } from "./ssr.mjs";
import { INSTALL, compileForms, stateSites, surfaceRows, typescript } from "./surface.mjs";
import { pinnedHeadlessShell } from "../playwright-browser.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const read = (name) => JSON.parse(readFileSync(join(here, name), "utf8"));
const surface = read("surface.json");
const state = read("surface-state.json");
const dom = read("surface-dom.json");
const COMPILER_README = "287437dae35969488d7cffcc2509fb0dbb4df9c339d6920c58016c6d895fb17b";

// A grid's ids for one code, as rediscovery.test.mjs reads them (`gridIds` there): an id is its cell and shape, so no program is generated.
function gridIds(grid, code, { full = false } = {}) {
  const [script, ...args] = GRIDS[grid];
  const value = (name) => { const at = args.indexOf(name); return at < 0 ? undefined : args[at + 1]; };
  const mounts = mountsOption(value("--mounts")), sources = sourcesOption(value("--sources")), combine = args.includes("--combine");
  if (args.includes("--census")) return censusCells({ codes: [code] }).flatMap((cell) => censusShapes(code).map((shape) => shapeId(cell, shape)));
  if (args.includes("--pairs")) {
    if (script === "legal.mjs") return pairLegalCells({ codes: [code] }).flatMap((cell) => pairShapes(code, cell.context, { legal: true }).map((shape) => legalId(cell, { shape })));
    return cellList({ codes: [code] }).flatMap((cell) => pairShapes(code, cell.context).map((shape) => shapeId(cell, shape)));
  }
  if (args.includes("--recall")) {
    if (script === "legal.mjs") return legalCells({ codes: [code], recall: true }).flatMap((cell) => recallLegalShapes(code, cell.context, { lean: !full }).map((shape) => legalId(cell, { shape })));
    return cellList({ codes: [code], recall: true }).flatMap((cell) => recallShapes(code, cell.context, { lean: !full }).map((shape) => shapeId(cell, shape)));
  }
  if (script === "legal.mjs") return legalCells({ codes: [code] }).flatMap((cell) => combined(legalShapes(code, mounts, sources, cell.context), combine).map((shape) => legalId(cell, { shape })));
  return cellList({ codes: [code] }).flatMap((cell) => combined(gridShapes(code, cell.context, mounts, sources), combine).map((shape) => shapeId(cell, shape)));
}

const isLegal = (shape) => shape.grid.startsWith("lg-");
const axesOf = ({ grid, variant, binding, layer, mount, source, callback, combo }) => ({ variant: variant ?? undefined, binding, layer, ...(mount ? { mount } : {}), ...(source ? { source } : {}), ...(callback ? { callback } : {}), ...(combo ? { combo } : {}), ...(grid === "mx-census" ? { census: true } : {}) });
const idOf = (shape) => { const cell = { code: shape.code, context: shape.context }; return isLegal(shape) ? legalId(cell, { shape: axesOf(shape) }) : shapeId(cell, axesOf(shape)); };

const texts = new Map();
/** The program's own source (no imports' lines for a needle), generated once per claimed program: at most a few hundred, never a grid. */
function programOf(shape) {
  const key = JSON.stringify(shape);
  if (texts.has(key)) return texts.get(key);
  let built;
  if (shape.generator === "generate") built = buildCase(generateProgram(shape.seed, shape.profile ? { profile: shape.profile } : {}), { id: "surface" });
  else if (shape.generator === "ssr") built = { files: { "App.tsx": `${TEMPLATES.map((template) => template.text).join("\n")}\n${Object.values(CALLS).map((one) => one.call('"x"')).join("\n")}` } };
  else {
    const cell = { code: shape.code, context: shape.context };
    built = (isLegal(shape) ? generateLegal({ seed: 1 }, cell, { index: 0, shape: axesOf(shape) }) : generateShape({ seed: 1 }, cell, axesOf(shape), 0)).built;
  }
  const source = Object.entries(built.files).filter(([name]) => /\.(tsx?|jsx?)$/.test(name) && !name.endsWith("App.run.txt")).map(([, text]) => text).join("\n");
  const body = source.replace(/^\s*import\s[^\n]*$/gm, "");
  const made = { source, body };
  texts.set(key, made);
  return made;
}

// The AST kinds of a program: `lang/<kind>` is a node in it, by TypeScript's parser and not by a token.
const ts = typescript();
const CANONICAL = { FirstStatement: "VariableStatement", FirstLiteralToken: "NumericLiteral", FirstAssignment: "EqualsToken", FirstBinaryOperator: "LessThanToken", FirstCompoundAssignment: "PlusEqualsToken", FirstTemplateToken: "NoSubstitutionTemplateLiteral" };
function kindsOf(made) {
  if (made.kinds) return made.kinds;
  made.kinds = new Set();
  const walk = (node) => { const kind = ts.SyntaxKind[node.kind]; made.kinds.add(CANONICAL[kind] ?? kind); ts.forEachChild(node, walk); };
  walk(ts.createSourceFile("a.tsx", made.source, ts.ScriptTarget.ESNext, false, ts.ScriptKind.TSX));
  return made.kinds;
}

const { rows, details, typed, holders } = surfaceRows();

/** The program holds the row: the import and a use for an export, the node for a language row, the needle for the rest. */
function holds(key, row, made) {
  if (key.startsWith("export/") && !row.needle) {
    const name = key.slice(key.lastIndexOf("/") + 1);
    const entries = holders[key] ?? [key.slice("export/".length, key.lastIndexOf("/"))];
    const imported = entries.some((entry) => new RegExp(`import\\s*\\{[^}]*(?<![\\w$])${name.replaceAll("$", "\\$")}(?![\\w$])[^}]*\\}\\s*from\\s*"${entry}"`).test(made.source));
    return imported && new RegExp(`(?<![\\w$.])${name.replaceAll("$", "\\$")}(?![\\w$])`).test(made.body);
  }
  if (key.startsWith("lang/")) return kindsOf(made).has(key.slice("lang/".length));
  if (key.startsWith("entry/")) return made.source.includes(`from "${key.slice("entry/".length)}"`);
  return row.needle ? new RegExp(row.needle).test(row.shape?.generator === "ssr" ? made.source : made.body) : false;
}

const answers = (row) => [row.shape, row.reason, row.missing].filter(Boolean).length;
const NOT_A_REASON = /\b(not yet|todo|tbd|for now|later on|to be done|someday|out of time)\b/i;

/** Claims of `table` whose program is gone from its grid, or holds nothing of the row. */
function unprovenShapes(table) {
  const ids = new Map();
  const problems = [];
  for (const [key, row] of Object.entries(table)) {
    const { shape } = row;
    if (!shape) continue;
    if (shape.cell) { if (!cellList({ codes: [shape.cell], recall: true }).length && !censusCells({ codes: [shape.cell] }).length) problems.push(`${key}: no cell of ${shape.cell}`); continue; }
    if (shape.grid) {
      const list = `${shape.grid}\0${shape.code}\0${Boolean(shape.full)}`;
      if (!ids.has(list)) ids.set(list, new Set(gridIds(shape.grid, shape.code, { full: Boolean(shape.full) })));
      if (!ids.get(list).has(idOf(shape))) { problems.push(`${key}: ${shape.grid} no longer generates ${idOf(shape)}`); continue; }
    }
    const needle = shape.needle ? { ...row, needle: shape.needle } : row;
    if (!holds(key, needle, programOf(shape))) problems.push(`${key}: ${shape.grid ? idOf(shape) : shape.generator} holds nothing of it`);
  }
  return problems;
}

describe("surface.json: every row of the real surface has an answer", () => {
  it("lists every row the pinned bytes hold, and no row they lost", () => {
    const listed = new Set(Object.keys(surface));
    // A new export, option, overload, code, DOM method or syntax kind fails here by name: the question nobody asked about `.focus()`.
    expect(rows.filter((row) => !listed.has(row)), "rows of the bytes with no entry in surface.json").toEqual([]);
    const real = new Set(rows);
    expect([...listed].filter((row) => !real.has(row)), "entries the bytes no longer hold").toEqual([]);
  });

  it("the `frames` client's code is a row", () => {
    expect(rows).toContain("code/FRAME_MARKER_CORRUPTED");
  });

  it("answers each row exactly once: a shape, a reason, or `missing` with its rank", () => {
    expect(Object.entries(surface).filter(([, row]) => answers(row) !== 1).map(([key]) => key)).toEqual([]);
    expect(Object.entries(surface).filter(([, row]) => row.missing && !(Number.isInteger(row.rank) && row.rank >= 1 && row.axis)).map(([key]) => key)).toEqual([]);
  });

  it("a reason says why, and what it leans on still holds", () => {
    const entries = Object.entries(surface).filter(([, row]) => row.reason);
    expect(entries.filter(([, row]) => row.reason.length < 30 || NOT_A_REASON.test(row.reason)).map(([key]) => key)).toEqual([]);
    // `untyped`: importing it is TS2305. A declaration appearing makes the row a question again.
    const declared = (key) => { const at = key.lastIndexOf("/"), name = key.slice(at + 1); return (holders[key] ?? [key.slice("export/".length, at)]).some((entry) => typed[entry]?.includes(name)); };
    expect(entries.filter(([key, row]) => row.untyped && declared(key)).map(([key]) => key)).toEqual([]);
    const forms = compileForms();
    expect(entries.filter(([key, row]) => row.via && !forms[row.via]?.helpers?.includes(key.slice(key.lastIndexOf("/") + 1))).map(([key]) => key), "lowered by that form").toEqual([]);
    expect(entries.filter(([key, row]) => row.refused && forms[key.slice("jsx/form/".length)]?.ok !== false).map(([key]) => key), "refused by the compiler").toEqual([]);
  });

  it("a measured reason is what the probe measured, on the pinned Chrome", () => {
    expect(dom.chromiumHeadlessShell, "the Playwright pin moved: rerun surface-dom-probe.mjs").toBe(pinnedHeadlessShell().revision);
    const quiet = (state) => state?.args && !state.events?.length && !state.callback;
    const wrong = Object.entries(surface).filter(([, row]) => row.measured).filter(([key]) => { const one = dom.results[key.slice("host/".length)]; return !quiet(one?.fresh) || !quiet(one?.prepared); });
    expect(wrong.map(([key]) => key)).toEqual([]);
  });

  it("records what the pinned compiler does with each form", () => {
    const forms = compileForms();
    const drift = Object.entries(surface).filter(([key]) => key.startsWith("jsx/form/")).filter(([key, row]) => {
      const form = forms[key.slice("jsx/form/".length)];
      return form.ok ? JSON.stringify(row.helpers) !== JSON.stringify(form.helpers) : !row.refused;
    });
    expect(drift.map(([key]) => key)).toEqual([]);
    // The compiler has no list of its forms: a changed README on a bump says to reread them (surface-forms.mjs).
    expect(createHash("sha256").update(readFileSync(join(INSTALL, "node_modules/@solidjs/compiler/README.md"))).digest("hex")).toBe(COMPILER_README);
  });

  it("keeps each overload's signature", () => {
    expect(Object.keys(details).filter((key) => surface[key].signature !== details[key])).toEqual([]);
  });

  it("every claimed shape produces a program that holds the row", () => {
    expect(unprovenShapes(surface)).toEqual([]);
  }, 60000);

  it("lists what is missing, and that alone never fails", () => {
    const missing = Object.entries(surface).filter(([, row]) => row.missing);
    const byRank = {};
    for (const [, row] of missing) byRank[row.rank] = (byRank[row.rank] ?? 0) + 1;
    console.log(`surface: ${Object.keys(surface).length} rows, ${Object.values(surface).filter((row) => row.shape).length} shaped, ${Object.values(surface).filter((row) => row.reason).length} with a reason, ${missing.length} missing, by rank ${JSON.stringify(byRank)}`);
    expect(missing.length).toBeGreaterThanOrEqual(0);
  });
});

describe("surface-state.json: every site that changes tracking, owner or strict-window state", () => {
  const sites = stateSites();

  it("lists every site, with its text", () => {
    const keys = new Set(sites.map((site) => site.key));
    expect(sites.filter((site) => !state[site.key]).map((site) => site.key), "a state site with no row: which mount reaches it, and what does the checker assume?").toEqual([]);
    expect(Object.keys(state).filter((key) => !keys.has(key)), "rows the dists no longer hold").toEqual([]);
    expect(sites.filter((site) => state[site.key].text !== site.text).map((site) => site.key), "the site changed").toEqual([]);
  });

  it("records an `untrack`'s label: the argument that arms the window", () => {
    expect(sites.filter((site) => site.primitive === "untrack" && state[site.key].labelled !== site.label).map((site) => site.key)).toEqual([]);
    expect(state["solid.dev.js#observedComponent/untrack#0"].labelled).toBe(true);
    expect(state["solid.dev.js#dynamicCore/untrack#3"].labelled).toBe(false);
    expect(state["web.dev.js#clientOnly/untrack#1"].labelled).toBe(false);
  });

  it("answers each site, with what reaches it and what the checker assumes", () => {
    expect(Object.entries(state).filter(([, row]) => answers(row) !== 1 || !row.reach || !row.assumes || (row.missing && !Number.isInteger(row.rank))).map(([key]) => key)).toEqual([]);
    expect(Object.entries(state).filter(([, row]) => row.reason && (row.reason.length < 30 || NOT_A_REASON.test(row.reason))).map(([key]) => key)).toEqual([]);
  });

  it("every claimed shape produces a program that reaches the site's API", () => {
    expect(unprovenShapes(state)).toEqual([]);
  }, 60000);
});
