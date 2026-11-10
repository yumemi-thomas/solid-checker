// Run: bun packages/cli/node_modules/vitest/vitest.mjs run --config packages/cli/vitest.config.mjs scripts/fuzz/primitive-layer.test.mjs
// ADR 0940: the primitive-layer contexts (primitive-layer-shapes.mjs), their measured table, the scan that measures it, and the SC4002 package source.
import { describe, expect, it } from "vitest";
import { buildCase } from "./emit.mjs";
import { generateProgram } from "./generate.mjs";
import { stripInstrumentation } from "./hits.mjs";
import { cellList, censusCells, recallShapes } from "./matrix.mjs";
import { legalCells, recallLegalShapes } from "./legal.mjs";
import { BASE_ONLY_CONTEXTS, SOURCES, SPEC, fireShape, parentSource, sourcesOf } from "./misuse.mjs";
import { validate } from "./model.mjs";
import { PL_CODES, PL_CONTEXT_NAMES, PL_HOSTS, PL_LAYERS, PL_MEASURED, plParts, plNotVariants, plPlacement } from "./primitive-layer-shapes.mjs";
import { kindOf, measuredTable, sameTable, tableText } from "./primitive-layer-scan.mjs";
import { printProgram } from "./print.mjs";

// `census`: any context, whether or not the code has a cell there (the measure is what Solid does).
const program = (code, context, extra = {}) => generateProgram(7, { profile: "misuse", misusePlan: { code, context, binding: "direct", layer: "none", variant: null, census: true, ...extra } });

describe("the primitive-layer contexts", () => {
  it("name every layer in every host, and parse back", () => {
    expect(PL_CONTEXT_NAMES).toHaveLength(PL_LAYERS.length * PL_HOSTS.length);
    for (const layer of PL_LAYERS) for (const host of PL_HOSTS) expect(plParts(`pl-${layer}-${host}`)).toEqual({ layer, host });
    // `owner-null` is a layer of its own, not `owner` in a host called `null-…`.
    expect(plParts("pl-owner-null-tracked")).toEqual({ layer: "owner-null", host: "tracked" });
    expect(plParts("pl-owner-tracked")).toEqual({ layer: "owner", host: "tracked" });
    expect(plParts("pl-untrack-nowhere")).toBeNull();
    expect(plParts("untrack")).toBeNull();
  });
  it("are base-only contexts of the plain shape", () => {
    for (const context of PL_CONTEXT_NAMES) expect(BASE_ONLY_CONTEXTS()).toContain(context);
    const plain = { variant: null, binding: "direct", layer: "none" };
    for (const context of PL_CONTEXT_NAMES) {
      expect(fireShape(context)(plain)).toBe(true);
      expect(fireShape(context)({ ...plain, layer: "closure" })).toBe(false);
      expect(fireShape(context)({ ...plain, mount: "show" })).toBe(false);
    }
  });
  it("print the layer around the site, in the host, with one marked site", () => {
    const text = { untrack: "untrack(", latest: "latest(", ispending: "isPending(", owner: "runWithOwner(owner,", "owner-null": "runWithOwner(null,", root: "createRoot(", flush: "flush(" };
    const host = { tracked: "createTrackedEffect(", settled: "onSettled(", "effect-apply": "createEffect(", "memo-async": "async ()", action: "action(function*", body: "export function C1" };
    for (const layer of PL_LAYERS) for (const where of PL_HOSTS) {
      const p = program("CLEANUP_IN_FORBIDDEN_SCOPE", `pl-${layer}-${where}`);
      expect(validate(p), `${layer}/${where}`).toEqual([]);
      const printed = printProgram(p);
      expect(printed.sites, `${layer}/${where}`).toHaveLength(1);
      expect(printed.text, `${layer}/${where}`).toContain(text[layer]);
      expect(printed.text, `${layer}/${where}`).toContain(host[where]);
      expect(printed.text.indexOf(host[where]), `${layer}/${where}`).toBeLessThan(printed.text.indexOf(text[layer]));
      expect(stripInstrumentation(printed.runText), `${layer}/${where}`).toBe(printed.text);
    }
  });
  it("capture the owner before the await of an async host, so `runWithOwner(owner)` restores it", () => {
    const { text } = printProgram(program("NO_OWNER_CLEANUP", "pl-owner-memo-async"));
    expect(text.indexOf("getOwner()")).toBeGreaterThan(-1);
    expect(text.indexOf("getOwner()")).toBeLessThan(text.indexOf("await"));
    expect(text.indexOf("await")).toBeLessThan(text.indexOf("runWithOwner(owner"));
  });
  it("hand the primitive's own call to the layer: `flush(fn)` is a call of flush with the site in its function", () => {
    const { text } = printProgram(program("FLUSH_IN_ACTION", "pl-flush-action"));
    expect(text).toMatch(/flush\(\(\) => \{\n\s+flush\(\);\n\s+\}\)/);
  });
});

