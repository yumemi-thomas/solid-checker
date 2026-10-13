import assert from "node:assert/strict";
import { describe, test } from "vitest";

import {
  assertIdentityDocuments,
  cargoCompilerPin,
  compilerSourceManifest,
  rustStringConstant
} from "./check-compiler-facts-identity.mjs";

const upstream = "a".repeat(40);
const implementation = "b".repeat(40);
const distribution = "c".repeat(40);
const identity = {
  format: 1,
  documentKind: "solid-checker-solid2-compiler-facts-identity",
  upstreamRevision: upstream,
  implementationRevision: implementation,
  distributionRevision: distribution,
  semanticTraceVersion: 3,
  compilerFactsProtocol: 2
};
const cargoPackage = "solidjs-compiler";
const cargo = `${cargoPackage} = { git = "https://example.invalid/solid", rev = "${distribution}" }`;
const lock = `source = "git+https://example.invalid/solid?rev=${distribution}#${distribution}"`;
const adapter = `
const EXPECTED_COMPILER_UPSTREAM_REVISION: &str = "${upstream}";
const EXPECTED_COMPILER_IMPLEMENTATION_REVISION: &str =
    "${implementation}";
pub const COMPILER_DISTRIBUTION_REVISION: &str = "${distribution}";
pub const COMPILER_SOURCE_MANIFEST_SHA256: &str =
    "${compilerSourceManifest(identity, "solid-v2")}";
const COMPILER_FACTS_IDENTITY: &str = "solid-v2:trace3:${implementation}";
`;
const conformance = { upstream: { revision: upstream } };
const prose = `${upstream} ${implementation} ${distribution}`;

describe("compiler facts identity gate", () => {
  test("reads exact Cargo and Rust identities", () => {
    assert.equal(cargoCompilerPin(cargo, cargoPackage), distribution);
    assert.equal(rustStringConstant(adapter, "EXPECTED_COMPILER_UPSTREAM_REVISION"), upstream);
  });

  test("requires every independent identity owner to agree", () => {
    assert.doesNotThrow(() => assertIdentityDocuments({
      dialect: "solid-v2",
      cargoPackage,
      identity,
      cargo,
      lock,
      adapter,
      conformance,
      notices: prose,
      report: prose
    }));
    assert.throws(() => assertIdentityDocuments({
      dialect: "solid-v2",
      cargoPackage,
      identity,
      cargo: cargo.replace(distribution, "d".repeat(40)),
      lock,
      adapter,
      conformance,
      notices: prose,
      report: prose
    }), /Cargo pin disagrees/);
  });

  // Every identity string this gate checks is keyed on the dialect it is
  // checking, so asking solid-v2's documents under another dialect's name has
  // to fail. Without this, the parameters added when the gate started
  // enumerating dialects could be ignored and nothing would notice.
  test("refuses one dialect's documents under another dialect's name", () => {
    assert.throws(() => assertIdentityDocuments({
      dialect: "solid-v3",
      cargoPackage,
      identity,
      cargo,
      lock,
      adapter,
      conformance,
      notices: prose,
      report: prose
    }), /invalid identity document envelope/);
    assert.notEqual(
      compilerSourceManifest(identity, "solid-v3"),
      compilerSourceManifest(identity, "solid-v2")
    );
  });
});
