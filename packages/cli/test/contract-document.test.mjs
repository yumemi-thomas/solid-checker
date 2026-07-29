import assert from "node:assert/strict";
import test from "node:test";

import {
  expandContract,
  normalizeContract
} from "../scripts/contract-document.mjs";

const functionSummary = { kind: "function" };
const trackedSummary = {
  kind: "function",
  returns: { kind: "accessor", label: "memo result" },
  callbacks: [{ parameter: 0, execution: "tracked" }]
};

function expandedContract() {
  return {
    schemaVersion: 1,
    package: { name: "example", version: "1.0.0" },
    compilerFactsProtocol: 1,
    artifacts: {},
    entrypoints: {
      ".": {
        exports: {
          createMemo: trackedSummary,
          createRoot: functionSummary
        }
      },
      "./client": {
        exports: {
          createMemo: trackedSummary,
          createRoot: functionSummary
        },
        conditions: ["browser"]
      }
    },
    evidence: { kind: "inferred", generator: "test" }
  };
}

test("normalizes repeated summaries and identical entrypoint surfaces", () => {
  const normalized = normalizeContract(expandedContract());

  assert.deepEqual(normalized.summaries.function, functionSummary);
  assert.deepEqual(normalized.entrypoints["./client"], {
    sameAs: ".",
    conditions: ["browser"]
  });
  assert.deepEqual(
    Object.values(normalized.entrypoints["."].exports).flat().sort(),
    ["createMemo", "createRoot"]
  );
  assert.deepEqual(expandContract(normalized), expandedContract());
});

test("rejects missing summaries, duplicate exports, and alias cycles", () => {
  const normalized = normalizeContract(expandedContract());
  normalized.entrypoints["."].exports.missing = ["unknown"];
  assert.throws(() => expandContract(normalized), /references missing/);

  const duplicate = normalizeContract(expandedContract());
  duplicate.entrypoints["."].exports.function.push("createMemo");
  assert.throws(() => expandContract(duplicate), /repeats export/);

  const cycle = normalizeContract(expandedContract());
  cycle.entrypoints["."] = { sameAs: "./client" };
  assert.throws(() => expandContract(cycle), /alias cycle/);
});
