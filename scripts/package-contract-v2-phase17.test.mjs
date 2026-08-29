import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { describe, test } from "vitest";

import {
  auditDocumentEntries,
  auditRepository,
  MAIN_FORMAT,
  MAIN_SCHEMA_VERSION,
  SEMANTIC_MODEL_VERSION
} from "./package-contract-v2-phase17.mjs";

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

describe("Phase 17 temporary-v2 convergence", () => {
  test("accepts a temporary-v2 main and a receipt bound to its exact bytes", () => {
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
    assert.equal(result.mainDocuments, 1);
    assert.equal(result.receipts, 1);
  });

  test("rejects both legacy-v1 shapes and temporary mains carrying the stable number", () => {
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
    assert.throws(
      () =>
        auditDocumentEntries([
          { path: "contracts/wrong-version.json", bytes: bytes(main({ schemaVersion: 1 })) }
        ]),
      /schemaVersion 1.*expected 2/
    );
  });

  test("keeps receipt and catalog versions independent from the main schema", () => {
    const catalog = bytes({
      format: "solid-checker-accepted-contract-catalog",
      catalogVersion: 1,
      contracts: []
    });
    const result = auditDocumentEntries([{ path: "accepted-contracts.json", bytes: catalog }]);
    assert.equal(result.independentVersionedDocuments, 1);
    assert.throws(
      () =>
        auditDocumentEntries([
          {
            path: "accepted-contracts.json",
            bytes: bytes({
              format: "solid-checker-accepted-contract-catalog",
              catalogVersion: 2,
              contracts: []
            })
          }
        ]),
      /catalogVersion 2.*expected 1/
    );
  });

  test("rejects receipts for non-v2 or byte-different main documents", () => {
    const documentBytes = bytes(main());
    const wrongReceipt = bytes({
      receiptVersion: 1,
      wireDigest: `sha256:${"0".repeat(64)}`,
      semanticModelVersion: 1
    });
    assert.throws(
      () =>
        auditDocumentEntries([
          { path: "contracts/example.json", bytes: documentBytes },
          { path: "contracts/example.receipt.json", bytes: wrongReceipt }
        ]),
      /wireDigest does not bind/
    );
  });

  test("the checked repository satisfies the complete convergence inventory", () => {
    const result = auditRepository();
    assert.ok(result.mainDocuments > 0);
    assert.ok(result.receipts > 0);
    assert.ok(result.sourceOwners > 0);
    assert.equal(result.semanticModelVersion, 1);
    assert.equal(result.semanticDigestAlgorithm, "sha256");
  });
});
