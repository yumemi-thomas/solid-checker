#!/usr/bin/env bun

// Phase 18 repository-wide stable-v1 convergence authority.
//
// This gate inventories active main documents and every code boundary allowed
// to encode, decode, transport, or inspect them. It intentionally treats each
// neighboring protocol's version as a separate namespace: changing the main
// contract version must never rewrite receipts, sidecars, catalogs, Type Facts,
// rule manifests, runtime-resolution documents, or runtime-probe documents.

import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

export const MAIN_FORMAT = "solid-reactivity-contract";
export const MAIN_SCHEMA_VERSION = 1;
export const SEMANTIC_MODEL_VERSION = 1;

const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");

const VERSIONED_FORMATS = new Map([
  [MAIN_FORMAT, { field: "schemaVersion", version: 1, semanticModel: true, main: true }],
  ["solid-checker-package-contract-bundle-index", { field: "schemaVersion", version: 1 }],
  ["solid-checker-package-runtime-lock", { field: "schemaVersion", version: 2 }],
  ["solid-checker-package-contract-generator-corpus", { field: "schemaVersion", version: 1 }],
  ["solid-checker-contract-proposal-plan", { field: "planVersion", version: 1, semanticModel: true }],
  ["solid-checker-contract-proposal-refusals", { field: "refusalVersion", version: 1 }],
  ["solid-checker-contract-review", { field: "schemaVersion", version: 2, semanticModel: true }],
  ["solid-checker-accepted-contract-catalog", { field: "catalogVersion", version: 2 }],
  ["solid-checker-proof-evidence", { field: "sidecarVersion", version: 1, semanticModel: true }],
  ["solid-checker-runtime-probe-evidence", { field: "sidecarVersion", version: 1, semanticModel: true }],
  ["solid-checker-runtime-probe-request", { field: "schemaVersion", version: 2 }],
  ["solid-checker-runtime-probe-plan", { field: "schemaVersion", version: 2 }],
  ["solid-checker-runtime-probe-runs", { field: "schemaVersion", version: 2 }],
  ["solid-checker-runtime-probe-evaluation", { field: "schemaVersion", version: 2 }]
]);

const ACTIVE_JSON_PREFIXES = [
  "benchmarks/package-contract-v2/phase6/",
  "benchmarks/package-contract-v2/phase14/",
  "benchmarks/package-contract-v2/phase16/",
  "fixtures/package-contracts/",
  "fixtures/reactive-ir/",
  "pkg/contracts/bundled/",
  "rust/crates/solid-dialect/contracts/",
  "rust/crates/solid-facts-backend/tests/fixtures/",
  "schema/"
];

const ACTIVE_JSON_FILES = new Set([
  "fixtures/ownership-cases/cases.json",
  "fixtures/ownership-cases/migration-ledger.json",
  "fixtures/tsc-oracle/packages.json",
  "fixtures/tsc-oracle/rule-cases.json",
  "packages/cli/lib/rules-solid-v1.json",
  "packages/cli/lib/rules-solid-v2.json",
  "rust/dialects/solid-v1/dialect.json",
  "rust/dialects/solid-v2/dialect.json",
  "scripts/ecosystem-benchmark/manifest.json"
]);

const FORBIDDEN_ACTIVE_PATHS = [
  "schema/solid-reactivity-contract-v2.schema.json",
  "packages/cli/scripts/generate-package-contract-v2.mjs",
  "packages/cli/scripts/contract-document.mjs",
  "packages/cli/scripts/contract-verification.mjs",
  "packages/cli/scripts/runtime-module-closure.mjs",
  "rust/crates/solid-facts-backend/src/contract_document_v2.rs",
  "rust/crates/solid-facts-backend/src/inferred_contract_v2.rs",
  "rust/crates/solid-facts-backend/src/bin/solid-contract-gen.rs",
  "scripts/package-contract-v2-phase17.mjs"
];

const ACTIVE_TEXT_FILES = [
  "AGENTS.md",
  "README.md",
  "CONTRIBUTING.md",
  "Makefile",
  "rust/ARCHITECTURE.md",
  "docs/package-contracts.md",
  "docs/package-contract-v2/README.md",
  "docs/package-contract-v2/architecture.md",
  "docs/package-contract-v2/migration-and-verification.md",
  "scripts/verify.sh",
  "scripts/verify-delta.mjs",
  ".github/workflows/ci.yml",
  ".github/workflows/contract-corpus.yml",
  ".github/workflows/ecosystem-benchmark.yml"
];

