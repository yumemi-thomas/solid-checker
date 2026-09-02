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
const cargo = `solidjs-compiler = { git = "https://example.invalid/solid", rev = "${distribution}" }`;
const lock = `source = "git+https://example.invalid/solid?rev=${distribution}#${distribution}"`;
const adapter = `
const EXPECTED_COMPILER_UPSTREAM_REVISION: &str = "${upstream}";
const EXPECTED_COMPILER_IMPLEMENTATION_REVISION: &str =
    "${implementation}";
pub const COMPILER_DISTRIBUTION_REVISION: &str = "${distribution}";
pub const COMPILER_SOURCE_MANIFEST_SHA256: &str =
    "${compilerSourceManifest(identity)}";
const COMPILER_FACTS_IDENTITY: &str = "solid-v2:trace3:${implementation}";
`;
const conformance = { upstream: { revision: upstream } };
const prose = `${upstream} ${implementation} ${distribution}`;

describe("compiler facts identity gate", () => {
  test("reads exact Cargo and Rust identities", () => {
    assert.equal(cargoCompilerPin(cargo), distribution);
    assert.equal(rustStringConstant(adapter, "EXPECTED_COMPILER_UPSTREAM_REVISION"), upstream);
  });

  test("requires every independent identity owner to agree", () => {
    assert.doesNotThrow(() => assertIdentityDocuments({
      identity,
      cargo,
      lock,
      adapter,
      conformance,
      notices: prose,
      report: prose
    }));
    assert.throws(() => assertIdentityDocuments({
      identity,
      cargo: cargo.replace(distribution, "d".repeat(40)),
      lock,
      adapter,
      conformance,
      notices: prose,
      report: prose
    }), /Cargo pin disagrees/);
  });
});
