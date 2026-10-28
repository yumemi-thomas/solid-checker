import { describe, expect, it } from "vitest";
import { judge } from "./matrix.mjs";

// A second claimed code Chrome reports on the site's lines is judged on its own (ADR 0498): the target's outcome does not see it.
describe("the matrix's secondary codes", () => {
  const site = { line: 5 };
  const runtime = (codes) => ({ diagnostics: codes.map((code) => ({ code, positions: [{ file: "App.tsx", line: 5 }] })) });
  const finding = (rule, kind) => ({ rule, kind, line: 5, endLine: 5 });

  it("counts a missed strict read beside an agreed store write as a false clean", () => {
    const verdict = judge({
      code: "STORE_WRITE_OUTSIDE_SETTER", site, typecheck: { ok: true },
      checker: { ok: true, findings: [finding("no-store-mutation-outside-setter", "violation")] },
      runtime: runtime(["STORE_WRITE_OUTSIDE_SETTER", "STRICT_READ_UNTRACKED"]),
    });
    expect(verdict.outcome).toBe("agree");
    expect(verdict.detail.secondary).toEqual({ STRICT_READ_UNTRACKED: "false-clean" });
  });

  it("is silent when the checker proves the second code too", () => {
    const verdict = judge({
      code: "STORE_WRITE_OUTSIDE_SETTER", site, typecheck: { ok: true },
      checker: { ok: true, findings: [finding("no-store-mutation-outside-setter", "violation"), finding("strict-read-untracked", "violation")] },
      runtime: runtime(["STORE_WRITE_OUTSIDE_SETTER", "STRICT_READ_UNTRACKED"]),
    });
    expect(verdict.detail.secondary).toBeUndefined();
  });

  it("calls an uncertifiable answer for the second code a recall gap", () => {
    const verdict = judge({
      code: "STORE_WRITE_OUTSIDE_SETTER", site, typecheck: { ok: true },
      checker: { ok: true, findings: [finding("no-store-mutation-outside-setter", "violation"), finding("strict-read-untracked", "uncertifiable")] },
      runtime: runtime(["STORE_WRITE_OUTSIDE_SETTER", "STRICT_READ_UNTRACKED"]),
    });
    expect(verdict.detail.secondary).toEqual({ STRICT_READ_UNTRACKED: "recall-gap" });
  });
});
