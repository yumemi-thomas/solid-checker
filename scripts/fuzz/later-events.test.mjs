import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { MAIN_SOURCE } from "./checker.mjs";

// ADR 1099: after the settled phase the harness sends the program's own nodes outside #root (and the window's error listeners) the
// later events a user or the page would: the oracle in a page module of its own (`laterModule`), the fuzzer's hosts at the same moment.
const here = dirname(fileURLToPath(import.meta.url));
const read = (path) => readFileSync(join(here, path), "utf8");
const oracle = read("../runtime-oracle.mjs");
const hosts = { chrome: read("chrome-host/runner.tsx"), fast: read("fast/runner.tsx") };
const LATER_TYPES = ["reset", "formdata", "invalid", "beforetoggle"];

describe("the later events (ADR 1099)", () => {
  it("are not in the entry the checker reads: they come from outside the program", () => {
    expect(MAIN_SOURCE).not.toContain("sc-later-error");
    expect(MAIN_SOURCE).not.toContain("document.body.children");
    // The oracle writes them as a page module beside the halt marker's, which the case's tsconfig does not list.
    expect(oracle).toContain('writeFileSync(join(dir, "sc-later.ts"), laterModule)');
    expect(oracle).toContain('["sc-phase.ts", "sc-later.ts"]');
  });
  it("are the same events in the oracle and both hosts, with the error the harness's own listeners skip", () => {
    const module = oracle.slice(oracle.indexOf("const laterModule = `"), oracle.indexOf("`;", oracle.indexOf("const laterModule = `")));
    for (const type of LATER_TYPES) expect(module).toContain(`"${type}"`);
    expect(module).toContain('message: "sc-later-error"');
    expect(oracle).toContain('if (event.message !== "sc-later-error") mark();');
    for (const [name, text] of Object.entries(hosts)) {
      expect(text, name).toContain(`const LATER_TYPES = ${JSON.stringify(LATER_TYPES).replaceAll('"', "'").replaceAll(",", ", ")};`);
      expect(text, name).toContain("const LATER_ERROR = 'sc-later-error';");
      expect(text, name).toContain("if (event.message === LATER_ERROR) return;");
      // Once, after the event passes and the final pause, before the plan's edits and clicks (the oracle's #ready "settled").
      const later = text.indexOf("for (const element of outside())");
      expect(later, name).toBeGreaterThan(text.indexOf("await pause(settings.finalMs);"));
      expect(later, name).toBeLessThan(text.indexOf("phase = 'plan';"));
    }
  });
  it("skip the harness's own nodes", () => {
    const ids = ["root", "ready", "sc-halted", "sc-hits", "sc-phase", "oracle-behaviour"];
    for (const text of [oracle, ...Object.values(hosts)]) for (const id of ids) expect(text).toMatch(new RegExp(`harness(?:Ids)? = new Set\\(\\[[^\\]]*["']${id}["']`));
  });
});
