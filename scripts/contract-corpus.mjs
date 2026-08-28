#!/usr/bin/env bun

// Temporary-v2 generator corpus. Each fixture is acquired as an exact package
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
if (corpus.schemaVersion !== 2 || corpus.format !== "solid-checker-temporary-v2-generator-corpus") {
  throw new Error("fixture corpus manifest is not temporary schema version 2");
}
const fixtures = corpus.fixtures.map(name => join(fixturesRoot, name));
const temporary = mkdtempSync(join(tmpdir(), "solid-checker-contract-v2-corpus-"));

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function assertEnvelope(path, expectedFormat, expectedVersionField, expectedVersion) {
  const document = JSON.parse(readFileSync(path, "utf8"));
  if (document.format !== expectedFormat || document[expectedVersionField] !== expectedVersion) {
    throw new Error(`${path} has the wrong temporary-v2 workflow envelope`);
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
  const expectedRefusal = join(directory, "expected-refusal.txt");
  if (failure) {
    const rendered = `${failure}\n`;
    if (update) {
      writeFileSync(expectedRefusal, rendered);
      rmSync(expected, { force: true });
      rmSync(expectedPlan, { force: true });
    } else if (!existsSync(expectedRefusal) || readFileSync(expectedRefusal, "utf8") !== rendered) {
      throw new Error(`${name} temporary-v2 refusal differs; inspect and run --update intentionally\n${failure}`);
    }
    return { name, refused: true, cases: 0, closureCandidates: 0, unresolvedClaims: 0, positiveOperations: 0 };
  }
  if (update) rmSync(expectedRefusal, { force: true });
  const plan = `${output}.proposal.json`;
  const contract = assertEnvelope(output, "solid-reactivity-contract", "schemaVersion", 2);
  const planned = assertEnvelope(plan, "solid-checker-contract-proposal-plan", "planVersion", 1);
  if (contract.package.integrity !== integrity) {
    throw new Error(`${name} lost exact fixture package identity`);
  }
  if (planned.semanticDigest === "" || !Array.isArray(planned.unresolvedClaims)) {
    throw new Error(`${name} produced an incomplete proposal plan`);
  }
  if (update) {
    copyFileSync(output, expected);
    copyFileSync(plan, expectedPlan);
  } else {
    if (!existsSync(expectedPlan)) throw new Error(`${name} has no expected-proposal.json snapshot`);
    if (!readFileSync(output).equals(readFileSync(expected))) {
      throw new Error(`${name} temporary-v2 contract snapshot differs; inspect and run --update intentionally`);
    }
    if (!readFileSync(plan).equals(readFileSync(expectedPlan))) {
      throw new Error(`${name} temporary-v2 proposal-plan snapshot differs; inspect and run --update intentionally`);
    }
  }
  return {
    name,
    refused: false,
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
      result.closureCandidates += row.closureCandidates;
      result.unresolvedClaims += row.unresolvedClaims;
      result.positiveOperations += row.positiveOperations;
      return result;
    },
    { refusals: 0, cases: 0, closureCandidates: 0, unresolvedClaims: 0, positiveOperations: 0 }
  );
  console.log(
    `${update ? "updated" : "checked"} ${rows.length} temporary-v2 generator fixtures: ` +
      `${rows.filter(row => row.refused).length} exact fail-closed refusals, ` +
      `${aggregate.cases} artifact cases, ${aggregate.positiveOperations} possible operations, ` +
      `${aggregate.closureCandidates} proof candidates, ${aggregate.unresolvedClaims} local open claims`
  );
} finally {
  rmSync(temporary, { recursive: true, force: true });
}