const SOURCE_OWNERS = [
  {
    path: "rust/crates/solid-facts-backend/src/contract_document.rs",
    markers: [
      'const FORMAT: &str = "solid-reactivity-contract";',
      "const SCHEMA_VERSION: u16 = 1;",
      "pub(crate) fn decode(",
      "pub(crate) fn encode("
    ]
  },
  {
    path: "rust/crates/solid-facts-backend/src/lib.rs",
    markers: [
      "pub fn validate_contract_document(",
      "contract_document::decode(bytes)?.normalize()?;",
      "contract_document::encode(&proposal"
    ]
  },
  {
    path: "rust/crates/solid-facts-backend/src/contract_interface.rs",
    markers: [
      "pub fn load_accepted_contract(",
      "pub fn load_authenticated_policy2_contract(",
      "authenticate_policy2_receipt(",
      "const ACCEPTED_CATALOG_VERSION: u16 = 2;"
    ]
  },
  {
    path: "rust/crates/solid-facts-backend/src/contract_workflow.rs",
    markers: [
      "contract_document::encode(",
      "const PLAN_VERSION: u16 = 1;"
    ]
  },
  {
    path: "rust/crates/solid-facts-backend/src/proposal_generation.rs",
    markers: ["contract_document::encode(", "contract_document::decode("]
  },
  {
    path: "rust/crates/solid-facts-backend/src/evidence_sidecars.rs",
    markers: ["pub const EVIDENCE_SIDECAR_VERSION: u16 = 1;", "contract_document::decode("]
  },
  {
    path: "rust/crates/solid-facts-backend/src/runtime_probe_wire.rs",
    markers: ["const SCHEMA_VERSION: u16 = 2;", "contract_document::decode("]
  },
  {
    path: "rust/crates/solid-facts-backend/src/first_party_bundles.rs",
    markers: ["contract_document::decode("]
  },
  {
    path: "rust/crates/solid-checker-wasm/src/lib.rs",
    markers: ["accepted_contracts: Vec<HostAcceptedContract>", "load_accepted_contract_index("]
  },
  {
    path: "packages/cli/scripts/generate-package-contract.mjs",
    markers: ["Stable package-contract proposal producer.", "This file never reads a summary."]
  },
  {
    path: "packages/cli/scripts/review-contract.mjs",
    markers: ["JavaScript owns only CLI parsing and process lifecycle."]
  },
  {
    path: "packages/cli/scripts/verify-contract.mjs",
    markers: ["Policy-1 proof-file issuance was removed", "contract certify"]
  },
  {
    path: "packages/cli/scripts/contract-probe-driver.mjs",
    markers: ["stable main schema version 1"]
  },
  {
    path: "scripts/check-bundled-contracts.mjs",
    markers: ["Checks active stable-v1 bundle indexes", "document.schemaVersion !== 1"]
  },
  {
    path: "scripts/contract-corpus.mjs",
    markers: ["solid-checker-package-contract-generator-corpus", 'assertEnvelope(output, "solid-reactivity-contract"']
  }
];

const STABLE_BOUNDARY_TESTS = [
  {
    path: "packages/wasm/test/resolved-imports.test.mjs",
    required: [
      "temporary schema-version-2 documents have no compatibility decoder",
      "/schema version 2.*expected 1/i",
      "legacy schema-version-1 documents have no compatibility decoder",
      "delete legacy.format;",
      "/contract document cannot be decoded.*missing field.*format/i"
    ],
    forbidden: ["solid-checker-wasm-v2-", "/schema version 1.*expected 2/i"]
  }
];

const INDEPENDENT_JSON_VERSIONS = [
  ["packages/cli/lib/rules-solid-v1.json", "schemaVersion", 1],
  ["packages/cli/lib/rules-solid-v2.json", "schemaVersion", 1],
  ["scripts/ecosystem-benchmark/manifest.json", "schemaVersion", 1],
  ["fixtures/ownership-cases/cases.json", "schemaVersion", 1],
  ["fixtures/ownership-cases/migration-ledger.json", "schemaVersion", 1],
  ["rust/dialects/solid-v1/dialect.json", "schemaVersion", 2],
  ["rust/dialects/solid-v2/dialect.json", "schemaVersion", 2]
];

