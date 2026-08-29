#!/usr/bin/env bun

// Phase 17 repository-wide temporary-v2 convergence authority.
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
export const MAIN_SCHEMA_VERSION = 2;
export const SEMANTIC_MODEL_VERSION = 1;

const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");

const VERSIONED_FORMATS = new Map([
  [MAIN_FORMAT, { field: "schemaVersion", version: 2, semanticModel: true, main: true }],
  ["solid-checker-temporary-v2-bundle-index", { field: "schemaVersion", version: 2 }],
  ["solid-checker-package-runtime-lock", { field: "schemaVersion", version: 2 }],
  ["solid-checker-temporary-v2-generator-corpus", { field: "schemaVersion", version: 2 }],
  ["solid-checker-contract-proposal-plan", { field: "planVersion", version: 1, semanticModel: true }],
  ["solid-checker-contract-proposal-refusals", { field: "refusalVersion", version: 1 }],
  ["solid-checker-contract-proof-transcript", { field: "proofVersion", version: 1, semanticModel: true }],
  ["solid-checker-contract-review", { field: "schemaVersion", version: 2, semanticModel: true }],
  ["solid-checker-accepted-contract-catalog", { field: "catalogVersion", version: 1 }],
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
  "schema/solid-reactivity.schema.json",
  "packages/cli/scripts/generate-package-contract.mjs",
  "packages/cli/scripts/contract-document.mjs",
  "packages/cli/scripts/contract-verification.mjs",
  "packages/cli/scripts/runtime-module-closure.mjs",
  "rust/crates/solid-facts-backend/src/contract_document.rs",
  "rust/crates/solid-facts-backend/src/bin/solid-contract-gen.rs"
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
    path: "rust/crates/solid-facts-backend/src/contract_document_v2.rs",
    markers: [
      'const FORMAT: &str = "solid-reactivity-contract";',
      "const DEVELOPMENT_SCHEMA_VERSION: u16 = 2;",
      "pub(crate) fn decode(",
      "pub(crate) fn encode("
    ]
  },
  {
    path: "rust/crates/solid-facts-backend/src/lib.rs",
    markers: [
      "pub fn validate_contract_document(",
      "contract_document_v2::decode(bytes)?.normalize()?;",
      "contract_document_v2::encode(&proposal"
    ]
  },
  {
    path: "rust/crates/solid-facts-backend/src/contract_interface.rs",
    markers: [
      "pub fn load_accepted_contract(",
      "let document = contract_document_v2::decode(document_bytes)?;",
      "const ACCEPTED_CATALOG_VERSION: u16 = 1;"
    ]
  },
  {
    path: "rust/crates/solid-facts-backend/src/contract_workflow.rs",
    markers: [
      "contract_document_v2::encode(",
      "const PLAN_VERSION: u16 = 1;",
      "const PROOF_VERSION: u16 = 1;"
    ]
  },
  {
    path: "rust/crates/solid-facts-backend/src/proposal_generation.rs",
    markers: ["contract_document_v2::encode(", "contract_document_v2::decode("]
  },
  {
    path: "rust/crates/solid-facts-backend/src/evidence_sidecars.rs",
    markers: ["pub const EVIDENCE_SIDECAR_VERSION: u16 = 1;", "contract_document_v2::decode("]
  },
  {
    path: "rust/crates/solid-facts-backend/src/runtime_probe_wire.rs",
    markers: ["const SCHEMA_VERSION: u16 = 2;", "contract_document_v2::decode("]
  },
  {
    path: "rust/crates/solid-facts-backend/src/first_party_bundles.rs",
    markers: ["contract_document_v2::decode("]
  },
  {
    path: "rust/crates/solid-checker-wasm/src/lib.rs",
    markers: ["accepted_contracts: Vec<HostAcceptedContract>", "load_accepted_contract_index("]
  },
  {
    path: "packages/cli/scripts/generate-package-contract-v2.mjs",
    markers: ["Temporary-v2 package proposal producer.", "This file never reads a summary."]
  },
  {
    path: "packages/cli/scripts/review-contract.mjs",
    markers: ["JavaScript owns only CLI parsing and process lifecycle."]
  },
  {
    path: "packages/cli/scripts/verify-contract.mjs",
    markers: ["Rust proof checker is", "only component allowed to close claims"]
  },
  {
    path: "packages/cli/scripts/contract-probe-driver.mjs",
    markers: ["temporary schema version 2"]
  },
  {
    path: "scripts/check-bundled-contracts.mjs",
    markers: ["receipt-issued temporary-v2 bundles", "document.schemaVersion !== 2"]
  },
  {
    path: "scripts/contract-corpus.mjs",
    markers: ["solid-checker-temporary-v2-generator-corpus", 'assertEnvelope(output, "solid-reactivity-contract"']
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
  ["packages/cli/scripts/generate-package-contract-v2.mjs", "{\"schemaVersion\":1,\"resolutions\":[]}"],
  ["scripts/lib/gate-cache.mjs", "export const CACHE_FORMAT_VERSION = 2;"],
  ["scripts/check-contract-pins.mjs", "export const MEMO_FORMAT_VERSION = 2;"],
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
      requireVersion(entry.path, document, "receiptVersion", 1);
      requireVersion(entry.path, document, "semanticModelVersion", SEMANTIC_MODEL_VERSION);
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
      if (document.wireDigest !== sha256(mainEntry.bytes)) {
        throw new Error(`${entry.path} wireDigest does not bind ${mainPath}'s exact bytes`);
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

  const javascriptMainReaders = [...new Set([
    ...trackedFiles(root, "*.mjs"),
    "scripts/package-contract-v2-phase17.mjs"
  ])]
    .filter(path => !path.endsWith(".test.mjs"))
    .filter(path => new RegExp(`["']${MAIN_FORMAT}["']`).test(readText(root, path)))
    .sort();
  const allowedReaders = [
    "scripts/check-bundled-contracts.mjs",
    "scripts/contract-corpus.mjs",
    "scripts/ecosystem-benchmark/lib/contract-content.mjs",
    "scripts/package-contract-v2-phase17.mjs",
    "scripts/solid-recharts-performance.mjs"
  ];
  if (JSON.stringify(javascriptMainReaders) !== JSON.stringify(allowedReaders)) {
    throw new Error(
      `JavaScript main-document reader inventory drifted:\n${javascriptMainReaders.join("\n")}`
    );
  }

  return SOURCE_OWNERS.length;
}

export function auditRepository(root = repositoryRoot) {
  const jsonPaths = trackedFiles(root, "*.json")
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

  const sourceOwners = auditSourceInventory(root);
  return {
    ...documents,
    auditedJsonFiles: entries.length,
    sourceOwners,
    semanticModelVersion: SEMANTIC_MODEL_VERSION,
    semanticDigestAlgorithm: "sha256"
  };
}

function main() {
  const result = auditRepository();
  console.log(
    `phase17 convergence: ${result.mainDocuments} temporary-v2 mains, ` +
      `${result.receipts} byte-bound receipts, ${result.independentVersionedDocuments} ` +
      `independent versioned documents, ${result.sourceOwners} source owners, ` +
      `${result.auditedJsonFiles} JSON files audited; semantic model v1 / sha256 frozen`
  );
}

if (import.meta.main) main();
