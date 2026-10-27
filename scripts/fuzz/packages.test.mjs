import { existsSync, mkdtempSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { buildCase, writeCase } from "./emit.mjs";
import { materializeForChecker } from "./checker.mjs";
import { generateProgram } from "./generate.mjs";
import { stripInstrumentation } from "./hits.mjs";
import { clone, validate, walk } from "./model.mjs";
import { BEHAVIOURS, EXPORT_FORMS, accessOf, packageFiles } from "./package.mjs";
import { SEEDS } from "./packages-seeds.mjs";
import { printProgram, programPackages } from "./print.mjs";

const packages = (seed, plan) => generateProgram(seed, { profile: "packages", packagePlan: plan });
const seeds = Array.from({ length: 400 }, (_, i) => i + 1);
const helperOf = (name, behaviour, exportForm, extra = {}) => ({ k: "helper", name, form: `p:${behaviour}:${exportForm}`, pkg: { behaviour, exportForm, ...extra } });

describe("the packages profile", () => {
  it("is deterministic and only produces valid compositions", () => {
    for (const seed of seeds) {
      expect(validate(packages(seed)), `seed ${seed}`).toEqual([]);
      expect(printProgram(packages(seed)).text).toBe(printProgram(packages(seed)).text);
      expect(programPackages(packages(seed))).toEqual(programPackages(packages(seed)));
    }
  });

  it("covers every behaviour and every export form", () => {
    const helpers = seeds.flatMap((seed) => packages(seed).helpers);
    expect(new Set(helpers.map((h) => h.pkg.behaviour))).toEqual(new Set(Object.keys(BEHAVIOURS)));
    expect(new Set(helpers.map((h) => h.pkg.exportForm))).toEqual(new Set(EXPORT_FORMS));
  });

  it("calls the package in the program and imports solid-js in the package as a peer, never as a copy", () => {
    for (const seed of seeds.slice(0, 60)) {
      const program = packages(seed);
      const built = buildCase(program, { id: `p${seed}` });
      expect(Object.keys(built.packages)).toEqual(["fz-lib"]);
      const manifest = JSON.parse(built.packages["fz-lib"]["package.json"]);
      expect(manifest.peerDependencies).toEqual({ "solid-js": "^2.0.0-rc.17" });
      expect(manifest.dependencies).toBeUndefined();
      expect(manifest.type).toBe("module");
      expect(manifest.exports["."]).toEqual({ types: "./index.d.ts", default: "./index.js" });
      for (const [file, text] of Object.entries(built.packages["fz-lib"])) {
        if (file.endsWith(".js")) for (const line of text.split("\n").filter((l) => l.startsWith("import "))) expect(line, `${file}`).toMatch(/from "solid-js";$/);
      }
      expect(built.text).toMatch(/from "fz-lib";/);
      expect(built.neutral, `seed ${seed}`).toBe(true);
      expect(stripInstrumentation(built.runText)).toBe(built.text);
      expect(built.sites.length).toBeGreaterThan(0);
    }
  });

  it("lays the package out in the case directory as node_modules/fz-lib and names it for the oracle", () => {
    const built = buildCase(packages(7), { id: "p7" });
    const dir = writeCase(mkdtempSync(join(tmpdir(), "fuzz-pkg-")), built);
    expect(existsSync(join(dir, "node_modules/fz-lib/package.json"))).toBe(true);
    expect(existsSync(join(dir, "node_modules/fz-lib/index.js"))).toBe(true);
    expect(JSON.parse(readFileSync(join(dir, "oracle.json"), "utf8")).packages).toEqual(["fz-lib"]);
    expect(JSON.parse(readFileSync(join(dir, "oracle.run.json"), "utf8")).packages).toEqual(["fz-lib"]);
    const install = mkdtempSync(join(tmpdir(), "fuzz-install-"));
    const checked = materializeForChecker(built, install, "p7");
    expect(existsSync(join(checked, "node_modules/fz-lib/index.d.ts"))).toBe(true);
  });

  it("writes no package for a program that imports none", () => {
    const built = buildCase(generateProgram(3, { profile: "mixed" }), { id: "m3" });
    expect(built.packages).toEqual({});
    expect(Object.keys(built.files).some((file) => file.startsWith("node_modules/"))).toBe(false);
    expect(JSON.parse(built.files["oracle.json"]).packages).toBeUndefined();
  });

  it("keeps only the exports a program refers to (shrinking a call removes its export)", () => {
    let program;
    for (const seed of seeds) { program = packages(seed); if (program.helpers.length === 1 && program.helpers[0].pkg.behaviour !== "register") break; }
    expect(Object.keys(programPackages(program))).toEqual(["fz-lib"]);
    const bare = clone(program);
    for (const { node } of walk(bare)) if (node.k === "call" && node.callee.k === "ref" && node.callee.name === bare.helpers[0].name) { node.callee = { k: "ref", name: "nothing" }; }
    expect(programPackages(bare)).toEqual({});
  });

  it("rejects what the validator guards: a fire without a register, two defaults, an unknown behaviour", () => {
    const base = packages(5, SEEDS[2].plan);
    expect(validate(base)).toEqual([]);
    const lonely = clone(base);
    lonely.helpers = lonely.helpers.filter((h) => h.pkg.behaviour !== "register");
    expect(validate(lonely).join()).toMatch(/has no register/);
    const odd = clone(base);
    odd.helpers[0].pkg.behaviour = "nope";
    expect(validate(odd).join()).toMatch(/not a known package export/);
    const twice = clone(base);
    twice.helpers = [helperOf("h1", "inline", "default"), helperOf("h2", "timer", "default")];
    expect(validate(twice).join()).toMatch(/one default export/);
  });

  it("pins the shapes of the brief", () => {
    const shape = (index) => printProgram(packages(200 + index, SEEDS[index].plan), { instrument: false }).text;
    // an inline call with a write in a memo
    expect(shape(0)).toMatch(/createMemo\(\(\) => \{\n\s+inline\d+\(\(\) => \{\n\s+return setW1\(17\);/);
    // a call in setTimeout with a write
    expect(shape(1)).toMatch(/createMemo\(\(\) => \{\n\s+timer\d+\(\(\) => \{\n\s+return setW1\(17\);/);
    // stored, then called from a handler
    expect(shape(2)).toMatch(/register\d+\(\(\) => \{\n\s+return setW1\(17\);[\s\S]*onClick=\{\(\) => \{\n\s+fire\d+\(\);/);
    for (const [index, seed] of SEEDS.entries()) {
      const program = packages(200 + index, seed.plan);
      expect(validate(program), seed.name).toEqual([]);
      expect(buildCase(program, { id: `s${index}` }).sites, seed.name).toHaveLength(1);
    }
  });
});

describe("the generated package", () => {
  it("writes each export form as real ESM with a matching declaration", () => {
    const helpers = [
      helperOf("h1", "inline", "named"), helperOf("h2", "inline", "arrow"), helperOf("h3", "inline", "default"),
      helperOf("h4", "timer", "reexport"), helperOf("h5", "timer", "reexportRenamed"), helperOf("h6", "timer", "star"),
      helperOf("h7", "untrack", "class"), helperOf("h8", "untrack", "static"), helperOf("h9", "untrack", "object"), helperOf("h10", "inline", "namespace"),
    ];
    const files = packageFiles(helpers);
    expect(Object.keys(files).sort()).toEqual(["impl.d.ts", "impl.js", "index.d.ts", "index.js", "package.json"]);
    expect(files["index.js"]).toContain("export function inline1(cb)");
    expect(files["index.js"]).toContain("export const inline2 = (cb) =>");
    expect(files["index.js"]).toContain("export default function h3(cb)");
    expect(files["index.js"]).toContain('export { timer4 } from "./impl.js";');
    expect(files["index.js"]).toContain('export { timer5Impl as timer5Alias } from "./impl.js";');
    expect(files["index.js"]).toContain('export * from "./impl.js";');
    expect(files["index.js"]).toContain("export class Untrack7 {");
    expect(files["index.js"]).toContain("static run(cb)");
    expect(files["index.js"]).toContain("export const lib9 = {");
    expect(files["index.d.ts"]).toContain("export declare function inline1<T>(cb: () => T): T;");
    expect(files["index.d.ts"]).toContain("export declare const inline2: <T>(cb: () => T) => T;");
    expect(files["index.d.ts"]).toContain("export default h3;");
    expect(files["impl.js"]).toContain("setTimeout(cb, 0)");
    expect(files["index.js"]).toMatch(/^import \{ untrack \} from "solid-js";/);
    expect(accessOf(helpers[6])).toEqual({ callee: "new Untrack7().run", importLine: 'import { Untrack7 } from "fz-lib";' });
    expect(accessOf(helpers[9]).callee).toBe("ns.inline10");
  });

  it("shares the state of a register with its fire, in one module", () => {
    const register = helperOf("h1", "register", "class", { store: "array", owned: true, container: "Register1" });
    const fire = helperOf("h2", "fire", "class", { of: "h1", container: "Register1" });
    const files = packageFiles([register, fire]);
    expect(files["index.js"]).toContain("let owner1 = null;");
    expect(files["index.js"]).toContain("const saved1 = [];");
    expect(files["index.js"]).toContain("saved1.push(cb);");
    expect(files["index.js"]).toContain("runWithOwner(owner1, f)");
    expect(files["index.js"]).toMatch(/export class Register1 \{\n  register\(cb\)[\s\S]*\n  fire\(\) \{/);
    expect(files["index.d.ts"]).toContain("fire(): number;");
  });
});