const INDEPENDENT_SOURCE_VERSIONS = [
  ["rust/crates/solid-facts-backend/src/main.rs", "if document.schema_version != 1"],
  ["packages/cli/scripts/generate-package-contract.mjs", "{\"schemaVersion\":1,\"resolutions\":[]}"],
  ["scripts/lib/gate-cache.mjs", "export const CACHE_FORMAT_VERSION = 3;"],
  ["scripts/check-contract-pins.mjs", "export const MEMO_FORMAT_VERSION = 3;"],
  ["rust/crates/solid-reactive-ir/src/contract_semantics.rs", "pub const SEMANTIC_MODEL_VERSION: u16 = 1;"],
  ["rust/crates/solid-reactive-ir/src/contract_semantics.rs", 'pub const SEMANTIC_DIGEST_ALGORITHM: &str = "sha256";'],
  ["rust/crates/solid-reactive-ir/src/contract_semantics.rs", 'pub const SEMANTIC_DIGEST_DOMAIN: &str = "solid-checker:normalized-package-contract";']
];

function sha256(value) {
  return `sha256:${createHash("sha256").update(value).digest("hex")}`;
}

function parseEntry(entry) {
  try {
    return JSON.parse(entry.bytes);
  } catch (error) {
    throw new Error(`${entry.path} is not valid JSON: ${error.message}`);
  }
}

function requireVersion(path, document, field, expected) {
  if (document[field] !== expected) {
    throw new Error(`${path} has ${field} ${JSON.stringify(document[field])}; expected ${expected}`);
  }
}

function looksLikeLegacyMain(document) {
  return (
    document &&
    typeof document === "object" &&
    !Array.isArray(document) &&
    document.format !== MAIN_FORMAT &&
    "schemaVersion" in document &&
    "package" in document &&
    "entrypoints" in document &&
    ("summaries" in document || "compilerFactsProtocol" in document || "evidence" in document)
  );
}

function containsKey(value, searched, seen = new Set()) {
  if (!value || typeof value !== "object" || seen.has(value)) return false;
  seen.add(value);
  if (!Array.isArray(value) && Object.hasOwn(value, searched)) return true;
  return Object.values(value).some(child => containsKey(child, searched, seen));
}

export function auditDocumentEntries(entries) {
  const byPath = new Map(entries.map(entry => [entry.path, { ...entry, document: parseEntry(entry) }]));
  let mainDocuments = 0;
  let receipts = 0;
  let independentVersionedDocuments = 0;

  for (const entry of byPath.values()) {
    const document = entry.document;
    if (looksLikeLegacyMain(document)) {
      throw new Error(`${entry.path} is a legacy-v1 main document without the required format discriminator`);
    }

    const rule = VERSIONED_FORMATS.get(document?.format);
    if (rule) {
      requireVersion(entry.path, document, rule.field, rule.version);
      if (rule.semanticModel) {
        requireVersion(entry.path, document, "semanticModelVersion", SEMANTIC_MODEL_VERSION);
      }
      if (rule.main) {
        if (containsKey(document, "schemaStatus")) {
          throw new Error(`${entry.path} contains forbidden schemaStatus`);
        }
        mainDocuments += 1;
      } else {
        independentVersionedDocuments += 1;
      }
    }

    if (Object.hasOwn(document ?? {}, "receiptVersion")) {
      requireVersion(entry.path, document, "receiptVersion", 2);
      requireVersion(
        entry.path,
        document.payload ?? {},
        "semanticModelVersion",
        SEMANTIC_MODEL_VERSION
      );
      if (!entry.path.endsWith(".receipt.json")) {
        throw new Error(`${entry.path} is a receipt but does not use the .receipt.json suffix`);
      }
      const mainPath = entry.path.replace(/\.receipt\.json$/, ".json");
      const mainEntry = byPath.get(mainPath);
      if (!mainEntry) throw new Error(`${entry.path} has no sibling main document ${mainPath}`);
      requireVersion(mainPath, mainEntry.document, "schemaVersion", MAIN_SCHEMA_VERSION);
      if (mainEntry.document.format !== MAIN_FORMAT) {
        throw new Error(`${entry.path} binds a non-main document ${mainPath}`);
      }
      if (document.payload?.mainDigest !== sha256(mainEntry.bytes)) {
        throw new Error(`${entry.path} mainDigest does not bind ${mainPath}'s exact bytes`);
      }
      receipts += 1;
    }
  }

  return { mainDocuments, receipts, independentVersionedDocuments };
}

function trackedFiles(root, pattern) {
  const output = execFileSync("git", ["ls-files", "-z", "--", pattern], {
    cwd: root,
    encoding: "utf8"
  });
  return output.split("\0").filter(Boolean);
}

function activeJsonPath(path) {
  return ACTIVE_JSON_FILES.has(path) || ACTIVE_JSON_PREFIXES.some(prefix => path.startsWith(prefix));
}

