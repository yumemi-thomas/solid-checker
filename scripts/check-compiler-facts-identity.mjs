#!/usr/bin/env bun

import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");

function read(path) {
  return readFileSync(resolve(root, path), "utf8");
}

function run(command, args, cwd = root) {
  return execFileSync(command, args, { cwd, encoding: "utf8" }).trim();
}

function requireMatch(text, expression, label) {
  const match = text.match(expression);
  if (!match) throw new Error(`compiler facts identity: could not read ${label}`);
  return match[1];
}

export function cargoCompilerPin(cargo) {
  return requireMatch(
    cargo,
    /solidjs-compiler\s*=\s*\{[^\n]*rev\s*=\s*"([0-9a-f]{40})"/,
    "solidjs-compiler Cargo pin"
  );
}

export function rustStringConstant(source, name) {
  return requireMatch(
    source,
    new RegExp(`const\\s+${name}:\\s*&str\\s*=\\s*\\n?\\s*"([0-9a-f]{40})"`),
    name
  );
}

export function compilerSourceManifest(identity) {
  const canonical =
    "solid-checker:solid-v2-compiler-source-manifest:v1\n" +
    `upstream=${identity.upstreamRevision}\n` +
    `implementation=${identity.implementationRevision}\n` +
    `distribution=${identity.distributionRevision}\n` +
    `trace=${identity.semanticTraceVersion}\n` +
    `protocol=${identity.compilerFactsProtocol}\n`;
  return `sha256:${createHash("sha256").update(canonical).digest("hex")}`;
}

export function assertIdentityDocuments({ identity, cargo, lock, adapter, conformance, notices, report }) {
  if (identity.format !== 1 || identity.documentKind !== "solid-checker-solid2-compiler-facts-identity") {
    throw new Error("compiler facts identity: invalid identity document envelope");
  }
  for (const field of ["upstreamRevision", "implementationRevision", "distributionRevision"]) {
    if (!/^[0-9a-f]{40}$/.test(identity[field] ?? "")) {
      throw new Error(`compiler facts identity: ${field} is not a full commit`);
    }
  }
  if (cargoCompilerPin(cargo) !== identity.distributionRevision) {
    throw new Error("compiler facts identity: Cargo pin disagrees with distribution revision");
  }
  if (!lock.includes(`rev=${identity.distributionRevision}#${identity.distributionRevision}`)) {
    throw new Error("compiler facts identity: Cargo.lock disagrees with distribution revision");
  }
  if (
    rustStringConstant(adapter, "EXPECTED_COMPILER_UPSTREAM_REVISION") !==
    identity.upstreamRevision
  ) {
    throw new Error("compiler facts identity: adapter upstream revision drifted");
  }
  if (
    rustStringConstant(adapter, "EXPECTED_COMPILER_IMPLEMENTATION_REVISION") !==
    identity.implementationRevision
  ) {
    throw new Error("compiler facts identity: adapter implementation revision drifted");
  }
  if (
    rustStringConstant(adapter, "COMPILER_DISTRIBUTION_REVISION") !==
    identity.distributionRevision
  ) {
    throw new Error("compiler facts identity: adapter distribution revision drifted");
  }
  const sourceManifest = requireMatch(
    adapter,
    /COMPILER_SOURCE_MANIFEST_SHA256:\s*&str\s*=\s*\n?\s*"(sha256:[0-9a-f]{64})"/,
    "COMPILER_SOURCE_MANIFEST_SHA256"
  );
  if (sourceManifest !== compilerSourceManifest(identity)) {
    throw new Error("compiler facts identity: compiler source-manifest digest drifted");
  }
  if (!adapter.includes(`solid-v2:trace${identity.semanticTraceVersion}:${identity.implementationRevision}`)) {
    throw new Error("compiler facts identity: adapter cache identity drifted");
  }
  if (conformance.upstream?.revision !== identity.upstreamRevision) {
    throw new Error("compiler facts identity: bootstrap conformance upstream revision drifted");
  }
  for (const [name, text] of [["third-party notice", notices], ["Phase 4 report", report]]) {
    for (const revision of [
      identity.upstreamRevision,
      identity.implementationRevision,
      identity.distributionRevision
    ]) {
      if (!text.includes(revision)) {
        throw new Error(`compiler facts identity: ${name} omits ${revision}`);
      }
    }
  }
}

function compilerPackageFromMetadata() {
  const metadata = JSON.parse(run("cargo", [
    "+1.97",
    "metadata",
    "--offline",
    "--manifest-path",
    "rust/Cargo.toml",
    "--format-version",
    "1"
  ]));
  const packages = metadata.packages.filter(pkg =>
    pkg.name === "solidjs-compiler" && pkg.source?.startsWith("git+https://github.com/yumemi-thomas/solid?")
  );
  if (packages.length !== 1) {
    throw new Error(`compiler facts identity: expected one Solid 2 compiler package, found ${packages.length}`);
  }
  return packages[0];
}

function assertGitIdentity(identity, compilerPackage) {
  if (!compilerPackage.source.endsWith(`#${identity.distributionRevision}`)) {
    throw new Error("compiler facts identity: Cargo metadata source disagrees with distribution revision");
  }
  const checkout = run("git", ["rev-parse", "--show-toplevel"], dirname(compilerPackage.manifest_path));
  if (run("git", ["rev-parse", "HEAD"], checkout) !== identity.distributionRevision) {
    throw new Error("compiler facts identity: Cargo checkout is not the pinned distribution commit");
  }
  const parents = run("git", ["show", "-s", "--format=%P", identity.distributionRevision], checkout);
  if (parents !== identity.implementationRevision) {
    throw new Error("compiler facts identity: distribution commit is not identity-only atop implementation");
  }
  try {
    run("git", ["merge-base", "--is-ancestor", identity.upstreamRevision, identity.implementationRevision], checkout);
  } catch {
    throw new Error("compiler facts identity: implementation is not descended from the recorded upstream base");
  }
  const changed = run(
    "git",
    ["diff", "--name-only", identity.implementationRevision, identity.distributionRevision],
    checkout
  );
  if (changed !== "packages/compiler/src/semantic_trace.rs") {
    throw new Error("compiler facts identity: distribution commit changes more than the identity source");
  }
  const implementationSource = run(
    "git",
    ["show", `${identity.implementationRevision}:packages/compiler/src/semantic_trace.rs`],
    checkout
  );
  const distributionSource = run(
    "git",
    ["show", `${identity.distributionRevision}:packages/compiler/src/semantic_trace.rs`],
    checkout
  );
  const identityConstant = /pub const SEMANTIC_TRACE_IMPLEMENTATION_REVISION: &str = "[0-9a-f]{40}";/g;
  if ((implementationSource.match(identityConstant) ?? []).length !== 1) {
    throw new Error("compiler facts identity: implementation identity constant is ambiguous");
  }
  const expectedDistributionSource = implementationSource.replace(
    identityConstant,
    `pub const SEMANTIC_TRACE_IMPLEMENTATION_REVISION: &str = "${identity.implementationRevision}";`
  );
  if (distributionSource !== expectedDistributionSource) {
    throw new Error("compiler facts identity: distribution commit is not the one-line identity substitution");
  }
  if (!distributionSource.includes(
    `pub const SEMANTIC_TRACE_UPSTREAM_REVISION: &str = "${identity.upstreamRevision}";`
  )) {
    throw new Error("compiler facts identity: producer upstream constant drifted");
  }
}

function main() {
  const identity = JSON.parse(read("docs/package-contract-v2/phase4/compiler-identity.json"));
  assertIdentityDocuments({
    identity,
    cargo: read("rust/Cargo.toml"),
    lock: read("rust/Cargo.lock"),
    adapter: read("rust/dialects/solid-v2/compiler/src/lib.rs"),
    conformance: JSON.parse(read("docs/package-contract-v2/compiler-bootstrap/2026-08-27-conformance.json")),
    notices: read("THIRD_PARTY_NOTICES.md"),
    report: read("docs/package-contract-v2/phase4/2026-08-27-compiler-facts.md")
  });
  assertGitIdentity(identity, compilerPackageFromMetadata());
  console.log(
    `compiler facts identity: upstream ${identity.upstreamRevision.slice(0, 12)}, implementation ${identity.implementationRevision.slice(0, 12)}, distribution ${identity.distributionRevision.slice(0, 12)} verified`
  );
}

if (import.meta.main) main();
