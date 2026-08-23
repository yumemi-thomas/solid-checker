// `solid-checker contract review` -- the promotion path for a generated
// contract, as a sequence of recorded decisions rather than a hand edit.
//
// The edit this exists to stop is the quiet one. A generated contract omits
// `callbacks` for every export the analyzer saw no callback in, and an omitted
// effect field is a *reviewed negative claim*: "this export never invokes a
// caller-supplied callback". Resolving an `{"status": "unknown"}` sentinel by
// deleting the field is the same negative claim, one keystroke away from a
// reviewer who meant "I have not decided yet". So certifying a negative is a
// decision this command demands per item (`absent`), never a default and never
// implicit, and promotion refuses while anything on the plan is undecided.

import { createHash, randomUUID } from "node:crypto";
import { existsSync, readFileSync, renameSync, rmSync, statSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import process from "node:process";
import { isDeepStrictEqual } from "node:util";

import { runNative } from "../bin/launcher.mjs";
import { expandContract, normalizeContract } from "./contract-document.mjs";
import {
  CONTRACT_WITNESSED_KINDS,
  collectReviewItems,
  isUnknownClaim,
  reviewPlanJsonPath,
  reviewStatePath
} from "./contract-review-plan.mjs";
import { pruneSummaryProbedMarkers } from "./contract-verification.mjs";

export const REVIEW_STATE_SCHEMA_VERSION = 1;

/// What a reviewer can say about an item, and about which items.
///
/// `verified`, `trusted` and `attested` are deliberately absent from the whole
/// command: they mean a mechanical artifact/surface/behavior check, an
/// out-of-band trust decision, and a verifier-produced release identity. This
/// records a human review, so `reviewed` is the only evidence kind it writes.
const DECISIONS = {
  // The generated claim or row is correct as generated. An unknown sentinel is
  // the one thing it cannot say: unknown is not evidence, so "confirming" it
  // would promote a marker that certifies nothing.
  confirm: {
    describe: "the generated claim is correct as generated",
    accepts: kind => kind !== "unknown-sentinel"
  },
  // `generated-summary` takes `confirm` and nothing else, deliberately. It is
  // raised for as long as the export exists, so there is no negative to certify
  // (`absent`) and no edit that answers it (`resolved-by-edit`): the question is
  // whether the rows the generator wrote, and the domains it omitted, describe
  // this export. Editing the contract changes the question rather than
  // resolving it, and the changed bytes make every resolution stale anyway.
  // Certify the negative, explicitly. For a sentinel this deletes the field at
  // promotion; for an export with no callback row the omission is already the
  // claim and nothing is written -- but the reviewer still has to say it.
  absent: {
    describe: "the behavior is certified absent",
    accepts: kind => kind === "unknown-sentinel" || kind === "no-callback-row"
  },
  // The reviewer hand-edited the contract to carry the audited value. Accepted
  // only once the contract's own bytes no longer raise the item.
  "resolved-by-edit": {
    describe: "the contract was edited to carry the audited value",
    accepts: kind => CONTRACT_WITNESSED_KINDS.has(kind)
  }
};

function usage(message) {
  return new Error(
    `${message}\nusage: solid-checker contract review <contract|directory> ` +
      "[--transfer-from CONTRACT] [--resolve ID=DECISION]... [--answers FILE] [--note TEXT] " +
      "[--promote reviewed]"
  );
}

function parseArguments(arguments_) {
  const options = {
    contract: "",
    transferFrom: "",
    resolutions: [],
    answers: "",
    note: "",
    promote: ""
  };
  for (let index = 0; index < arguments_.length; index++) {
    const argument = arguments_[index];
    if (!argument.startsWith("--")) {
      if (options.contract) throw usage(`unexpected argument ${argument}`);
      options.contract = argument;
      continue;
    }
    const separator = argument.indexOf("=");
    const key = separator === -1 ? argument : argument.slice(0, separator);
    const inline = separator === -1 ? undefined : argument.slice(separator + 1);
    const value = inline ?? arguments_[++index];
    if (value === undefined) throw usage(`${key} needs a value`);
    // An empty value used to disable the flag it was passed for: `--promote ""`
    // listed instead of promoting, and `--transfer-from ""` skipped the
    // transfer and then reported the untransferred plan as if that had been
    // asked for. A flag that silently does nothing is worse than one that
    // errors, because the exit code says the request was honored.
    if (value === "") throw usage(`${key} needs a non-empty value`);
    switch (key) {
      case "--resolve":
        options.resolutions.push(value);
        break;
      case "--transfer-from":
        options.transferFrom = value;
        break;
      case "--answers":
        options.answers = value;
        break;
      case "--note":
        options.note = value;
        break;
      case "--promote":
        options.promote = value;
        break;
      default:
        throw usage(`unknown contract review argument ${key}`);
    }
  }
  if (!options.contract) throw usage("contract review needs a contract path");
  if (options.promote && options.promote !== "reviewed") {
    throw usage(
      options.promote === "verified"
        ? "--promote verified is not this command's to make: contract review records a human " +
          "review, and verified means a mechanical check that takes no decision at all. " +
          "Run `solid-checker contract verify <contract>` instead -- it consumes the probe " +
          "report, converts every unconfirmed positive claim to the unknown sentinel, and " +
          "promotes with no human in the path"
        : `--promote ${options.promote} is not this command's to make: contract review records a ` +
          "human review, so it promotes only to reviewed"
    );
  }
  if (options.note && options.resolutions.length !== 1) {
    throw usage("--note accompanies exactly one --resolve");
  }
  // Last-wins is the wrong answer for two decisions about one item in one
  // invocation: the reviewer said two different things, and the command has no
  // way to know which one they meant.
  const seen = new Set();
  for (const argument of options.resolutions) {
    const [id] = parseResolution(argument);
    if (seen.has(id)) throw usage(`--resolve names ${id} more than once`);
    seen.add(id);
  }
  return options;
}

function contractPath(argument) {
  const path = resolve(argument);
  if (existsSync(path) && statSync(path).isDirectory()) {
    return join(path, "solid-reactivity.json");
  }
  return path;
}

function sha256Bytes(bytes) {
  return `sha256:${createHash("sha256").update(bytes).digest("hex")}`;
}

function readJson(path, what) {
  try {
    return JSON.parse(readFileSync(path, "utf8"));
  } catch (error) {
    throw new Error(`could not read the ${what} at ${path}: ${error.message}`);
  }
}

/// Every export summary as its own object.
///
/// `expandContract` hands the same summary object to every export that shares
/// it, so one deletion would otherwise silently apply to all of them.
function expandedContract(document) {
  const contract = expandContract(document);
  for (const entrypoint of Object.values(contract.entrypoints)) {
    for (const [name, summary] of Object.entries(entrypoint.exports)) {
      entrypoint.exports[name] = structuredClone(summary);
    }
  }
  return contract;
}

function parseFieldPath(field) {
  const segments = [];
  for (const part of field.split(".")) {
    const match = /^([A-Za-z_$][\w$]*)((?:\[\d+\])*)$/.exec(part);
    if (!match) throw new Error(`unsupported review target field ${field}`);
    segments.push(match[1]);
    for (const index of match[2].matchAll(/\[(\d+)\]/g)) segments.push(Number(index[1]));
  }
  return segments;
}

function valueAt(root, segments) {
  let value = root;
  for (const segment of segments) {
    if (value == null || typeof value !== "object") return undefined;
    value = value[segment];
  }
  return value;
}

function deleteAt(root, segments) {
  const parent = valueAt(root, segments.slice(0, -1));
  if (parent == null || typeof parent !== "object") return false;
  const last = segments.at(-1);
  if (!(last in parent)) return false;
  delete parent[last];
  return true;
}

/// Drops `inferred` row markers so promotion actually promotes.
///
/// Certification rejects a promoted contract that still carries an inferred
/// row (`claims_are_certifiable` in rust/crates/solid-reactive-ir/src/lib.rs),
/// while a row with *no* evidence of its own inherits the document's. Removing
/// the marker is therefore the honest operation: it says "the review this
/// document records covers this row", where writing `reviewed` onto each row
/// would claim a per-row human assertion nobody made. Probed and
/// inherited-from rows are left exactly as they are.
function dropInferredRowEvidence(value) {
  if (Array.isArray(value)) {
    let dropped = 0;
    for (const element of value) dropped += dropInferredRowEvidence(element);
    return dropped;
  }
  if (!value || typeof value !== "object") return 0;
  let dropped = 0;
  if (value.evidence?.kind === "inferred") {
    delete value.evidence;
    dropped += 1;
  }
  for (const child of Object.values(value)) dropped += dropInferredRowEvidence(child);
  return dropped;
}

function unknownSentinels(contract) {
  return collectReviewItems(contract.entrypoints).filter(
    item => item.kind === "unknown-sentinel"
  );
}

function loadState(path) {
  if (!existsSync(path)) {
    return { schemaVersion: REVIEW_STATE_SCHEMA_VERSION, contract: "", resolutions: {} };
  }
  const state = readJson(path, "review state");
  if (state.schemaVersion !== REVIEW_STATE_SCHEMA_VERSION) {
    throw new Error(
      `review state ${path} has schema version ${state.schemaVersion}; this checker writes ${REVIEW_STATE_SCHEMA_VERSION}`
    );
  }
  // `plan` is what makes this state's own plan recognizable after an edit or a
  // promotion has moved the contract's bytes, so a non-string here is not a
  // field to ignore: it decides whether a plan beside the contract is accepted.
  if (state.plan !== undefined && typeof state.plan !== "string") {
    throw new Error(
      `review state ${path} has a "plan" field that is not a contract hash string; this checker ` +
        "will not overwrite a review state it cannot read"
    );
  }
  // A malformed `resolutions` used to be replaced by an empty object, so the
  // run reported "recorded" for decisions it had just discarded along with
  // every decision already on disk. This file is the audit trail; a shape it
  // cannot read is a refusal, never a silent reset.
  const resolutions = state.resolutions ?? {};
  if (typeof resolutions !== "object" || resolutions === null || Array.isArray(resolutions)) {
    throw new Error(
      `review state ${path} has a "resolutions" field that is not an {id: resolution} object; ` +
        "this checker will not overwrite a review state it cannot read"
    );
  }
  for (const [id, resolution] of Object.entries(resolutions)) {
    if (
      typeof resolution !== "object" ||
      resolution === null ||
      Array.isArray(resolution) ||
      typeof resolution.decision !== "string"
    ) {
      throw new Error(
        `review state ${path} records ${id} without a string "decision"; this checker will not ` +
          "overwrite a review state it cannot read"
      );
    }
  }
  return { ...state, resolutions };
}

function writeState(path, state) {
  writeFileSync(path, `${JSON.stringify(state, null, 2)}\n`);
}

function parseResolution(argument) {
  const separator = argument.indexOf("=");
  if (separator === -1) throw usage(`--resolve takes ID=DECISION, not ${argument}`);
  return [argument.slice(0, separator), argument.slice(separator + 1)];
}

/// Why two closure records do not describe the same bytes, or "" when they do.
///
/// A note is an omission -- a target whose closure could not be walked, a
/// module whose bytes were unreadable -- so a record carrying one does not
/// establish what the summaries were derived from, and cannot establish that
/// two generations were derived from the same thing. Both sides are emitted
/// sorted, so equal content compares position by position.
function closureDifference(left, right) {
  if (left.notes?.length || right.notes?.length) {
    return "its closure record is incomplete: a module's bytes were not recorded at generation";
  }
  if (!Array.isArray(left.modules) || !Array.isArray(right.modules) || !left.modules.length) {
    return "its closure record names no module";
  }
  if (!isDeepStrictEqual(left.targets, right.targets)) return "its runtime targets changed";
  if (
    left.modules.length !== right.modules.length ||
    left.modules.some(
      (module, index) =>
        module.path !== right.modules[index].path || module.hash !== right.modules[index].hash
    )
  ) {
    return "its runtime module closure changed";
  }
  return "";
}

/// One entrypoint with the previous review's own mutations applied, so two
/// contracts at different points in the review flow compare on equal terms.
///
/// A promoted contract is not the document generation wrote: promotion deleted
/// the claims certified `absent` and dropped the `inferred` row markers the
/// review resolved. Applying that same projection to both sides makes the
/// comparison ask what it means to ask -- do these two generations describe the
/// same behavior -- instead of re-detecting the earlier promotion as a
/// difference. It is idempotent, so an unpromoted previous contract compares
/// just as well. `probed` and `inherited-from` markers survive it, which is why
/// a row that changed which package it was inherited from still blocks.
function projectedEntrypoint(entry, absentFields) {
  const projected = structuredClone(entry);
  for (const [exportName, field] of absentFields) {
    const summary = projected.exports?.[exportName];
    if (!summary) continue;
    const segments = parseFieldPath(field);
    if (!isUnknownClaim(valueAt(summary, segments))) continue;
    deleteAt(summary, segments);
  }
  dropInferredRowEvidence(projected);
  return projected;
}

/// Carry a previous review's conclusions onto a regenerated contract.
///
/// A contract binds to one artifact, so every upstream release turns a reviewed
/// contract into a stale one. Re-reviewing the whole package each time is what
/// makes a reviewed corpus rot with ecosystem velocity; re-reviewing the diff is
/// what makes it maintainable. The soundness rule is byte identity, per
/// entrypoint: a conclusion transfers only when the new summaries were derived
/// from exactly the modules the reviewer resolved against. Anything else -- a
/// changed module, a closure neither side recorded, two generations that
/// disagree about the summaries -- leaves the item open. Fail closed: an
/// unreviewed conclusion promoted as a reviewed one is precisely the silent
/// certification this command exists to prevent.
function transferReview({ from, contractFile, contractHash, contract, plan, state, stillRaised }) {
  const oldContractFile = contractPath(from);
  if (!existsSync(oldContractFile)) throw new Error(`no contract at ${oldContractFile}`);
  // A machine verification is not a review, so there is nothing here to carry.
  // It is a pure function of the installed artifact, the generator identity and
  // the probe-driver identity -- which means the upgrade path *reproduces* it
  // rather than transferring it, and reproducing it is two commands. Transfer
  // exists because a human review is expensive and non-reproducible; borrowing
  // it for the tier that is neither would launder an observation of one release
  // into a claim about another.
  if (JSON.parse(readFileSync(oldContractFile, "utf8")).evidence?.kind === "verified") {
    throw new Error(
      `${oldContractFile} carries verified evidence, which \`solid-checker contract verify\` ` +
        "produced mechanically; a verification is reproduced, never transferred, because it is " +
        "an observation of the bytes that were installed when it ran. Probe and verify the " +
        `regenerated contract instead: solid-checker contract probe ${contractFile} --write && ` +
        `solid-checker contract verify ${contractFile}`
    );
  }
  const oldStatePath = reviewStatePath(oldContractFile);
  if (!existsSync(oldStatePath)) {
    throw new Error(
      `no review state at ${oldStatePath}: ${oldContractFile} records no review, so it has no ` +
        "reviewed conclusion to transfer. Review the new contract on its own, or point " +
        "--transfer-from at a contract that was reviewed"
    );
  }
  const oldState = loadState(oldStatePath);
  const oldBytes = readFileSync(oldContractFile);
  const oldHash = sha256Bytes(oldBytes);
  const oldContract = expandedContract(JSON.parse(oldBytes.toString("utf8")));
  if (oldContract.package?.name !== contract.package?.name) {
    throw new Error(
      `${oldContractFile} describes ${oldContract.package?.name} and ${contractFile} describes ` +
        `${contract.package?.name}; a review transfers within one package, across its versions`
    );
  }
  // The per-resolution hashes already refuse a decision recorded against other
  // bytes. Saying it once at the document level too is what makes the source's
  // own consistency an explicit precondition rather than an emergent one: a
  // state whose `contract` is not this file's hash describes some other
  // document, and reading a "reviewed conclusion" off it is a guess.
  if (oldState.contract && oldState.contract !== oldHash) {
    throw new Error(
      `${oldStatePath} records its review against ${oldState.contract} and ${oldContractFile} ` +
        `hashes to ${oldHash}; that review is not a review of these bytes, so there is no ` +
        "conclusion to transfer. Re-review the old contract, or transfer from the bytes it was " +
        "reviewed against"
    );
  }
  const local = Object.keys(state.resolutions).filter(id => !state.resolutions[id].transferred);
  if (local.length) {
    throw new Error(
      `${reviewStatePath(contractFile)} already records ${local.length} resolution(s) against ` +
        "other bytes than the ones being transferred onto, so a review of this contract is " +
        "already under way; transfer is the first step of a re-review and merging it into " +
        "decisions already taken would leave the audit trail unable to say which is which. " +
        "Delete that review state and transfer first, or continue the review without " +
        "--transfer-from"
    );
  }

  // A resolution recorded against other bytes than the old contract's own is
  // stale there too, and a stale conclusion is not one to carry anywhere.
  const previousOf = id => {
    const resolution = oldState.resolutions?.[id];
    if (!resolution || resolution.contract !== oldHash) return undefined;
    return resolution;
  };

  const absentByEntrypoint = new Map();
  for (const item of plan.items) {
    if (item.kind !== "unknown-sentinel") continue;
    if (previousOf(item.id)?.decision !== "absent") continue;
    const fields = absentByEntrypoint.get(item.target.entrypoint) ?? [];
    fields.push([item.target.export, item.target.field]);
    absentByEntrypoint.set(item.target.entrypoint, fields);
  }

  const verdicts = new Map();
  const verdictFor = name => {
    if (verdicts.has(name)) return verdicts.get(name);
    const verdict = (() => {
      const oldClosure = oldState.closures?.entrypoints?.[name];
      const newClosure = plan.generation?.entrypoints?.[name];
      if (!oldClosure) return "the previous review state records no module closure for it";
      if (!newClosure) return "the review plan records no module closure for it";
      const difference = closureDifference(oldClosure, newClosure);
      if (difference) return difference;
      const oldEntry = oldContract.entrypoints[name];
      const newEntry = contract.entrypoints[name];
      if (!oldEntry || !newEntry) return "it is absent from one of the two contracts";
      const absentFields = absentByEntrypoint.get(name) ?? [];
      return isDeepStrictEqual(
        projectedEntrypoint(oldEntry, absentFields),
        projectedEntrypoint(newEntry, absentFields)
      )
        ? ""
        : "the two generations disagree about its export summaries";
    })();
    verdicts.set(name, verdict);
    return verdict;
  };

  // Whole-transfer preconditions. Neither is about one entrypoint: a different
  // compiler-facts protocol means the two contracts speak different claim
  // vocabularies, and a different generator means the summaries and the closure
  // enumeration behind them were produced by different code -- in both cases
  // byte-identical inputs no longer imply an equivalent review.
  const globalBlock = (() => {
    if (oldContract.compilerFactsProtocol !== contract.compilerFactsProtocol) {
      return (
        `the two contracts record compiler-facts protocol ${oldContract.compilerFactsProtocol} ` +
        `and ${contract.compilerFactsProtocol}`
      );
    }
    const oldGenerator = oldState.closures?.generator;
    const newGenerator = plan.generation?.generator;
    if (oldGenerator && newGenerator && oldGenerator !== newGenerator) {
      return `the two plans were written by ${oldGenerator} and ${newGenerator}`;
    }
    return "";
  })();

  // An artifact-binding item is about the document rather than one entrypoint,
  // and it is the item every project-owned (out-of-package) contract carries --
  // so leaving it permanently non-transferable made the documented version-bump
  // fast path unreachable for exactly the tier it was written for. It transfers
  // on the strongest condition available instead of on none: every entrypoint
  // either contract knows about satisfies the full byte rule, so nothing the
  // binding could be about changed.
  const bindingVerdict = () => {
    const names = new Set([
      ...Object.keys(oldContract.entrypoints ?? {}),
      ...Object.keys(contract.entrypoints ?? {})
    ]);
    if (!names.size) return "neither contract records an entrypoint";
    for (const name of [...names].sort()) {
      const verdict = verdictFor(name);
      if (verdict) return `${name}: ${verdict}`;
    }
    return "";
  };

  const lines = [];
  const openReasons = new Map();
  const stayOpen = (name, reason) => {
    const key = JSON.stringify([name, reason]);
    const row = openReasons.get(key) ?? { name, reason, count: 0 };
    row.count += 1;
    openReasons.set(key, row);
  };
  let transferred = 0;
  for (const item of plan.items) {
    const name = item.target?.entrypoint ?? "(no entrypoint)";
    if (globalBlock) {
      stayOpen("(all)", globalBlock);
      continue;
    }
    // A refused entrypoint has no summaries, so no closure was derived for it
    // and nothing witnesses that the refusal is the same refusal; an entrypoint
    // with no export summary is the same fact from the other side.
    if (item.kind === "refused-entrypoint" || item.kind === "no-export-summary") {
      stayOpen(
        name,
        `a ${item.kind} item is about an entrypoint the contract does not describe, so no module closure witnesses it`
      );
      continue;
    }
    if (item.kind === "artifact-binding") {
      const verdict = bindingVerdict();
      if (verdict) {
        stayOpen(
          "(artifact binding)",
          `the binding covers every entrypoint and one of them changed -- ${verdict}`
        );
        continue;
      }
    } else if (!item.target?.entrypoint) {
      stayOpen(
        "(no entrypoint)",
        "the item is about generation itself, which no module closure witnesses"
      );
      continue;
    } else {
      const verdict = verdictFor(name);
      if (verdict) {
        stayOpen(name, verdict);
        continue;
      }
    }
    // The legacy manifest field a root entrypoint resolved from is not in the
    // module closure: a republish that leaves `module` byte-identical and adds
    // a divergent `main` changes what the reviewer was asked to confirm while
    // every hash stays equal.
    if (item.kind === "legacy-root-field") {
      const oldLegacy = oldState.closures?.legacyRoot;
      const newLegacy = plan.generation?.legacyRoot;
      if (oldLegacy && newLegacy && !isDeepStrictEqual(oldLegacy, newLegacy)) {
        stayOpen(name, "the legacy manifest field the root resolves from changed");
        continue;
      }
    }
    const previous = previousOf(item.id);
    if (!previous) {
      stayOpen(name, "no prior resolution to transfer");
      continue;
    }
    // The universal condition, and the one that does not depend on any item
    // kind knowing about its own inputs: an id is derived from what an item is
    // *about*, and two items that are about the same thing can still ask
    // different questions. Only the text says which question was answered, so
    // the state carries the text the reviewer saw and a transfer requires it to
    // be the text the new plan shows.
    if (typeof previous.text !== "string") {
      stayOpen(name, "the previous review state does not record what the item said");
      continue;
    }
    if (previous.text !== item.text) {
      stayOpen(name, "the item says something different than the one that was resolved");
      continue;
    }
    if (!DECISIONS[previous.decision]?.accepts(item.kind)) {
      stayOpen(name, `the prior ${previous.decision} decision does not apply to a ${item.kind} item`);
      continue;
    }
    // The standard resolved-by-edit acceptance check, against the new bytes: the
    // edit that answered the item lives in the old contract, and the new one
    // carries it only if the reviewer made it there too.
    if (previous.decision === "resolved-by-edit" && stillRaised(item)) {
      stayOpen(name, "a resolved-by-edit conclusion the new contract still raises");
      continue;
    }
    const existing = state.resolutions[item.id];
    const unchanged =
      existing?.decision === previous.decision &&
      existing?.transferred?.from === oldHash &&
      existing?.transferred?.at === previous.at;
    state.resolutions[item.id] = {
      decision: previous.decision,
      at: unchanged ? existing.at : new Date().toISOString(),
      ...(previous.note ? { note: previous.note } : {}),
      // What the reviewer was looking at, carried forward so the *next*
      // transfer can make the same comparison against a plan file that this
      // regeneration has already overwritten.
      text: item.text,
      contract: contractHash,
      // Provenance, not a second opinion: this decision was made about other
      // bytes, and the transfer rule is the only reason it applies to these.
      transferred: { from: oldHash, at: previous.at }
    };
    transferred += 1;
    if (!unchanged) lines.push(`transferred ${item.id} ${previous.decision}`);
  }

  for (const { name, reason, count } of openReasons.values()) {
    lines.push(`open ${name}: ${count} item(s) not transferable: ${reason}`);
  }
  lines.push(
    `transferred ${transferred} of ${plan.items.length} review item(s) from ${oldContractFile} ` +
      `(${oldHash}); ${plan.items.length - transferred} remain open`
  );
  return lines;
}

export async function reviewContract(arguments_) {
  if (arguments_.includes("--help") || arguments_.includes("-h")) {
    const { packageContractHelp } = await import("./generate-package-contract.mjs");
    process.stdout.write(packageContractHelp);
    return;
  }
  const options = parseArguments(arguments_);
  const contractFile = contractPath(options.contract);
  if (!existsSync(contractFile)) throw new Error(`no contract at ${contractFile}`);
  const planFile = reviewPlanJsonPath(contractFile);
  if (!existsSync(planFile)) {
    throw new Error(
      `no review plan at ${planFile}; regenerate the contract with ` +
        "`solid-checker contract generate` to write one"
    );
  }
  const plan = readJson(planFile, "review plan");
  if (plan.schemaVersion !== 1 || !Array.isArray(plan.items)) {
    throw new Error(`review plan ${planFile} is not a schema-version-1 plan`);
  }
  const statePath = reviewStatePath(contractFile);

  let contractBytes = readFileSync(contractFile);
  let contractHash = sha256Bytes(contractBytes);
  let contract = expandedContract(JSON.parse(contractBytes.toString("utf8")));
  if (contract.package?.name !== plan.package?.name) {
    throw new Error(
      `review plan ${planFile} describes ${plan.package?.name} and the contract describes ${contract.package?.name}`
    );
  }
  const state = loadState(statePath);
  const byId = new Map(plan.items.map(item => [item.id, item]));

  // The plan is bound to the contract it was written beside, and every mode
  // refuses when the pairing is broken. Validating on package *name* -- which
  // is what this used to do -- accepts v1's plan sitting next to v2's contract,
  // and then resolves v2 by answering questions asked about v1.
  //
  // Two things legitimately move the contract's bytes away from the hash its
  // plan carries, and both are this command's own doing: a hand edit, which is
  // what `resolved-by-edit` exists for, and the promotion at the end. Binding on
  // the *current* hash alone would therefore delete both from the product.
  //
  // So the state records which plan this review answers, once, and never
  // rewrites it: `state.plan` is the plan's `contract` hash, not the contract's.
  // It survives every edit and the promotion, which is exactly why it is a
  // different field from `state.contract` -- that one tracks the bytes on disk
  // and moves with them, so keying acceptance off it broke as soon as the two
  // halves of an edit happened in two invocations, which is how a human does
  // it. A plan copied in from another contract can never satisfy this: the only
  // way `state.plan` gets a value is a write that already passed the binding,
  // whose base case is the pristine hash match.
  //
  // Safety is unchanged, because acceptance is not approval: the per-resolution
  // hashes still make every decision recorded before an edit stale, and stale
  // counts as open.
  const boundToContract =
    plan.contract === contractHash ||
    (typeof state.plan === "string" && state.plan.length > 0 && state.plan === plan.contract);
  if (!boundToContract) {
    throw new Error(
      typeof plan.contract === "string"
        ? `review plan ${planFile} was written for contract bytes ${plan.contract} and ` +
          `${contractFile} hashes to ${contractHash}; regenerate the contract to write a ` +
          "matching review plan"
        : `review plan ${planFile} records no contract hash, so nothing binds it to ` +
          `${contractFile}; regenerate the contract to write a matching review plan`
    );
  }

  // Re-derived from the contract as it stands now, which is how a
  // `resolved-by-edit` claim is checked: the item is gone from the current
  // bytes, or the claim is refused.
  const raised = collectReviewItems(contract.entrypoints);
  const witnessed = new Set(raised.map(item => item.id));
  const stillRaised = item =>
    CONTRACT_WITNESSED_KINDS.has(item.kind) ? witnessed.has(item.id) : true;

  // Everything is parsed and checked before anything is written. A bad flag in
  // the last position used to error *after* a transfer in the first position had
  // already rewritten the review state, so a run that exited non-zero had still
  // changed the audit trail.
  const answers = new Map();
  if (options.answers) {
    const document = readJson(resolve(options.answers), "answers file");
    if (!document || typeof document !== "object" || Array.isArray(document)) {
      throw new Error(`answers file ${options.answers} is not an {id: decision} object`);
    }
    for (const [id, decision] of Object.entries(document)) {
      if (typeof decision !== "string") {
        throw new Error(`answers file ${options.answers} maps ${id} to a non-string decision`);
      }
      answers.set(id, { decision });
    }
  }
  for (const argument of options.resolutions) {
    const [id, decision] = parseResolution(argument);
    answers.set(id, { decision, ...(options.note ? { note: options.note } : {}) });
  }
  const validated = [];
  for (const [id, { decision, note }] of answers) {
    const item = byId.get(id);
    if (!item) {
      throw new Error(
        `${id} is not a review item of ${planFile}; run \`solid-checker contract review ${contractFile}\` to list them`
      );
    }
    const rule = DECISIONS[decision];
    if (!rule) {
      throw new Error(
        `${decision} is not a review decision; use ${Object.keys(DECISIONS).join(", ")}`
      );
    }
    if (!rule.accepts(item.kind)) {
      throw new Error(
        decision === "confirm"
          ? `${id} is an unknown claim: unknown is not evidence and cannot be confirmed. Resolve it with absent to certify the behavior is absent, or edit the contract to carry the audited value and record resolved-by-edit`
          : `${decision} does not apply to a ${item.kind} item (${id}); it means "${rule.describe}"`
      );
    }
    if (decision === "resolved-by-edit" && stillRaised(item)) {
      throw new Error(
        `${id} is still raised by the contract at ${contractFile}: ${item.text}. Edit the contract first, then record resolved-by-edit`
      );
    }
    validated.push({ item, decision, note });
  }

  // The closure block a resolution is recorded against always comes from the
  // plan that raised it. Falling back to whatever the state already carried let
  // a plan with no generation block inherit the previous contract's closure
  // hashes, and a later transfer then compared the new summaries against bytes
  // they were never derived from.
  const planClosures = () => plan.generation ?? { entrypoints: {} };

  /// One shape for every write, because the three fields it fixes have to move
  /// together and one of them must not move at all.
  ///
  /// `contract` tracks the bytes on disk and changes with them; `plan` records
  /// which plan this review answers and is the same value on every write, which
  /// is what lets a hand edit and its re-resolution happen in two invocations
  /// instead of one.
  const persist = () => {
    state.schemaVersion = REVIEW_STATE_SCHEMA_VERSION;
    state.contract = contractHash;
    state.plan = plan.contract;
    state.closures = planClosures();
    writeState(statePath, state);
  };

  if (options.transferFrom) {
    if (state.promoted) {
      throw new Error(
        `${statePath} records ${contractFile} as already promoted to ${state.promoted.evidence} ` +
          "evidence; there is nothing to transfer onto a completed review. Regenerate the " +
          "contract and transfer onto the fresh review state"
      );
    }
    const lines = transferReview({
      from: options.transferFrom,
      contractFile,
      contractHash,
      contract,
      plan,
      state,
      stillRaised
    });
    persist();
    for (const line of lines) process.stdout.write(`${line}\n`);
  }

  if (validated.length) {
    const recorded = [];
    for (const { item, decision, note } of validated) {
      state.resolutions[item.id] = {
        decision,
        at: new Date().toISOString(),
        ...(note ? { note } : {}),
        // What this decision was an answer to. The plan file is rewritten by
        // the next regeneration, so the state is the only place the question a
        // reviewer actually saw survives -- and a transfer that cannot compare
        // the questions can only compare ids, which are deliberately blind to
        // what an item says.
        text: item.text,
        // Per resolution, not only per file: a later hand edit changes the
        // bytes, and a document-level hash would silently re-bless every
        // decision recorded against the bytes before it.
        contract: contractHash
      };
      recorded.push(`recorded ${item.id} ${decision}`);
    }
    delete state.promoted;
    persist();
    for (const line of recorded) process.stdout.write(`${line}\n`);
  }

  const status = item => {
    const resolution = state.resolutions[item.id];
    if (!resolution) return "open";
    if (resolution.contract !== contractHash) return "stale";
    return "resolved";
  };

  // A promotion that already happened, asked for again.
  //
  // Re-promoting used to refuse, and for a reason that had nothing to do with
  // the review: promotion deletes the `absent`-certified sentinels, which turns
  // every such export into one with no callback row, so the gate below saw a
  // `no-callback-row` question the plan -- written before the deletion -- does
  // not list, and told the reviewer to regenerate a contract that was already
  // finished. The three facts here identify the promotion this state describes:
  // it happened, the bytes on disk are still the ones it produced, and the
  // document says `reviewed`. Nothing is left to do, and nothing is written.
  const alreadyPromoted =
    Boolean(state.promoted) &&
    state.contract === contractHash &&
    contract.evidence?.kind === state.promoted?.evidence;
  if (options.promote && alreadyPromoted) {
    process.stdout.write(
      `already promoted ${contractFile} to ${state.promoted.evidence} evidence at ${state.promoted.at}; nothing to do\n`
    );
  }

  if (options.promote && !alreadyPromoted) {
    const refusals = [];
    for (const item of plan.items) {
      const state_ = status(item);
      if (state_ === "open") refusals.push(`open review item ${item.id} ${item.kind}: ${item.text}`);
      else if (state_ === "stale") {
        refusals.push(
          `stale resolution for ${item.id} ${item.kind}: it was recorded against different contract bytes, so re-review and re-resolve it`
        );
      } else if (
        state.resolutions[item.id].decision === "resolved-by-edit" &&
        stillRaised(item)
      ) {
        refusals.push(
          `${item.id} is recorded resolved-by-edit but the contract still raises it: ${item.text}`
        );
      }
    }
    // What the contract raises *now*, which a hand edit can have changed since
    // the plan was written: a question the plan never asked has never been
    // reviewed, and promoting past it is exactly the silent certification this
    // command exists to prevent.
    for (const item of raised) {
      if (!byId.has(item.id)) {
        refusals.push(
          `the contract raises ${item.kind}: ${item.text}, which the review plan at ${planFile} does not list; regenerate the contract so the question is reviewed rather than promoted unseen`
        );
        continue;
      }
      if (item.kind !== "unknown-sentinel") continue;
      const decision = state.resolutions[item.id]?.decision;
      if (decision !== "absent" && decision !== "resolved-by-edit") {
        refusals.push(
          `unknown claim ${item.text} remains in the contract; certify it absent or edit the contract to carry the audited value`
        );
      }
    }
    if (refusals.length) {
      for (const refusal of refusals) {
        process.stderr.write(`solid-checker: not promoted: ${refusal}\n`);
      }
      process.exitCode = 1;
      return;
    }

    // Only two mutations are legal here, plus the marker drop below: no
    // entrypoint and no export may leave the document through a promotion.
    let deleted = 0;
    for (const item of plan.items) {
      if (item.kind !== "unknown-sentinel") continue;
      if (state.resolutions[item.id].decision !== "absent") continue;
      const summary = contract.entrypoints[item.target.entrypoint]?.exports?.[item.target.export];
      if (!summary) continue;
      const segments = parseFieldPath(item.target.field);
      if (!isUnknownClaim(valueAt(summary, segments))) continue;
      if (deleteAt(summary, segments)) deleted += 1;
    }
    // A summary-level `probed` marker asserts an observation of the claims the
    // summary states. Certifying one of those claims absent deletes it, and the
    // marker then asserts an observation of nothing while every row with no
    // evidence of its own still inherits it. Recomputed here for the same
    // reason `contract verify` recomputes it after a conversion.
    const staleSummaryMarkers = pruneSummaryProbedMarkers(contract.entrypoints);
    const droppedMarkers = dropInferredRowEvidence(contract.entrypoints);
    const preHash = contractHash;
    contract.evidence = { ...contract.evidence, kind: "reviewed" };

    // Validate what will be written, then write it -- not the other way round.
    //
    // Writing first left a rejected promotion on disk *with* `evidence:
    // reviewed` and a `promoted` review state beside it, so the very next
    // `contract review` listing found every item resolved and exited 0: a
    // document the loader refuses, reported by the gate as a completed review.
    // The temporary file lives in the contract's own directory so the rename
    // that installs it is atomic on the same filesystem, and so the relative
    // artifact paths the validator resolves mean the same thing they will mean
    // at the final path.
    const candidate = `${contractFile}.tmp-${randomUUID()}`;
    writeFileSync(candidate, `${JSON.stringify(normalizeContract(contract), null, 2)}\n`);
    const validation = runNative("solid-checker", ["--validate-contract", candidate], {
      encoding: "utf8",
      stdio: "pipe"
    });
    if (validation.error) {
      rmSync(candidate, { force: true });
      throw validation.error;
    }
    if (validation.status !== 0) {
      rmSync(candidate, { force: true });
      process.stderr.write(
        `solid-checker: not promoted: the promoted document for ${contractFile} does not validate, ` +
          `so the contract and its review state are unchanged: ${
            [validation.stderr, validation.stdout].filter(Boolean).join("\n").trim() ||
            `native solid-checker exited ${validation.status}`
          }\n`
      );
      process.exitCode = 1;
      return;
    }
    renameSync(candidate, contractFile);
    contractBytes = readFileSync(contractFile);
    contractHash = sha256Bytes(contractBytes);
    for (const resolution of Object.values(state.resolutions)) {
      // The promotion rewrote the bytes every one of these was recorded
      // against, and it only ran because they were all current beforehand.
      resolution.contract = contractHash;
    }
    // Audit information only: `from` is the hash of the document as it stood
    // immediately before this promotion, which is the plan's hash only when
    // nothing was hand-edited in between. What keeps the plan recognizable
    // across the promotion is `state.plan`, which `persist` writes and no
    // mutation moves.
    state.promoted = { at: new Date().toISOString(), evidence: "reviewed", from: preHash };
    persist();

    process.stdout.write(
      `promoted ${contractFile} to reviewed evidence: ${deleted} unknown claim(s) certified absent, ` +
        `${staleSummaryMarkers} summary probed marker(s) recomputed away, ` +
        `${droppedMarkers} inferred row marker(s) dropped\n`
    );
    contract = expandedContract(JSON.parse(contractBytes.toString("utf8")));
  }

  let resolved = 0;
  let open = 0;
  let stale = 0;
  for (const item of plan.items) {
    const state_ = status(item);
    if (state_ === "resolved") resolved += 1;
    else if (state_ === "stale") stale += 1;
    else open += 1;
    process.stdout.write(`[${state_}] ${item.id} ${item.kind}: ${item.text}\n`);
  }
  const sentinels = unknownSentinels(contract).length;
  process.stdout.write(
    `${plan.items.length} review item(s) for ${contract.package.name}@${contract.package.version}: ` +
      `${resolved} resolved, ${open} open, ${stale} stale; ${sentinels} unknown claim(s) remaining; ` +
      `evidence ${contract.evidence?.kind}\n`
  );
  if (!(open || stale || sentinels)) return;
  // The gate answers one question -- does this contract certify anything yet --
  // and for a machine-verified document the answer is already yes. Its items are
  // the *upgrade* to the reviewed tier: the unknown domains a machine could not
  // confirm, the callback owner rows no machine can ever produce, and the
  // generated-summary questions that exist so no row reaches `reviewed`
  // evidence without a human naming its export. None of them block what the
  // document already claims, so exiting 1 would report a certifying contract as
  // unfinished. `--promote reviewed` still refuses on every one of them, which
  // is where the invariant actually lives.
  if (contract.evidence?.kind === "verified" && !options.promote) {
    process.stdout.write(
      `${contractFile} already certifies as verified; the item(s) above are the optional upgrade ` +
        "to the reviewed tier, where an unknown claim becomes a reviewed row and a callback owner " +
        "row becomes possible at all\n"
    );
    return;
  }
  process.exitCode = 1;
}