function readText(root, path) {
  return readFileSync(join(root, path), "utf8");
}

export function auditStableBoundaryTestEntries(entries) {
  const byPath = new Map(entries.map(entry => [entry.path, String(entry.source)]));
  for (const test of STABLE_BOUNDARY_TESTS) {
    const source = byPath.get(test.path);
    if (source === undefined) throw new Error(`stable boundary test is missing: ${test.path}`);
    for (const marker of test.required) {
      if (!source.includes(marker)) {
        throw new Error(`${test.path} is missing stable boundary assertion ${marker}`);
      }
    }
    for (const marker of test.forbidden) {
      if (source.includes(marker)) {
        throw new Error(`${test.path} retains temporary-v2 assertion ${marker}`);
      }
    }
  }
  return STABLE_BOUNDARY_TESTS.length;
}

function auditSourceInventory(root) {
  for (const path of FORBIDDEN_ACTIVE_PATHS) {
    if (existsSync(join(root, path))) throw new Error(`retired legacy path still exists: ${path}`);
  }

  for (const path of ACTIVE_TEXT_FILES) {
    const source = readText(root, path);
    for (const forbidden of FORBIDDEN_ACTIVE_PATHS) {
      if (source.includes(forbidden)) {
        throw new Error(`${path} still references retired legacy path ${forbidden}`);
      }
    }
  }

  for (const owner of SOURCE_OWNERS) {
    const source = readText(root, owner.path);
    for (const marker of owner.markers) {
      if (!source.includes(marker)) throw new Error(`${owner.path} is missing convergence marker ${marker}`);
    }
  }

  for (const [path, marker] of INDEPENDENT_SOURCE_VERSIONS) {
    if (!readText(root, path).includes(marker)) {
      throw new Error(`${path} changed independent version marker ${marker}`);
    }
  }

  const stableBoundaryTests = auditStableBoundaryTestEntries(
    STABLE_BOUNDARY_TESTS.map(test => ({ path: test.path, source: readText(root, test.path) }))
  );

  const javascriptMainReaders = [...new Set([
    ...trackedFiles(root, "*.mjs"),
    "scripts/package-contract-phase18.mjs"
  ])]
    .filter(path => existsSync(join(root, path)))
    .filter(path => !path.endsWith(".test.mjs"))
    .filter(path => new RegExp(`["']${MAIN_FORMAT}["']`).test(readText(root, path)))
    .sort();
  const allowedReaders = [
    "scripts/check-bundled-contracts.mjs",
    "scripts/contract-corpus.mjs",
    "scripts/ecosystem-benchmark/lib/contract-content.mjs",
    "scripts/package-contract-phase18.mjs",
    "scripts/solid-recharts-performance.mjs"
  ];
  if (JSON.stringify(javascriptMainReaders) !== JSON.stringify(allowedReaders)) {
    throw new Error(
      `JavaScript main-document reader inventory drifted:\n${javascriptMainReaders.join("\n")}`
    );
  }

  return { sourceOwners: SOURCE_OWNERS.length, stableBoundaryTests };
}

export function auditRepository(root = repositoryRoot) {
  const jsonPaths = [...new Set([
    ...trackedFiles(root, "*.json"),
    "schema/solid-reactivity.schema.json"
  ])]
    .filter(activeJsonPath)
    .filter(path => existsSync(join(root, path)))
    .sort();
  const entries = jsonPaths.map(path => ({ path, bytes: readFileSync(join(root, path)) }));
  const documents = auditDocumentEntries(entries);
  const byPath = new Map(entries.map(entry => [entry.path, parseEntry(entry)]));

  for (const [path, field, expected] of INDEPENDENT_JSON_VERSIONS) {
    const document = byPath.get(path);
    if (!document) throw new Error(`independent version document is missing: ${path}`);
    requireVersion(path, document, field, expected);
  }

  const sourceInventory = auditSourceInventory(root);
  return {
    ...documents,
    auditedJsonFiles: entries.length,
    ...sourceInventory,
    semanticModelVersion: SEMANTIC_MODEL_VERSION,
    semanticDigestAlgorithm: "sha256"
  };
}

function main() {
  const result = auditRepository();
  console.log(
    `phase18 convergence: ${result.mainDocuments} stable-v1 mains, ` +
      `${result.receipts} byte-bound receipts, ${result.independentVersionedDocuments} ` +
      `independent versioned documents, ${result.sourceOwners} source owners, ` +
      `${result.auditedJsonFiles} JSON files audited; semantic model v1 / sha256 frozen`
  );
}

if (import.meta.main) main();
