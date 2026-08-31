#!/usr/bin/env bun

// Stable-v1 generator corpus. Each fixture is acquired as an exact package
// artifact, analyzed by Rust, emitted as an open proposal plus a separate
// proof/probe plan, and compared byte-for-byte with checked snapshots. No
// JavaScript code expands or normalizes semantic summaries.

import { createHash } from "node:crypto";
import { execFile } from "node:child_process";
import {
  copyFileSync,
  existsSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync
} from "node:fs";
import { tmpdir } from "node:os";
import { basename, join, resolve } from "node:path";
import { promisify } from "node:util";

import { gateConcurrency, mapPool } from "./lib/pool.mjs";

const run = promisify(execFile);
const root = resolve(import.meta.dirname, "..");
const fixturesRoot = join(root, "fixtures/package-contracts");
const cli = join(root, "packages/cli/bin/solid-checker.mjs");
const native = process.env.SOLID_CHECKER_NATIVE_BIN ?? join(root, "rust/target/debug/solid-checker-rust");
const typeFacts = process.env.SOLID_TYPEFACTS_BIN ?? join(root, "bin/solid-typefacts");
const update = process.argv.includes("--update");

if (!existsSync(native) || !existsSync(typeFacts)) {
  throw new Error("contract corpus requires fresh native and Type Facts binaries");
}

const corpus = JSON.parse(readFileSync(join(fixturesRoot, "corpus.json"), "utf8"));
if (corpus.schemaVersion !== 1 || corpus.format !== "solid-checker-package-contract-generator-corpus") {
  throw new Error("fixture corpus manifest is not stable schema version 1");
}
const fixtures = corpus.fixtures.map(name => join(fixturesRoot, name));
const temporary = mkdtempSync(join(tmpdir(), "solid-checker-contract-corpus-"));

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function assertEnvelope(path, expectedFormat, expectedVersionField, expectedVersion) {
  const document = JSON.parse(readFileSync(path, "utf8"));
  if (document.format !== expectedFormat || document[expectedVersionField] !== expectedVersion) {
    throw new Error(`${path} has the wrong package-contract workflow envelope`);
  }
  return document;
}

