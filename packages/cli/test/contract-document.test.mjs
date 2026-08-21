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

test("round-trips conditional export summaries without collapsing variants", () => {
  const conditional = expandedContract();
  conditional.entrypoints["."].exports.createMemo = {
    ...trackedSummary,
    variants: [
      { conditions: ["browser"], summary: trackedSummary },
      { conditions: ["node"], summary: { kind: "function" } }
    ]
  };
  const normalized = normalizeContract(conditional);
  const expanded = expandContract(normalized);
  assert.deepEqual(expanded.entrypoints["."].exports.createMemo, conditional.entrypoints["."].exports.createMemo);
  assert.equal(normalized.summaries.function.variants, undefined);
});

test("collapses evidence-only variants and merges their probe modes", () => {
  const conditional = expandedContract();
  conditional.entrypoints["."].exports.createMemo = {
    ...trackedSummary,
    evidence: { kind: "probed", modes: ["production"], calls: 1 },
    variants: [
      {
        conditions: ["browser", "import"],
        summary: {
          ...trackedSummary,
          evidence: { kind: "probed", modes: ["production"], calls: 1 }
        }
      },
      {
        conditions: ["browser", "development", "import"],
        summary: {
          ...trackedSummary,
          evidence: { kind: "probed", modes: ["development"], calls: 2 }
        }
      }
    ]
  };

  const expanded = expandContract(normalizeContract(conditional));
  const summary = expanded.entrypoints["."].exports.createMemo;
  assert.equal(summary.variants, undefined);
  assert.deepEqual(summary.evidence, {
    kind: "probed",
    modes: ["development", "production"],
    calls: 2
  });
});

test("removes a redundant specific variant while preserving distinct target behavior", () => {
  const conditional = expandedContract();
  conditional.entrypoints["."].exports.createMemo = {
    ...trackedSummary,
    variants: [
      {
        conditions: ["browser", "import"],
        summary: {
          ...trackedSummary,
          evidence: { kind: "probed", modes: ["production"], calls: 1 }
        }
      },
      {
        conditions: ["browser", "development", "import"],
        summary: {
          ...trackedSummary,
          evidence: { kind: "probed", modes: ["development"], calls: 2 }
        }
      },
      {
        conditions: ["import", "node"],
        summary: { kind: "function" }
      }
    ]
  };

  const expanded = expandContract(normalizeContract(conditional));
  const variants = expanded.entrypoints["."].exports.createMemo.variants;
  assert.equal(variants.length, 2);
  assert.equal(
    variants.some(variant => variant.conditions.includes("development")),
    false
  );
  const browser = variants.find(variant => variant.conditions.includes("browser"));
  assert.deepEqual(browser.summary.evidence, {
    kind: "probed",
    modes: ["development", "production"],
    calls: 2
  });
});

test("does not promote broad inferred evidence from a reviewed specific branch", () => {
  const conditional = expandedContract();
  conditional.entrypoints["."].exports.createMemo = {
    ...trackedSummary,
    evidence: { kind: "inferred" },
    variants: [
      {
        conditions: ["browser", "import"],
        summary: { ...trackedSummary, evidence: { kind: "inferred" } }
      },
      {
        conditions: ["browser", "development", "import"],
        summary: { ...trackedSummary, evidence: { kind: "reviewed" } }
      }
    ]
  };

  const expanded = expandContract(normalizeContract(conditional));
  const summary = expanded.entrypoints["."].exports.createMemo;
  assert.equal(summary.variants, undefined);
  assert.deepEqual(summary.evidence, { kind: "inferred" });
});
