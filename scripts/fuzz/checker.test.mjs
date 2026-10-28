import { spawn } from "node:child_process";
import { chmodSync, existsSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { MAIN_SOURCE, WORK_NAMESPACE, materializeForChecker, oracleMainSource, parseFindings, runChecker, siteVerdicts } from "./checker.mjs";
import { buildCase } from "./emit.mjs";
import { generateProgram } from "./generate.mjs";
import { DEFAULT_ORACLE_INSTALL } from "./chrome.mjs";
import { DEFAULT_TSC, resolveTsc, tsconfigFor, typecheck } from "./typecheck.mjs";

const oracle = readFileSync(join(dirname(fileURLToPath(import.meta.url)), "../runtime-oracle.mjs"), "utf8");

describe("MAIN_SOURCE", () => {
  it("is the oracle's whole entry, driver included: the checker reads the synchronous dispatch (ADR 0435), as it does in Chrome's run", () => {
    expect(MAIN_SOURCE).toContain("async function exercise()");
    expect(MAIN_SOURCE).toContain("element.dispatchEvent(make(type))");
    expect(MAIN_SOURCE).toContain('document.getElementById("ready")');
    expect(MAIN_SOURCE).not.toMatch(/\$\{|behaviourProbe/);
    expect(oracleMainSource()).toBe(MAIN_SOURCE);
    expect(() => oracleMainSource("no such template")).toThrow(/no longer defines mainSource/);
  });
  it("stays the oracle's entry for a case with no observation (every line is still in runtime-oracle.mjs)", () => {
    for (const line of MAIN_SOURCE.split("\n").map((text) => text.trim()).filter(Boolean)) expect(oracle, line).toContain(line);
  });
});

describe("materializeForChecker", () => {
  it("writes App.tsx, the oracle's main.tsx and a tsconfig with the oracle's options, below the install", () => {
    const install = mkdtempSync(join(tmpdir(), "fuzz-install-"));
    const built = buildCase(generateProgram(11), { id: "fuzz-11" });
    const dir = materializeForChecker(built, install);
    expect(dir).toBe(join(install, ".solid-checker-fuzz", WORK_NAMESPACE, "fuzz-11"));
    expect(readFileSync(join(dir, "App.tsx"), "utf8")).toBe(built.text);
    expect(readFileSync(join(dir, "main.tsx"), "utf8")).toBe(MAIN_SOURCE);
    const tsconfig = JSON.parse(readFileSync(join(dir, "tsconfig.json"), "utf8"));
    expect(tsconfig.files).toEqual(["main.tsx", "App.tsx"]);
    expect(tsconfig.compilerOptions).toMatchObject({ jsx: "preserve", jsxImportSource: "@solidjs/web", strict: true, types: [] });
    for (const key of ["target", "module", "moduleResolution", "jsx", "jsxImportSource", "strict", "noEmit", "skipLibCheck"]) expect(oracle).toContain(key);
  });
});

describe("parseFindings and siteVerdicts", () => {
  const dir = "/work/fuzz-1";
  const analysis = {
    findings: [
      { rule: "no-write-in-pure-computation", id: "SC2001", kind: "violation", primaryLocation: { path: `${dir}/App.tsx`, line: 8, endLine: 8 } },
      { rule: "strict-read-untracked", id: "SC1001", kind: "uncertifiable", primaryLocation: { path: `${dir}/App.tsx`, line: 18, endLine: 22 } },
      { rule: "prefer-show", id: "SC7001", kind: "violation", primaryLocation: { path: `${dir}/App.tsx`, line: 30 } },
      { rule: "x", id: "SC0", kind: "violation", primaryLocation: { path: `${dir}/main.tsx`, line: 3 } },
      { rule: "x", id: "SC0", kind: "violation", primaryLocation: { path: "/elsewhere/App.tsx", line: 3 } },
    ],
  };
  it("keeps only the case's App.tsx findings", () => {
    const findings = parseFindings(analysis, dir);
    expect(findings.map((f) => `${f.rule}:${f.line}-${f.endLine}`)).toEqual(["no-write-in-pure-computation:8-8", "strict-read-untracked:18-22", "prefer-show:30-30"]);
    expect(() => parseFindings({}, dir)).toThrow();
  });
  it("reduces findings to violation / uncertifiable / none per site, leaving ignored style rules out", () => {
    const sites = [{ id: "s1", line: 8, kind: "write" }, { id: "s2", line: 20, kind: "read" }, { id: "s3", line: 30, kind: "write" }, { id: "s4", line: 40, kind: "write" }];
    const verdicts = siteVerdicts(sites, parseFindings(analysis, dir));
    expect(verdicts.map((v) => [v.siteId, v.verdict])).toEqual([["s1", "violation"], ["s2", "uncertifiable"], ["s3", "none"], ["s4", "none"]]);
    expect(verdicts[0].findings[0]).toMatchObject({ anchored: true, id: "SC2001" });
    expect(verdicts[1].findings[0].anchored).toBe(false);
  });
});

describe("runChecker", () => {
  it("reports a missing binary rather than throwing", async () => {
    expect(await runChecker("/nowhere", { bin: "/nonexistent/solid-checker" })).toMatchObject({ ok: false, findings: [] });
  });
  it("runs a fake checker with the oracle's arguments and environment, and parses its JSON", async () => {
    const root = mkdtempSync(join(tmpdir(), "fuzz-fake-"));
    const bin = join(root, "fake-checker.sh");
    writeFileSync(bin, `#!/bin/sh
echo "$@" > "${root}/args"
echo "$SOLID_TYPEFACTS_BIN:$SOLID_CHECKER_DAEMON" > "${root}/env"
printf '{"findings":[{"rule":"strict-read-untracked","id":"SC1001","kind":"violation","primaryLocation":{"path":"${root}/App.tsx","line":4,"endLine":4}}]}'
exit 1
`);
    chmodSync(bin, 0o755);
    const result = await runChecker(root, { bin, typefacts: "/tf/solid-typefacts", env: { PATH: process.env.PATH } });
    expect(result.ok).toBe(true);
    expect(result.findings).toEqual([{ file: "App.tsx", rule: "strict-read-untracked", id: "SC1001", kind: "violation", line: 4, endLine: 4 }]);
    expect(readFileSync(join(root, "args"), "utf8")).toContain(`--project ${root}/tsconfig.json --format json --feedback-facts --runtime-target browser --runtime-build development --rendering csr`);
    expect(readFileSync(join(root, "env"), "utf8").trim()).toBe("/tf/solid-typefacts:0");
  });
  it("is not ok for a crash (exit 2) or malformed output", async () => {
    const root = mkdtempSync(join(tmpdir(), "fuzz-fake-"));
    const crash = join(root, "crash.sh");
    writeFileSync(crash, "#!/bin/sh\necho oops >&2\nexit 2\n"); chmodSync(crash, 0o755);
    expect(await runChecker(root, { bin: crash, env: { PATH: process.env.PATH } })).toMatchObject({ ok: false, exitStatus: 2 });
    const bad = join(root, "bad.sh");
    writeFileSync(bad, "#!/bin/sh\necho not-json\nexit 0\n"); chmodSync(bad, 0o755);
    expect(await runChecker(root, { bin: bad, env: { PATH: process.env.PATH } })).toMatchObject({ ok: false, error: expect.stringMatching(/malformed/) });
  });
});

describe("typecheck", () => {
  it("builds the oracle's tsconfig for the given files", () => {
    expect(JSON.parse(tsconfigFor(["main.tsx", "App.tsx"]))).toMatchObject({ files: ["main.tsx", "App.tsx"], compilerOptions: { jsxImportSource: "@solidjs/web" } });
  });
  it("reports a missing type checker instead of passing the program", async () => {
    const result = await typecheck("/tmp", { tsc: "/nonexistent/tsrs" });
    expect(result).toMatchObject({ ok: false, missing: true });
  });
  it("defaults to tsrs, never to tsc from node_modules", () => {
    expect(DEFAULT_TSC).toMatch(/tsrs\/target\/release\/tsrs$/);
  });
});

// Two runs with the same seed and a different profile name the same program ids (`fuzz-<n>`): their work directories must not be shared.
describe("work directories of concurrent runs", () => {
  const here = dirname(fileURLToPath(import.meta.url));
  const child = (install, text) => new Promise((resolveChild, reject) => {
    const source = `
      import { materializeForChecker } from ${JSON.stringify(join(here, "checker.mjs"))};
      import { readFileSync, existsSync } from "node:fs";
      const dir = materializeForChecker({ files: { "App.tsx": ${JSON.stringify(text)} } }, ${JSON.stringify(install)}, "fuzz-1");
      await new Promise((r) => setTimeout(r, 500));
      console.log(JSON.stringify({ dir, text: readFileSync(dir + "/App.tsx", "utf8") }));`;
    const run = spawn(process.execPath, ["--input-type=module", "-e", source], { stdio: ["ignore", "pipe", "inherit"] });
    let out = "";
    run.stdout.on("data", (chunk) => { out += chunk; });
    run.on("error", reject);
    run.on("close", (status) => (status === 0 ? resolveChild(JSON.parse(out)) : reject(new Error(`child exited ${status}`))));
  });

  it("gives each process its own directory for the same id, which holds its own text and is removed when it exits", async () => {
    const install = mkdtempSync(join(tmpdir(), "fuzz-install-"));
    const [a, b] = await Promise.all([child(install, "export default 1; // closure-escape"), child(install, "export default 2; // containers")]);
    expect(a.dir).not.toBe(b.dir);
    expect(a.text).toContain("closure-escape");
    expect(b.text).toContain("containers");
    expect(existsSync(a.dir)).toBe(false);
    expect(existsSync(b.dir)).toBe(false);
  });
});

// The same, through the command line: needs the oracle install and a type checker.
const installed = existsSync(join(DEFAULT_ORACLE_INSTALL, "node_modules/solid-js/package.json")) && existsSync(resolveTsc());
describe.skipIf(!installed)("two runs with the same seed and different profiles", () => {
  it("both finish and keep distinct work namespaces", async () => {
    const here = dirname(fileURLToPath(import.meta.url));
    const root = mkdtempSync(join(tmpdir(), "fuzz-concurrent-"));
    const work = join(DEFAULT_ORACLE_INSTALL, ".solid-checker-fuzz");
    const before = new Set(existsSync(work) ? readdirSync(work) : []);
    const go = (profile) => new Promise((resolveRun) => {
      const run = spawn(process.execPath, [join(here, "run.mjs"), "--generate-only", "--keep-work", "--seed", "5", "--count", "6", "--profile", profile, "--out", join(root, profile)], { stdio: ["ignore", "ignore", "pipe"] });
      let err = "";
      run.stderr.on("data", (chunk) => { err += chunk; });
      run.on("close", (status) => resolveRun({ status, err }));
    });
    const results = await Promise.all([go("closure-escape"), go("containers")]);
    const made = readdirSync(work).filter((name) => !before.has(name));
    try {
      for (const result of results) { expect(result.err).not.toMatch(/ENOTEMPTY/); expect(result.status).toBe(0); }
      expect(made.length).toBe(2);
    } finally {
      for (const name of made) rmSync(join(work, name), { recursive: true, force: true });
      rmSync(root, { recursive: true, force: true });
    }
  }, 120000);
});