async function generate(directory) {
  const name = basename(directory);
  const output = join(temporary, `${name}.json`);
  const manifest = readFileSync(join(directory, "package.json"));
  const integrity = `fixture:sha256:${sha256(manifest)}`;
  let failure;
  try {
    await run(
      process.execPath,
      [
        cli,
        "contract",
        "generate",
        "--package-root",
        directory,
        "--output",
        output,
        "--integrity",
        integrity
      ],
      {
        cwd: root,
        env: {
          ...process.env,
          SOLID_CHECKER_NATIVE_BIN: native,
          SOLID_TYPEFACTS_BIN: typeFacts
        },
        encoding: "utf8",
        maxBuffer: 64 * 1024 * 1024
      }
    );
  } catch (error) {
    failure = String(error.stderr ?? error.message)
      .trim()
      .replaceAll(root, "<repository>")
      .replaceAll(temporary, "<temporary>");
  }
  const expected = join(directory, "expected.json");
  const expectedPlan = join(directory, "expected-proposal.json");
  const expectedRefusals = join(directory, "expected-refusals.json");
  const expectedRefusal = join(directory, "expected-refusal.txt");
  const refusalOutput = `${output}.refusals.json`;
  if (failure) {
    const rendered = `${failure}\n`;
    // A fixture that refuses every artifact case still writes the complete
    // census sidecar before taking the full-refusal exit. The thrown message
    // names only the first refusal, so pin the sidecar here exactly as the
    // success path does: otherwise a class, count, or reason can change — or a
    // whole inapplicable census can appear — behind an unchanged one-line
    // message.
    const audit = existsSync(refusalOutput)
      ? assertEnvelope(
          refusalOutput,
          "solid-checker-contract-proposal-refusals",
          "refusalVersion",
          1
        )
      : null;
    if (audit && (!Array.isArray(audit.refusals) || !Array.isArray(audit.inapplicable))) {
      throw new Error(`${name} produced an invalid artifact-case refusal sidecar`);
    }
    const auditedCases = audit
      ? audit.refusals.length + audit.inapplicable.length
      : 0;
    if (update) {
      writeFileSync(expectedRefusal, rendered);
      rmSync(expected, { force: true });
      rmSync(expectedPlan, { force: true });
      if (auditedCases > 0) copyFileSync(refusalOutput, expectedRefusals);
      else rmSync(expectedRefusals, { force: true });
    } else {
      if (!existsSync(expectedRefusal) || readFileSync(expectedRefusal, "utf8") !== rendered) {
        throw new Error(`${name} stable-v1 refusal differs; inspect and run --update intentionally\n${failure}`);
      }
      if (auditedCases > 0) {
        if (!existsSync(expectedRefusals)) {
          throw new Error(`${name} has no expected-refusals.json snapshot`);
        }
        if (!readFileSync(refusalOutput).equals(readFileSync(expectedRefusals))) {
          throw new Error(
            `${name} artifact-case refusal snapshot differs; inspect and run --update intentionally`
          );
        }
      } else if (existsSync(expectedRefusals)) {
        throw new Error(`${name} retains a stale expected-refusals.json snapshot`);
      }
    }
    return {
      name,
      refused: true,
      refusedArtifactCases: audit?.refusals.length ?? 0,
      inapplicableArtifactCases: audit?.inapplicable.length ?? 0,
      cases: 0,
      closureCandidates: 0,
      unresolvedClaims: 0,
      positiveOperations: 0
    };
  }
  if (update) rmSync(expectedRefusal, { force: true });
  const plan = `${output}.proposal.json`;
  const contract = assertEnvelope(output, "solid-reactivity-contract", "schemaVersion", 1);
  const planned = assertEnvelope(plan, "solid-checker-contract-proposal-plan", "planVersion", 1);
  const refusals = assertEnvelope(
    refusalOutput,
    "solid-checker-contract-proposal-refusals",
    "refusalVersion",
    1
  );
  if (contract.package.integrity !== integrity) {
    throw new Error(`${name} lost exact fixture package identity`);
  }
  if (planned.semanticDigest === "" || !Array.isArray(planned.unresolvedClaims)) {
    throw new Error(`${name} produced an incomplete proposal plan`);
  }
  if (
    refusals.package?.name !== contract.package.name ||
    refusals.package?.version !== contract.package.version ||
    !Array.isArray(refusals.refusals) ||
    !Array.isArray(refusals.inapplicable)
  ) {
    throw new Error(`${name} produced an invalid artifact-case refusal sidecar`);
  }
  // An inapplicable disposition is not a refusal, but it is still a recorded
  // census decision: pin the sidecar whenever either array carries a row, so a
  // disposition cannot appear, change class, or vanish unreviewed.
  const auditedCases = refusals.refusals.length + refusals.inapplicable.length;
  if (update) {
    copyFileSync(output, expected);
    copyFileSync(plan, expectedPlan);
    if (auditedCases > 0) {
      copyFileSync(refusalOutput, expectedRefusals);
    } else {
      rmSync(expectedRefusals, { force: true });
    }
  } else {
    if (!existsSync(expectedPlan)) throw new Error(`${name} has no expected-proposal.json snapshot`);
    if (!readFileSync(output).equals(readFileSync(expected))) {
      throw new Error(`${name} stable-v1 contract snapshot differs; inspect and run --update intentionally`);
    }
    if (!readFileSync(plan).equals(readFileSync(expectedPlan))) {
      throw new Error(`${name} stable-v1 proposal-plan snapshot differs; inspect and run --update intentionally`);
    }
    if (auditedCases > 0) {
      if (!existsSync(expectedRefusals)) {
        throw new Error(`${name} has no expected-refusals.json snapshot`);
      }
      if (!readFileSync(refusalOutput).equals(readFileSync(expectedRefusals))) {
        throw new Error(`${name} artifact-case refusal snapshot differs; inspect and run --update intentionally`);
      }
    } else if (existsSync(expectedRefusals)) {
      throw new Error(`${name} retains a stale expected-refusals.json snapshot`);
    }
  }
  return {
    name,
    refused: false,
    refusedArtifactCases: refusals.refusals.length,
    inapplicableArtifactCases: refusals.inapplicable.length,
    cases: Object.values(contract.entrypoints).reduce((count, entrypoint) => count + entrypoint.cases.length, 0),
    closureCandidates: planned.closureCandidates.length,
    unresolvedClaims: planned.unresolvedClaims.length,
    positiveOperations: planned.positiveOperations.length
  };
}

try {
  const rows = await mapPool(fixtures, generate, { concurrency: gateConcurrency() });
  const aggregate = rows.reduce(
    (result, row) => {
      result.cases += row.cases;
      result.refusedArtifactCases += row.refusedArtifactCases;
      result.inapplicableArtifactCases += row.inapplicableArtifactCases;
      result.closureCandidates += row.closureCandidates;
      result.unresolvedClaims += row.unresolvedClaims;
      result.positiveOperations += row.positiveOperations;
      return result;
    },
    {
      refusedArtifactCases: 0,
      inapplicableArtifactCases: 0,
      cases: 0,
      closureCandidates: 0,
      unresolvedClaims: 0,
      positiveOperations: 0
    }
  );
  console.log(
    `${update ? "updated" : "checked"} ${rows.length} stable-v1 generator fixtures: ` +
      `${rows.filter(row => row.refused).length} exact fail-closed refusals, ` +
      `${aggregate.refusedArtifactCases} local artifact-case refusals, ` +
      `${aggregate.inapplicableArtifactCases} inapplicable artifact cases, ` +
      `${aggregate.cases} artifact cases, ${aggregate.positiveOperations} possible operations, ` +
      `${aggregate.closureCandidates} proof candidates, ${aggregate.unresolvedClaims} local open claims`
  );
} finally {
  rmSync(temporary, { recursive: true, force: true });
}
