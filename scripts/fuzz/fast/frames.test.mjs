import { describe, expect, it } from "vitest";
import { appFrames } from "./frames.mjs";

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