describe("the measured table", () => {
  const names = (code, kind) => plPlacement()[code][kind];
  it("names the ten codes, each with cells of both sides but the read after an await's hosts", () => {
    expect(Object.keys(PL_MEASURED).sort()).toEqual([...PL_CODES].sort());
    for (const code of PL_CODES) expect(Object.keys(PL_MEASURED[code]).every((key) => ["misuse", "legal", "without"].includes(key)), code).toBe(true);
    expect(Object.keys(PL_MEASURED.UNTRACKED_READ_AFTER_AWAIT.misuse)).toEqual(["memo-async"]);
    expect(Object.keys(PL_MEASURED.UNTRACKED_READ_AFTER_AWAIT.legal)).toEqual(["memo-async"]);
  });
  it("holds only contexts that exist, each once, a code's misuse and legal sides disjoint", () => {
    for (const code of PL_CODES) {
      const misuse = names(code, "misuse"), legal = names(code, "legal");
      for (const context of [...misuse, ...legal]) expect(PL_CONTEXT_NAMES, `${code}/${context}`).toContain(context);
      expect(new Set([...misuse, ...legal]).size, code).toBe(misuse.length + legal.length);
    }
  });
  it("lists the layers a leaf owner does not clear, and the ones that catch what Solid only throws", () => {
    // untrack, latest, isPending, flush(fn) and runWithOwner(owner) keep the leaf; runWithOwner(null) leaves it (no owner: NO_OWNER_CLEANUP).
    expect(PL_MEASURED.CLEANUP_IN_FORBIDDEN_SCOPE.misuse.tracked).toEqual(["untrack", "latest", "ispending", "owner", "flush"]);
    expect(PL_MEASURED.NO_OWNER_CLEANUP.misuse.tracked).toEqual(["owner-null"]);
    // `isPending(fn)` catches the throw of a code Solid only throws: f2's `isPending(() => resolve(…))` and a `flush()` in an action are legal.
    expect(PL_MEASURED.RESOLVE_IN_REACTIVE_SCOPE.legal.tracked).toContain("ispending");
    expect(PL_MEASURED.RESOLVE_IN_REACTIVE_SCOPE.misuse.tracked).not.toContain("ispending");
    expect(PL_MEASURED.FLUSH_IN_ACTION.legal.action).toEqual(["ispending"]);
    expect(PL_MEASURED.FLUSH_IN_TRACKED_QUEUE_CALLBACK.legal.tracked).toEqual(["ispending"]);
    // but not the diagnostic Solid emits before it throws.
    expect(PL_MEASURED.CLEANUP_IN_FORBIDDEN_SCOPE.misuse.tracked).toContain("ispending");
  });
  it("puts every cell in the grids, and none in the census", () => {
    const misuse = cellList({ recall: true, codes: PL_CODES }).filter((cell) => plParts(cell.context));
    const legal = legalCells({ recall: true, codes: PL_CODES }).filter((cell) => plParts(cell.context));
    expect(misuse).toHaveLength(PL_CODES.reduce((n, code) => n + names(code, "misuse").length, 0));
    expect(legal).toHaveLength(PL_CODES.reduce((n, code) => n + names(code, "legal").length, 0));
    expect(censusCells({}).filter((cell) => plParts(cell.context))).toEqual([]);
    // no other code lists one
    for (const code of Object.keys(SPEC)) if (!PL_CODES.includes(code)) expect([...(SPEC[code].recallContexts ?? []), ...(SPEC[code].recallLegal ?? [])].filter((context) => plParts(context)), code).toEqual([]);
  });
  it("leaves the effect variant out of a legal cell where the site has no owner (NO_OWNER_EFFECT there)", () => {
    const excluded = plNotVariants();
    expect(excluded.PRIMITIVE_IN_FORBIDDEN_SCOPE["pl-owner-null-tracked"]).toEqual(["effect"]);
    const variants = (code, context) => new Set(recallLegalShapes(code, context).map((shape) => shape.variant));
    expect(variants("PRIMITIVE_IN_FORBIDDEN_SCOPE", "pl-owner-null-tracked")).toEqual(new Set(["memo", "root", "resolve", "until"]));
    expect(variants("PRIMITIVE_IN_FORBIDDEN_SCOPE", "pl-untrack-body")).toEqual(new Set(["memo", "root", "effect", "resolve", "until"]));
  });
  it("takes the plain shape of each variant, and the code's own plVariants beside the base ones", () => {
    for (const cell of cellList({ recall: true, codes: PL_CODES }).filter((one) => plParts(one.context))) {
      for (const shape of recallShapes(cell.code, cell.context)) expect(fireShape(cell.context)(shape), `${cell.code}/${cell.context}`).toBe(true);
    }
    // plVariants: a variant the base grid does not draw, only here.
    expect(SPEC.PRIMITIVE_IN_FORBIDDEN_SCOPE.plVariants).toEqual(["resolve", "until"]);
    expect(SPEC.UNTRACKED_READ_AFTER_AWAIT.plVariants).toEqual(["store-link"]);
    expect(recallShapes("UNTRACKED_READ_AFTER_AWAIT", "pl-latest-memo-async").map((shape) => shape.variant).sort()).toEqual([null, "store-link"].sort());
    expect(recallShapes("CLEANUP_IN_FORBIDDEN_SCOPE", "pl-latest-tracked")).toHaveLength(1);
  });
});

