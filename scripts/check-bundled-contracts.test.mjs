import assert from "node:assert/strict";
import { describe, test } from "vitest";

import { checkBundleIndexes, validateBundleCase } from "./check-bundled-contracts.mjs";

// The bundle-case version contract is two *independent* version namespaces —
// stable `schemaVersion: 1` for the main document, `receiptVersion: 2` for the
// acceptance receipt Phase 18 cut. Both bundle indexes are currently empty, so
// the real corpus exercises neither arm; these synthetic pairs are the only
// thing that does.
const mainDocument = { schemaVersion: 1, format: "solid-reactivity-contract" };
const receipt = { receiptVersion: 2, wireDigest: `sha256:${"a".repeat(64)}` };

const check = (document, issued) =>
  validateBundleCase({
    documentPath: "pkg/contracts/bundled/solid-v1/main.json",
    receiptPath: "pkg/contracts/bundled/solid-v1/receipt.json",
    document,
    receipt: issued
  });

describe("bundled contract version namespaces", () => {
  test("accepts a stable-v1 main document paired with a receipt-v2 acceptance receipt", () => {
    check(mainDocument, receipt);
  });

  test("refuses a main document that is not the first stable public schema version", () => {
    assert.throws(
      () => check({ ...mainDocument, schemaVersion: 2 }, receipt),
      /is not a stable-v1 main document/
    );
    assert.throws(
      () => check({ ...mainDocument, format: "solid-reactivity-proposal" }, receipt),
      /is not a stable-v1 main document/
    );
  });

  test("refuses a receipt from the retired policy-1 namespace", () => {
    // The exact regression this branch had: it demanded `receiptVersion: 1`,
    // which every receipt issued from Phase 18 onwards would have failed.
    assert.throws(
      () => check(mainDocument, { ...receipt, receiptVersion: 1 }),
      /is not a proof-issued acceptance receipt/
    );
  });

  test("refuses a receipt with no wire digest to bind", () => {
    assert.throws(
      () => check(mainDocument, { receiptVersion: 2 }),
      /is not a proof-issued acceptance receipt/
    );
  });

  test("the checked-in bundle indexes still hold exactly the cases the cut left", () => {
    // Zero, deliberately: every former first-party case lost policy-1
    // authority at the atomic cut. A nonzero count here means a case was
    // reissued, and the arms above are what it will be checked against.
    assert.equal(checkBundleIndexes(), 0);
  });
});
