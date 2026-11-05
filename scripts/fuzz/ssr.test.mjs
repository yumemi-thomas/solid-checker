import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { DEFAULT_FAST_INSTALL } from "./fast.mjs";
import { CALLS, SSR_PROBLEMS, TEMPLATES, compileForSsr, judgeSsr, parseSsrArgs, renderPrograms, ssrPrograms } from "./ssr.mjs";

const installed = existsSync(join(DEFAULT_FAST_INSTALL, "node_modules/@solidjs/web/dist/server.dev.js"));

describe("the SSR programs", () => {
  it("are each template with each declaration, one declaration line each, misuse dropped and legal applied by design", () => {
    const programs = ssrPrograms();
    expect(programs).toHaveLength(TEMPLATES.length * Object.keys(CALLS).length);
    expect(new Set(programs.map((program) => program.id)).size).toBe(programs.length);
    for (const program of programs) {
      const line = program.text.split("\n")[program.site - 1];
      expect(line).toContain(`${CALLS[program.call].imported}(`);
      expect(program.design).toBe(program.kind === "misuse" ? "dropped" : "applied");
      expect(program.text).not.toContain("$CALL");
    }
    expect(TEMPLATES.filter((t) => t.kind === "misuse").length).toBeGreaterThan(0);
    expect(TEMPLATES.filter((t) => t.kind === "legal").length).toBeGreaterThan(0);
    expect(ssrPrograms({ templates: ["l-shell"] }).map((program) => program.id)).toEqual(["ssr-l-shell-header", "ssr-l-shell-status"]);
  });

  it("generates the programs the rediscovery ledger names for its defects", () => {
    const ids = new Set(ssrPrograms().map((program) => program.id));
    const entries = JSON.parse(readFileSync(new URL("./rediscovery.json", import.meta.url), "utf8")).filter((entry) => entry.ssrIds);
    expect(entries.length).toBeGreaterThan(0);
    for (const entry of entries) expect(entry.ssrIds.filter((id) => !ids.has(id))).toEqual([]);
  });

  it("parses its options and refuses unknown ones", () => {
    expect(parseSsrArgs(["--out", "o", "--checker-bin", "b", "--keep-work"])).toEqual({ out: "o", checkerBin: "b", keepWork: true });
    expect(() => parseSsrArgs(["--per-cell", "3"])).toThrow(/unknown or empty option/);
  });
});

describe("judgeSsr", () => {
  const program = { site: 7, design: "dropped" };
  const at = (kind, line = 7) => [{ id: "SC7005", kind, line, endLine: line }];
  it("covers a dropped declaration with any SC7005 finding at its line, and calls silence there a false clean", () => {
    expect(judgeSsr(program, { runtime: { applied: false }, findings: at("uncertifiable"), typecheck: { ok: true } }).outcome).toBe("agree");
    expect(judgeSsr(program, { runtime: { applied: false }, findings: at("uncertifiable", 9), typecheck: { ok: true } }).outcome).toBe("false-clean");
    expect(judgeSsr(program, { runtime: { applied: false }, findings: [{ id: "SC8003", kind: "violation", line: 7, endLine: 7 }], typecheck: { ok: true } }).outcome).toBe("false-clean");
  });

  it("calls any client rule's violation on a server-only program a false positive, whatever the head did", () => {
    const client = [{ id: "SC5001", kind: "violation", line: 3, endLine: 3 }];
    expect(judgeSsr({ site: 7, design: "applied" }, { runtime: { applied: true }, findings: client, typecheck: { ok: true } }).outcome).toBe("false-positive");
    const open = [{ id: "SC5001", kind: "uncertifiable", line: 3, endLine: 3 }];
    expect(judgeSsr({ site: 7, design: "applied" }, { runtime: { applied: true }, findings: open, typecheck: { ok: true } }).outcome).toBe("clean");
  });

  it("never counts an obligation on an applied declaration as a violation", () => {
    const legal = { site: 7, design: "applied" };
    expect(judgeSsr(legal, { runtime: { applied: true }, findings: at("uncertifiable"), typecheck: { ok: true } }).outcome).toBe("uncertifiable");
    expect(judgeSsr(legal, { runtime: { applied: true }, findings: [], typecheck: { ok: true } }).outcome).toBe("clean");
    expect(judgeSsr(legal, { runtime: { applied: true }, findings: at("violation"), typecheck: { ok: true } }).outcome).toBe("false-positive");
  });

  it("flags a runtime answer against the design, a failed render or checker, and keeps a type error out of the verdicts", () => {
    expect(judgeSsr(program, { runtime: { applied: true }, findings: [], typecheck: { ok: true } })).toMatchObject({ outcome: "not-as-designed", detail: { observed: "applied", design: "dropped" } });
    expect(judgeSsr(program, { runtime: { failure: "boom" }, findings: [], typecheck: { ok: true } })).toMatchObject({ outcome: "harness", detail: { why: "boom" } });
    expect(judgeSsr(program, { runtime: { applied: false }, findings: null, typecheck: { ok: true } }).outcome).toBe("harness");
    expect(judgeSsr(program, { runtime: { applied: false }, findings: [], typecheck: { ok: false, errors: ["TS2322"] } }).outcome).toBe("typed-away");
    for (const outcome of ["false-clean", "false-positive", "not-as-designed", "harness"]) expect(SSR_PROBLEMS).toContain(outcome);
    expect(SSR_PROBLEMS).not.toContain("uncertifiable");
  });
});

describe.skipIf(!installed)("the SSR host on the pinned install", () => {
  it("renders every template as designed on rc.17's server build", async () => {
    const work = join(DEFAULT_FAST_INSTALL, ".solid-checker-fuzz-ssr", `test-${process.pid}`);
    rmSync(work, { recursive: true, force: true });
    try {
      const programs = ssrPrograms();
      const cases = programs.map((program) => {
        const dir = join(work, program.id);
        mkdirSync(dir, { recursive: true });
        const module = join(dir, "App.ssr.mjs");
        writeFileSync(module, compileForSsr(program.text, DEFAULT_FAST_INSTALL));
        return { ...program, module };
      });
      const records = await renderPrograms(cases, { install: DEFAULT_FAST_INSTALL });
      for (const program of cases) {
        const record = records.get(program.id);
        expect(record.failure, program.id).toBeUndefined();
        expect(CALLS[program.call].observe(record) ? "applied" : "dropped", program.id).toBe(program.design);
      }
    } finally {
      rmSync(work, { recursive: true, force: true });
    }
  }, 60_000);

  it("leaves a program the host never answered a failure, never a head (the host dies)", async () => {
    const dir = mkdtempSync(join(tmpdir(), "fuzz-ssr-death-"));
    const module = join(dir, "App.ssr.mjs");
    writeFileSync(module, "process.exit(9);\nexport default () => null;\n");
    const records = await renderPrograms([{ id: "dies", module }, { id: "after", module }], { install: DEFAULT_FAST_INSTALL });
    expect(records.get("dies").failure).toMatch(/the SSR host exited 9/);
    expect(records.get("after").failure).toMatch(/the SSR host exited 9/);
  }, 30_000);
});