describe("the scan of a probe", () => {
  const row = (code, context, outcome, extra = {}) => ({ code, context, variant: extra.variant ?? null, outcome, detail: extra.detail ?? {}, ...extra.row });
  it("reads what the runtime did: confirmed is misuse, a clean run that reported nothing is legal, anything else is none", () => {
    expect(kindOf(row("A", "pl-untrack-tracked", "false-clean"), undefined)).toBe("misuse");
    expect(kindOf(row("A", "pl-untrack-tracked", "agree"), undefined)).toBe("misuse");
    expect(kindOf(row("A", "pl-untrack-tracked", "not-fired", { detail: { reported: [] } }), { outcome: "clean" })).toBe("legal");
    expect(kindOf(row("A", "pl-untrack-tracked", "not-fired", { detail: { reported: ["REACTIVITY_HALTED"] } }), { outcome: "clean" })).toBe("legal");
    // another defect code is reported there: not a legal program, not this code's cell
    expect(kindOf(row("A", "pl-untrack-tracked", "not-fired", { detail: { reported: ["NO_OWNER_EFFECT"] } }), { outcome: "uncertifiable" })).toBe("none");
    expect(kindOf(row("A", "pl-untrack-tracked", "not-fired"), { outcome: "disturbed" })).toBe("none");
    expect(kindOf(row("A", "pl-untrack-tracked", "not-fired"), { outcome: "not-legal" })).toBe("none");
    // a violation the runtime did not confirm leaves the program legal (the checker is what is wrong)
    expect(kindOf(row("A", "pl-untrack-tracked", "false-positive"), { outcome: "false-positive" })).toBe("legal");
  });
  it("builds the table, listing the variants a cell does not take, and refusing a cell of both sides", () => {
    const code = "PRIMITIVE_IN_FORBIDDEN_SCOPE";
    const mx = [
      row(code, "pl-untrack-tracked", "false-clean", { variant: "memo" }), row(code, "pl-untrack-tracked", "false-clean", { variant: "effect" }),
      row(code, "pl-owner-null-body", "not-fired", { variant: "memo" }), row(code, "pl-owner-null-body", "not-fired", { variant: "effect", detail: { reported: ["NO_OWNER_EFFECT"] } }),
      row(code, "pl-root-tracked", "not-fired", { variant: "memo" }),
      row(code, "pl-latest-body", "false-clean", { variant: "memo" }), row(code, "pl-latest-body", "not-fired", { variant: "effect" }),
    ];
    const lg = [
      row(code, "pl-owner-null-body", "clean", { variant: "memo" }), row(code, "pl-owner-null-body", "uncertifiable", { variant: "effect" }),
      row(code, "pl-root-tracked", "disturbed", { variant: "memo" }),
      row(code, "pl-latest-body", "clean", { variant: "effect" }),
    ];
    const { table, unresolved } = measuredTable(mx, lg);
    expect(table[code].misuse).toEqual({ tracked: ["untrack"] });
    expect(table[code].legal).toEqual({ body: ["owner-null"] });
    expect(table[code].without).toEqual({ body: { "owner-null": ["effect"] } });
    expect(unresolved).toEqual([expect.stringContaining("pl-latest-body")]);
  });
  it("prints the table as the module's own text, and the module's table is a fixed point", async () => {
    const { table } = measuredTable([], []);
    expect(table).toEqual({});
    const text = tableText(PL_MEASURED);
    const loaded = await import(`data:text/javascript,${encodeURIComponent(text)}`);
    expect(sameTable(loaded.PL_MEASURED, PL_MEASURED)).toBe(true);
    expect(sameTable({ a: { x: ["1"] } }, { a: { x: ["2"] } })).toBe(false);
  });
});

