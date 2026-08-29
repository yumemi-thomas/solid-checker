import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { describe, test } from "vitest";

import {
  auditDocumentEntries,
  auditRepository,
  MAIN_FORMAT,
  MAIN_SCHEMA_VERSION,
  SEMANTIC_MODEL_VERSION
} from "./package-contract-phase18.mjs";

const bytes = value => Buffer.from(JSON.stringify(value));
const wireDigest = value =>
  `sha256:${createHash("sha256").update(value).digest("hex")}`;

const main = (overrides = {}) => ({
  format: MAIN_FORMAT,
  schemaVersion: MAIN_SCHEMA_VERSION,
  semanticModelVersion: SEMANTIC_MODEL_VERSION,
  package: { name: "example", version: "1.0.0" },
  summaries: {},
  entrypoints: {},
  sidecars: {},
  ...overrides
});

describe("Phase 18 stable schema-version-1 convergence", () => {
  test("accepts a stable main and a receipt bound to its exact bytes", () => {
    const documentBytes = bytes(main());
    const receiptBytes = bytes({
      receiptVersion: 1,
      wireDigest: wireDigest(documentBytes),
      semanticModelVersion: 1
    });
    const result = auditDocumentEntries([
      { path: "contracts/example.json", bytes: documentBytes },
      { path: "contracts/example.receipt.json", bytes: receiptBytes }
    ]);
    assert.equal(MAIN_SCHEMA_VERSION, 1);
    assert.equal(result.mainDocuments, 1);
    assert.equal(result.receipts, 1);
  });

  test("rejects both the retired temporary-v2 main and the legacy-v1 shape", () => {
    assert.throws(
      () =>
        auditDocumentEntries([
          { path: "contracts/temporary.json", bytes: bytes(main({ schemaVersion: 2 })) }
        ]),
      /schemaVersion 2.*expected 1/
    );
    assert.throws(
      () =>
        auditDocumentEntries([
          {
            path: "contracts/legacy.json",
            bytes: bytes({
              schemaVersion: 1,
              compilerFactsProtocol: 1,
              package: { name: "legacy", version: "1.0.0" },
              summaries: {},
              entrypoints: {},
              evidence: { kind: "verified" }
            })
          }
        ]),
      /legacy-v1 main document/
    );
  });

  test("keeps every neighboring protocol in its independent namespace", () => {
    const result = auditDocumentEntries([
      {
        path: "accepted-contracts.json",
        bytes: bytes({
          format: "solid-checker-accepted-contract-catalog",
          catalogVersion: 1,
          contracts: []
        })
      },
      {
        path: "probe-request.json",
        bytes: bytes({
          format: "solid-checker-runtime-probe-request",
          schemaVersion: 2
        })
      }
    ]);
    assert.equal(result.independentVersionedDocuments, 2);
  });

  test("rejects a receipt carrying the pre-cut wire digest", () => {
    const documentBytes = bytes(main());
    assert.throws(
      () =>
        auditDocumentEntries([
          { path: "contracts/example.json", bytes: documentBytes },
          {
            path: "contracts/example.receipt.json",
            bytes: bytes({
              receiptVersion: 1,
              wireDigest: `sha256:${"0".repeat(64)}`,
              semanticModelVersion: 1
            })
          }
        ]),
      /wireDigest does not bind/
    );
  });

  test("the checked repository is stable-only with semantic model 1", () => {
    const result = auditRepository();
    assert.ok(result.mainDocuments > 0);
    assert.ok(result.receipts > 0);
    assert.ok(result.sourceOwners > 0);
    assert.equal(result.semanticModelVersion, 1);
    assert.equal(result.semanticDigestAlgorithm, "sha256");
  });
});
