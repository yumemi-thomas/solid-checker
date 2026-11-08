import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
const here = dirname(fileURLToPath(import.meta.url));
import { appFrames, programFrames } from "./frames.mjs";

describe("appFrames", () => {
  const stack = [
    "Error: diagnostic REACTIVE_WRITE_IN_OWNED_SCOPE",
    "    at file:///work/node_modules/@solidjs/signals/dist/dev.js:120:9",
    "    at f1 (/work/cases/fuzz-12/App.tsx:9:14)",
    "    at /work/cases/fuzz-12/App.tsx:14:5",
    "    at /work/cases/fuzz-120/App.tsx:3:3",
    "    at C1 (/work/cases/fuzz-12/App.tsx:20:3)",
    "    at runTest (/work/node_modules/vitest/dist/runner.js:1:1)",
  ].join("\n");
  it("returns the App.tsx lines of this case, innermost first", () => {
    expect(appFrames("fuzz-12", stack)).toEqual([
      { file: "App.tsx", line: 9, column: 14 }, { file: "App.tsx", line: 14, column: 5 }, { file: "App.tsx", line: 20, column: 3 },
    ]);
  });
  it("does not confuse an id with another that shares its prefix", () => {
    expect(appFrames("fuzz-1", stack)).toEqual([]);
    expect(appFrames("fuzz-120", stack)).toEqual([{ file: "App.tsx", line: 3, column: 3 }]);
  });
  it("tolerates an empty or missing stack", () => {
    expect(appFrames("x", "")).toEqual([]);
    expect(appFrames("x", undefined)).toEqual([]);
  });
});

describe("programFrames (reviews R66 and its follow-up)", () => {
  const stack = [
    "Error: Context can only be accessed under a reactive root.",
    "    at file:///work/node_modules/@solidjs/signals/dist/dev.js:120:9",
    "    at read (/work/cases/fuzz-12/helpers.ts:4:3)",
    "    at js (/work/cases/fuzz-12/helpers.js:5:3)",
    "    at nested (/work/cases/fuzz-12/sub/helpers.ts:6:3)",
    "    at pkg (/work/cases/fuzz-12/node_modules/fz-lib/index.js:1:1)",
    "    at /work/cases/fuzz-12/App.tsx:14:5",
    "    at /work/cases/fuzz-120/helpers.ts:3:3",
    "    at /work/cases/xfuzz-12/helpers.ts:3:3",
  ].join("\n");
  it("keeps every file of this case at any depth and extension, App.tsx with its position; never a package frame, the case's own node_modules included", () => {
    expect(programFrames("fuzz-12", stack)).toEqual([{ file: "helpers.ts" }, { file: "helpers.js" }, { file: "sub/helpers.ts" }, { file: "App.tsx", line: 14, column: 5 }]);
    expect(programFrames("fuzz-120", stack)).toEqual([{ file: "helpers.ts" }]);
    expect(programFrames("x", undefined)).toEqual([]);
  });
  it("the Chrome host reads a logged error's frames with the same pattern (chrome-host/runner.tsx caseFrames)", () => {
    const pattern = (text) => /const match = (\/\^\(\[\^:.*?\/)\.exec/.exec(text)?.[1];
    const fast = pattern(readFileSync(join(here, "frames.mjs"), "utf8"));
    expect(fast).toBeTruthy();
    expect(pattern(readFileSync(join(here, "../chrome-host/runner.tsx"), "utf8"))).toBe(fast);
  });
});
