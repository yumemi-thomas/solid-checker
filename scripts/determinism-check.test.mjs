import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { compareReports, main, planRuns, validOrder } from "./determinism-check.mjs";

const here = fileURLToPath(import.meta.url);
const report = (...findings) => JSON.stringify({ findings });
const finding = (at, message) => ({ id: "SC5003", kind: "uncertifiable", primaryLocation: { path: "App.tsx", startByte: at }, message });

describe("determinism-check", () => {
  it("calls identical output the same and a changed message a move", () => {
    const a = report(finding(1, "async accessor getNode()"), finding(2, "x"));
    expect(compareReports(a, a).same).toBe(true);
    // The accessor a row names is part of the answer: the leak ADR 1128 closed changed nothing else.
    const b = report(finding(1, "async accessor this value"), finding(2, "x"));
    const diff = compareReports(a, b);
    expect(diff.same).toBe(false);
    expect(diff.first).toBe(2);
    expect(diff.other).toBe(2);
    expect(diff.onlyFirst).toEqual([finding(1, "async accessor getNode()")]);
    expect(diff.onlyOther).toEqual([finding(1, "async accessor this value")]);
  });

  it("compares output that is no report as text", () => {
    expect(compareReports("a", "a").same).toBe(true);
    expect(compareReports("a", "b")).toMatchObject({ same: false, first: null, onlyFirst: [] });
  });

  it("accepts the checker's claim orders and nothing else", () => {
    for (const order of ["forward", "reverse", "rotate:7", "shuffle:1"]) expect(validOrder(order)).toBe(true);
    for (const order of ["sideways", "rotate:", "rotate:-1", "shuffle", "Reverse", ""]) expect(validOrder(order)).toBe(false);
  });

  it("plans every worker count and order, repeated, the first run the reference", () => {
    const runs = planRuns(["1", "default"], ["forward", "reverse"], 2);
    expect(runs).toHaveLength(8);
    expect(runs[0]).toEqual({ workers: "1", order: "forward", repeat: 1 });
    expect(runs.at(-1)).toEqual({ workers: "default", order: "reverse", repeat: 2 });
  });

  it("refuses a call it cannot run, with exit 2, before any run", () => {
    const quiet = console.error;
    console.error = () => {};
    try {
      expect(main([])).toBe(2);
      expect(main(["--project", "tsconfig.json", "--orders", "sideways"])).toBe(2);
      expect(main(["--project", "tsconfig.json", "--workers", "0"])).toBe(2);
      expect(main(["--project", "does-not-exist.json"])).toBe(2);
    } finally {
      console.error = quiet;
    }
  });

  it("fails a run that differs from the reference and passes runs that agree", () => {
    const outputs = [report(finding(1, "a")), report(finding(1, "a")), report(finding(1, "b")), report(finding(1, "a"))];
    let call = 0;
    const spawn = () => ({ status: 0, stdout: outputs[call++], stderr: "" });
    const log = console.log;
    const lines = [];
    console.log = (line) => lines.push(line);
    try {
      expect(main(["--project", here, "--checker", here, "--typefacts", here, "--workers", "1", "--orders", "forward,reverse", "--repeats", "2"], spawn)).toBe(1);
    } finally {
      console.log = log;
    }
    expect(lines.filter((line) => line.startsWith("same")).length).toBe(2);
    expect(lines.filter((line) => line.startsWith("DIFF")).length).toBe(1);
    expect(lines.some((line) => line.includes("FAILED"))).toBe(true);
  });

  it("checks every project of a corpus manifest from its app's root, with the manifest's configuration", () => {
    const root = mkdtempSync(join(tmpdir(), "determinism-check-"));
    writeFileSync(join(root, "tsconfig.json"), "{}");
    const manifest = join(root, "manifest.json");
    writeFileSync(manifest, JSON.stringify({ configuration: { runtimeTarget: "browser", runtimeBuild: "development", rendering: "csr" }, projects: [{ app: "a", root: ".", project: "tsconfig.json" }] }));
    const calls = [];
    const spawn = (checker, args, options) => { calls.push({ args, cwd: options.cwd }); return { status: 1, stdout: report(finding(1, "a")), stderr: "" }; };
    const lines = [];
    const code = main(["--corpus", "--manifest", manifest, "--apps-root", root, "--checker", here, "--typefacts", here, "--workers", "1,default", "--orders", "forward", "--repeats", "1"], spawn, (line) => lines.push(line));
    expect(code).toBe(0);
    expect(calls).toHaveLength(2);
    expect(calls[0].cwd).toBe(root);
    expect(calls[0].args).toEqual(["--format", "json", "--project", join(root, "tsconfig.json"), "--runtime-target", "browser", "--runtime-build", "development", "--rendering", "csr"]);
    expect(lines[0]).toMatch(/^same a\/tsconfig\.json: 2 runs/);
  });
});