describe("the package source (SC4002, ADR 0940)", () => {
  it("is a source of CONTEXT_NOT_FOUND's plain variant only, grid-only, and no parent's", () => {
    expect(SOURCES).toContain("package-export");
    expect(sourcesOf("CONTEXT_NOT_FOUND", null, "direct", "body")).toContain("package-export");
    expect(sourcesOf("CONTEXT_NOT_FOUND", "default-undefined", "direct", "body")).not.toContain("package-export");
    for (const code of Object.keys(SPEC)) if (code !== "CONTEXT_NOT_FOUND" && !SPEC[code].typedAway) expect(sourcesOf(code, SPEC[code].variants?.[0] ?? null, SPEC[code].bindings[0], SPEC[code].contexts[0] ?? "body"), code).not.toContain("package-export");
    expect(parentSource("package-export")).toBe(false);
  });
  it("imports the dependency's export, ships the dependency without a default, and declares no context of its own", () => {
    const built = buildCase(program("CONTEXT_NOT_FOUND", "memo", { source: "package-export", binding: "wrapper" }), { id: "c1" });
    expect(built.files["App.tsx"]).toContain('import { PkgContext } from "fz-ctx";');
    expect(built.files["App.tsx"]).toContain("useContext(PkgContext)");
    expect(built.files["App.tsx"]).not.toContain("createContext");
    expect(built.files["node_modules/fz-ctx/index.js"]).toContain("export const PkgContext = createContext();");
    expect(built.files["node_modules/fz-ctx/index.d.ts"]).toContain("export declare const PkgContext: Context<number>;");
    expect(JSON.parse(built.files["oracle.json"]).packages).toEqual(["fz-ctx"]);
  });
  // ADR 0943: the dependency's own hook reads the context, so the read is in the package's source and the program only calls the hook.
  it("has a hook form: the dependency's hook is called with no provider above, the read is in the package, the direct binding only", () => {
    expect(SOURCES).toContain("package-hook");
    expect(sourcesOf("CONTEXT_NOT_FOUND", null, "direct", "body")).toContain("package-hook");
    expect(sourcesOf("CONTEXT_NOT_FOUND", null, "alias", "body")).not.toContain("package-hook");
    expect(sourcesOf("CONTEXT_NOT_FOUND", "default-undefined", "direct", "body")).not.toContain("package-hook");
    expect(sourcesOf("CONTEXT_NOT_FOUND", null, "direct", "body")).toContain("package-export");
    expect(parentSource("package-hook")).toBe(false);
    const built = buildCase(program("CONTEXT_NOT_FOUND", "memo", { source: "package-hook", binding: "direct" }), { id: "c1h" });
    expect(built.files["App.tsx"]).toContain('import { usePkgContext } from "fz-ctx";');
    expect(built.files["App.tsx"]).toContain("usePkgContext()");
    expect(built.files["App.tsx"]).not.toContain("useContext(");
    expect(built.files["App.tsx"]).not.toContain("createContext");
    expect(built.files["node_modules/fz-ctx/index.js"]).toContain("export function usePkgContext() {");
    expect(built.files["node_modules/fz-ctx/index.js"]).toContain("return useContext(PkgContext);");
    expect(built.files["node_modules/fz-ctx/index.d.ts"]).toContain("export declare function usePkgContext(): number;");
    expect(JSON.parse(built.files["oracle.json"]).packages).toEqual(["fz-ctx"]);
  });
  it("is not drawn by a combination, and leaves a local program as it was", () => {
    const local = buildCase(program("CONTEXT_NOT_FOUND", "memo"), { id: "local" });
    expect(local.files["App.tsx"]).toContain("const Ctx = createContext<number>();");
    expect(Object.keys(local.packages)).toEqual([]);
  });
});
