// `solid-checker contract verify <CONTRACT>`: RFC 0002 Stage 2.
//
// The mechanical promotion. It takes a generated contract and the probe report
// for its exact bytes, converts every positive claim the machine neither proved
// nor observed to the `{"status":"unknown"}` sentinel, and writes
// `evidence: {"kind": "verified"}` -- with no human decision anywhere in the
// path. `verified`'s reserved meaning in docs/package-contracts.md is already
// exactly "mechanical artifact/surface/behavior checks passed", and until this
// command no code in the repository wrote it.
//
// **Why its own verb, and not `contract review --promote verified`.** The review
// command's header rule is that certifying is a human decision per item, and its
// `generated-summary` plan item exists so that no row reaches `reviewed`
// evidence without a human decision naming its export. Mechanical promotion does
// not weaken that invariant; it declines to enter that tier. Folding it into
// `contract review` would have put a mode that takes zero decisions inside the
// command whose whole shape is decisions, so `--promote verified` there still
// refuses and points here.
//
// Where it sits in the pipeline:
//
//     contract generate  ->  contract probe --write  ->  contract verify
//                                                    \->  contract review   (the
//                                                         human tier, optional
//                                                         and composable: see
//                                                         the plan rewrite below)

import { randomUUID } from "node:crypto";
import { existsSync, readFileSync, renameSync, rmSync, statSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

import { runNative } from "../bin/launcher.mjs";
import { expandContract, normalizeContract } from "./contract-document.mjs";
import { sha256Bytes } from "./contract-probe-driver.mjs";
import {
  collectReviewItems,
  probeReportPath,
  renderReviewPlan,
  renderReviewPlanDocument,
  reviewPlanJsonPath,
  reviewPlanPath,
  reviewStatePath,
  verifyReportPath
} from "./contract-review-plan.mjs";
import {
  buildRefusalReport,
  buildVerifyReport,
  collectBlockers,
  convertUnconfirmedClaims,
  dropInferredRowEvidence
} from "./contract-verification.mjs";

export const contractVerifyHelp = `Usage:
  solid-checker contract verify <CONTRACT> [OPTIONS]

contract verify promotes a probed contract to verified evidence mechanically:
no human decision, reproducible by anyone with the same installed artifact. It
reads <contract>.probe.json, converts every positive claim that is neither
statically proven nor probed to the {"status": "unknown"} sentinel, drops the
inferred row markers certification rejects, and writes <contract>.verify.json
recording exactly what was lost and why.

Run contract probe --write first. Verification certifies what a probe observed,
so a contract with no probed evidence verifies to a document that claims almost
nothing.

A refusal also writes <contract>.verify.json, with the blockers it raised and
no evidence, conversion or promotion fields. The refusal path used to write
nothing at all, which left stderr as the only record of the most common
outcome.

It refuses -- one clear line each, contract untouched -- when a probe failed,
when a probe contradicted a negative claim, when any emitted entrypoint carries
a closure note, when the probe report is missing or is not the report for these
exact bytes, when a review of this contract has already recorded anything, or
when the promoted document does not validate.

A verified contract is not a reviewed one. Callback owner rows are permanently
out of a machine's reach, and every converted domain is an SC9005 uncertifiable
result at the consumer where the surface is touched. Resolve those with
contract review and promote to reviewed on top; the two compose.

Options:
  --probe-report <FILE>  Probe report to consume (default: <contract>.probe.json)
  --report <FILE>        Verify report output (default: <contract>.verify.json)
  -h, --help             Show this help
`;

function usage(message) {
  return new Error(`${message}\n\n${contractVerifyHelp}`);
}

export function parseVerifyArguments(arguments_) {
  const options = { contract: undefined, probeReport: undefined, report: undefined };
  const value = (flag, next) => {
    if (next === undefined || next.length === 0) throw usage(`${flag} requires a value`);
    return next;
  };
  for (let index = 0; index < arguments_.length; index += 1) {
    const argument = arguments_[index];
    if (argument === "--probe-report") options.probeReport = value(argument, arguments_[++index]);
    else if (argument === "--report") options.report = value(argument, arguments_[++index]);
    else if (argument.startsWith("-")) throw usage(`unknown argument ${argument}`);
    else if (options.contract) throw usage(`unexpected argument ${argument}`);
    else options.contract = argument;
  }
  if (!options.contract) throw usage("contract verify requires a contract path");
  return options;
}

function contractPath(argument) {
  const target = resolve(argument);
  return existsSync(target) && statSync(target).isDirectory()
    ? join(target, "solid-reactivity.json")
    : target;
}

function readJsonIfPresent(path, what) {
  if (!existsSync(path)) return undefined;
  try {
    return JSON.parse(readFileSync(path, "utf8"));
  } catch (error) {
    throw new Error(`could not read the ${what} at ${path}: ${error.message}`);
  }
}

function verifierIdentity() {
  const manifest = JSON.parse(
    readFileSync(fileURLToPath(new URL("../package.json", import.meta.url)), "utf8")
  );
  return `${manifest.name}@${manifest.version}`;
}

/// Every export summary as its own object, so one conversion is not shared
/// across the exports `expandContract` handed the same summary to.
function expandedContract(document) {
  const contract = expandContract(document);
  for (const entrypoint of Object.values(contract.entrypoints)) {
    for (const [name, summary] of Object.entries(entrypoint.exports)) {
      entrypoint.exports[name] = structuredClone(summary);
    }
  }
  return contract;
}

/// The kinds a review plan carries that no contract document witnesses.
///
/// They describe generation itself -- an entrypoint the generator refused, the
/// legacy manifest field a root resolved from, an artifact binding schema v1
/// cannot carry, an entrypoint that produced no summary -- so re-deriving the
/// plan from the verified bytes has to keep them verbatim rather than lose
/// them. `collectReviewItems` pushes exactly these before it walks the
/// entrypoints, so concatenating them in front reproduces the original order,
/// and therefore the original ids.
const GENERATION_ITEM_KINDS = new Set([
  "refused-entrypoint",
  "legacy-root-field",
  "artifact-binding",
  "no-export-summary"
]);

/// Re-binds the review plan to the verified bytes.
///
/// A plan is a set of questions about one exact document, and `contract review`
/// refuses a plan whose hash does not match the contract beside it -- so without
/// this, verifying would tell a reviewer to regenerate a contract that is
/// *fresher* than its plan, and the documented verify -> review -> reviewed
/// composition would be unreachable.
///
/// It is a rewrite and not the probe command's re-bind, because conversion
/// genuinely changes the questions: a converted `callbacks` domain is a new
/// `unknown-sentinel` item a human must answer, and the `no-callback-row`
/// question it replaces is gone. `contract probe --write` refuses to move a plan
/// whose item set changed for exactly the right reason -- probed evidence
/// provably raises no new question, so a new one there would be a question
/// re-bound past. Here the new questions *are* the product, and they are only
/// ever the reviewed tier's business: the verified document already certifies
/// without any of them being answered.
///
/// Safety is unchanged because the review *state* had to be empty for the
/// promotion to happen at all -- `collectBlockers` refuses otherwise -- so no
/// recorded decision can be re-blessed by the move.
///
/// **`because` survives the rewrite, in both directions.** The plan's `because`
/// block is the only place a reviewer learns *why* a claim is unknown -- which
/// proof obligation forced it, where, and how emission decided it belonged to
/// this export -- and re-deriving items from the promoted bytes threw all of it
/// away, because a contract document does not carry generation-time
/// attributions. Every item the rewrite reproduces therefore inherits the prior
/// plan's `because` by id (ids are derived from what an item is *about*, so an
/// unchanged question keeps its identity through the promotion), and every
/// sentinel this verification *created* gets one of its own, mirrored from the
/// conversion record in `<contract>.verify.json`. A reviewer facing a converted
/// domain now reads the same thing the sidecar says: the claim the machine held
/// and the reason the probe could not reach it.
export function rewriteReviewPlan({ plan, contract, contractHash, conversions = [] }) {
  const preserved = plan.items.filter(item => GENERATION_ITEM_KINDS.has(item.kind));
  const carried = new Map(
    plan.items.filter(item => item.because).map(item => [item.id, item.because])
  );
  const converted = new Map(
    conversions.map(conversion => [
      JSON.stringify([conversion.entrypoint, conversion.export, conversion.field]),
      conversion
    ])
  );
  const items = [...preserved, ...collectReviewItems(contract.entrypoints)].map(item => {
    const inherited = carried.get(item.id);
    const conversion =
      item.kind === "unknown-sentinel"
        ? converted.get(
            JSON.stringify([item.target.entrypoint, item.target.export, item.target.field])
          )
        : undefined;
    if (!inherited && !conversion) return item;
    return {
      ...item,
      because: {
        ...inherited,
        ...(conversion
          ? {
              conversion: {
                by: "contract verify",
                modes: conversion.modes,
                claims: conversion.claims
              }
            }
          : {})
      }
    };
  });
  return {
    document: renderReviewPlanDocument(
      plan.package?.name,
      plan.package?.version,
      items,
      plan.generation,
      contractHash
    ),
    items
  };
}

/// Writes the contract only after the document that will be written validates,
/// exactly as `--promote reviewed` and `contract probe --write` do, so a
/// rejected promotion never leaves a contract on disk the loader refuses.
function writeContract(contractFile, document) {
  const candidate = `${contractFile}.tmp-${randomUUID()}`;
  writeFileSync(candidate, document);
  try {
    const validation = runNative("solid-checker", ["--validate-contract", candidate], {
      encoding: "utf8",
      stdio: "pipe"
    });
    if (validation.error) throw validation.error;
    if (validation.status !== 0) {
      return (
        `the verified document for ${contractFile} does not validate, so the contract is ` +
        `unchanged: ${
          [validation.stderr, validation.stdout].filter(Boolean).join("\n").trim() ||
          `native solid-checker exited ${validation.status}`
        }`
      );
    }
    renameSync(candidate, contractFile);
    return "";
  } finally {
    rmSync(candidate, { force: true });
  }
}

export async function verifyContract(arguments_) {
  if (arguments_.includes("--help") || arguments_.includes("-h")) {
    process.stdout.write(contractVerifyHelp);
    return undefined;
  }
  const options = parseVerifyArguments(arguments_);
  const contractFile = contractPath(options.contract);
  if (!existsSync(contractFile)) throw new Error(`no contract at ${contractFile}`);
  const contractBytes = readFileSync(contractFile);
  const contractHash = sha256Bytes(contractBytes);
  const document = JSON.parse(contractBytes.toString("utf8"));
  const contract = expandedContract(document);
  const reportPath = options.probeReport
    ? resolve(options.probeReport)
    : probeReportPath(contractFile);
  const verifyPath = options.report ? resolve(options.report) : verifyReportPath(contractFile);

  // Idempotence, on the same three facts `--promote reviewed` uses: the sidecar
  // records a verification, the bytes on disk are still the ones it produced,
  // and the document says `verified`. Nothing is left to do and nothing is
  // written -- in particular the probe report, whose hash the promotion moved
  // past, is not re-checked into a refusal.
  const existing = readJsonIfPresent(verifyPath, "verify report");
  // A refusal sidecar is not a verification, and the idempotence check must not
  // read one as though it were. It carries no `contract.after`, so the test
  // below already fails on it -- but the sidecar it leaves behind is exactly
  // what a later successful run must be free to overwrite, which is why every
  // refusal path writes rather than appends.
  if (document.evidence?.kind === "verified" && existing?.contract?.after === contractHash) {
    process.stdout.write(
      `already verified ${contractFile} at ${existing.contract.before} -> ${contractHash}; nothing to do\n`
    );
    return existing;
  }
  /// Refuses, on stderr and on disk.
  ///
  /// Every `not verified` exit goes through here, so the sidecar exists for the
  /// same set of outcomes the stderr lines describe -- not for the subset one
  /// path happened to reach. The probe report is re-read leniently when the
  /// caller has not got one yet: a refusal that cannot parse it still records
  /// the refusal, because the blockers are the product and the report block is
  /// context.
  const refuse = (lines, context = {}) => {
    for (const line of lines) process.stderr.write(`solid-checker: not verified: ${line}\n`);
    // A refusal never overwrites a record of a promotion. A sidecar that
    // carries `evidence` is the audit trail of a verification that actually
    // happened -- of some other bytes, if it is still here after a
    // regeneration, and self-invalidating either way -- and replacing history
    // with the record of a failed attempt is a strictly worse artifact. A
    // refusal record replaces a refusal record; that is the only overwrite.
    if (existing && !existing.outcome && existing.evidence) {
      process.stdout.write(
        `not verified: ${lines.length} blocker(s); ${verifyPath} already records a verification of ` +
          `${existing.contract?.after ?? "other bytes"} and was left in place, so the refusal is on ` +
          "stderr only\n"
      );
      process.exitCode = 1;
      return undefined;
    }
    let refusalReport;
    try {
      refusalReport = context.report ?? readJsonIfPresent(reportPath, "probe report");
    } catch {
      refusalReport = undefined;
    }
    let refusalPlan = context.plan;
    if (refusalPlan === undefined) {
      try {
        refusalPlan = readJsonIfPresent(reviewPlanJsonPath(contractFile), "review plan");
      } catch {
        refusalPlan = undefined;
      }
    }
    writeFileSync(
      verifyPath,
      `${JSON.stringify(
        buildRefusalReport({
          contract,
          contractPath: contractFile,
          before: contractHash,
          report: refusalReport,
          reportPath,
          identities: {
            generator: refusalPlan?.generation?.generator ?? null,
            probeDriver: refusalReport?.identities?.probeDriver ?? null,
            verifier: verifierIdentity(),
            dialect: refusalReport?.identities?.dialect ?? null,
            runtime: refusalReport?.identities?.runtime ?? null,
            installed: {
              version: refusalReport?.package?.installedVersion ?? contract.package?.version,
              ...(refusalReport?.package?.integrity
                ? { integrity: refusalReport.package.integrity }
                : {})
            }
          },
          blockers: lines
        }),
        null,
        2
      )}\n`
    );
    process.stdout.write(
      `not verified: ${lines.length} blocker(s) recorded in ${verifyPath}; the contract is unchanged\n`
    );
    process.exitCode = 1;
    return undefined;
  };

  if (document.evidence?.kind && document.evidence.kind !== "inferred" && document.evidence.kind !== "generated") {
    return refuse([
      `${contractFile} already carries ${document.evidence.kind} evidence; ` +
        "mechanical verification promotes a generated draft and would replace a stronger claim with a " +
        "weaker one. Regenerate the contract, probe it, and verify the fresh document"
    ]);
  }

  const planPath = reviewPlanJsonPath(contractFile);
  const statePath = reviewStatePath(contractFile);
  const report = readJsonIfPresent(reportPath, "probe report");
  const plan = readJsonIfPresent(planPath, "review plan");
  const reviewState = readJsonIfPresent(statePath, "review state");

  const blockers = collectBlockers({
    contract,
    contractHash,
    contractPath: contractFile,
    report,
    reportPath,
    plan,
    planPath,
    reviewState,
    reviewStatePath: statePath
  });
  if (blockers.length) return refuse(blockers, { report, plan: plan ?? null });

  const converted = convertUnconfirmedClaims(contract, report);
  const droppedMarkers = dropInferredRowEvidence(converted.contract.entrypoints);
  // The one and only evidence write. The generator string is deliberately not
  // carried over: what a verified document attests is a mechanical check, not
  // who drafted the claims, and the generator identity is recorded in the
  // sidecar beside the probe-driver identity it has to be read with.
  const promoted = { ...converted.contract, evidence: { kind: "verified" } };
  const next = `${JSON.stringify(normalizeContract(promoted), null, 2)}\n`;

  const refusal = writeContract(contractFile, next);
  if (refusal) return refuse([refusal], { report, plan: plan ?? null });
  const afterHash = sha256Bytes(readFileSync(contractFile));

  const verifyReport = buildVerifyReport({
    contract: promoted,
    contractPath: contractFile,
    before: contractHash,
    after: afterHash,
    report,
    reportPath,
    identities: {
      generator: plan.generation?.generator ?? null,
      probeDriver: report.identities?.probeDriver ?? null,
      verifier: verifierIdentity(),
      dialect: report.identities?.dialect ?? null,
      runtime: report.identities?.runtime ?? null,
      installed: {
        version: report.package?.installedVersion ?? contract.package?.version,
        ...(report.package?.integrity ? { integrity: report.package.integrity } : {})
      }
    },
    conversions: converted.conversions,
    probed: converted.probed,
    staleMarkers: converted.staleMarkers,
    droppedMarkers
  });
  writeFileSync(verifyPath, `${JSON.stringify(verifyReport, null, 2)}\n`);

  const rewritten = rewriteReviewPlan({
    plan,
    contract: expandedContract(JSON.parse(next)),
    contractHash: afterHash,
    conversions: verifyReport.conversions
  });
  writeFileSync(planPath, `${JSON.stringify(rewritten.document, null, 2)}\n`);
  writeFileSync(
    reviewPlanPath(contractFile),
    renderReviewPlan(
      contract.package.name,
      contract.package.version,
      contractFile,
      rewritten.items
    ).text
  );

  for (const conversion of verifyReport.conversions) {
    process.stdout.write(
      `converted ${conversion.entrypoint}:${conversion.export} ${conversion.field} to unknown: ` +
        `${conversion.claims[0]?.reason ?? "not observed"}\n`
    );
  }
  for (const stale of verifyReport.staleProbedMarkers) {
    process.stdout.write(
      `stale probed marker ${stale.entrypoint}:${stale.export} ${stale.field} ` +
        `(${(stale.marker?.modes ?? []).join(", ") || "no modes"}): the consumed probe report does ` +
        `not witness ${stale.claim}\n`
    );
  }
  process.stdout.write(
    `verified ${contractFile}: ${verifyReport.summary.probedRows} probed row(s) kept, ` +
      `${verifyReport.summary.conversions} claim domain(s) converted to unknown, ` +
      `${verifyReport.summary.staleProbedMarkers} unwitnessed probed marker(s) discarded, ` +
      `${droppedMarkers} inferred row marker(s) dropped; report ${verifyPath}\n`
  );
  process.stdout.write(
    `${rewritten.items.length} review item(s) remain for the optional reviewed tier at ` +
      `${planPath}; the contract certifies as verified without them\n`
  );
  return verifyReport;
}

export { verifyReportPath };
