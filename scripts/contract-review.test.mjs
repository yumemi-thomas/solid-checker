import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  appendFileSync,
  chmodSync,
  cpSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
  writeFileSync
} from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import test from "node:test";

import { expandContract } from "../packages/cli/scripts/contract-document.mjs";
import {
  collectReviewItems,
  renderReviewPlanDocument
} from "../packages/cli/scripts/contract-review-plan.mjs";
import { closureDifference } from "../packages/cli/scripts/review-contract.mjs";

const root = resolve(import.meta.dirname, "..");
const cli = join(root, "packages/cli/bin/solid-checker.mjs");
const native = process.env.SOLID_CHECKER_NATIVE_BIN ?? join(root, "rust/target/debug/solid-checker-rust");
const typeFacts = process.env.SOLID_TYPEFACTS_BIN ?? join(root, "bin/solid-typefacts");
const canRun = existsSync(native) && existsSync(typeFacts);

function run(args, { env } = {}) {
  return spawnSync(process.execPath, [cli, ...args], {
    cwd: root,
    env: { ...process.env, ...env },
    encoding: "utf8"
  });
}

function runNative(args) {
  return run(args, {
    env: { SOLID_CHECKER_NATIVE_BIN: native, SOLID_TYPEFACTS_BIN: typeFacts }
  });
}

/// A contract and the review plan generation would have written beside it,
/// built with the generator's own plan module.
///
/// The review command reads and resolves those two files and never runs the
/// native checker until `--promote` succeeds, so everything up to promotion is
/// exercised without a binary.
function hermeticWorkspace(summaries, exports_) {
  const directory = mkdtempSync(join(tmpdir(), "solid-checker-review-"));
  const contract = join(directory, "solid-reactivity.json");
  const document = {
    schemaVersion: 1,
    package: { name: "hermetic-package", version: "1.0.0" },
    compilerFactsProtocol: 1,
    summaries,
    entrypoints: { ".": { exports: exports_ } },
    evidence: { kind: "inferred", generator: "solid-checker package generator" }
  };
  writeFileSync(contract, `${JSON.stringify(document, null, 2)}\n`);
  const items = collectReviewItems(expandContract(document).entrypoints);
  // Exactly as generation writes it: the contract first, then the plan bound to
  // the bytes that were written. A plan carrying no `contract` is refused, so
  // the fixture cannot skip this and still exercise anything.
  writeFileSync(
    join(directory, "solid-reactivity.review.json"),
    `${JSON.stringify(
      renderReviewPlanDocument(
        "hermetic-package",
        "1.0.0",
        items,
        { generator: "solid-checker@test", entrypoints: {} },
        `sha256:${createHash("sha256").update(readFileSync(contract)).digest("hex")}`
      ),
      null,
      2
    )}\n`
  );
  const identify = kind => items.find(item => item.kind === kind).id;
  return { directory, contract, items, identify };
}

function sentinelWorkspace() {
  return hermeticWorkspace(
    {
      "function-1": { kind: "function", callbacks: { status: "unknown" } },
      function: { kind: "function" }
    },
    { "function-1": ["schedule"], function: ["plain"] }
  );
}

test("contract generate writes a one-line summary and sibling review plan", { skip: !canRun }, () => {
  const temporary = mkdtempSync(join(tmpdir(), "solid-checker-contract-review-"));
  const output = join(temporary, "solid-reactivity.json");
  try {
    const result = spawnSync(
      process.execPath,
      [
        join(root, "packages/cli/bin/solid-checker.mjs"),
        "contract",
        "generate",
        "--package-root",
        join(root, "fixtures/package-contracts/shorthand-block-scope"),
        "--output",
        output
      ],
      {
        cwd: root,
        env: {
          ...process.env,
          SOLID_CHECKER_NATIVE_BIN: native,
          SOLID_TYPEFACTS_BIN: typeFacts
        },
        encoding: "utf8"
      }
    );
    assert.equal(result.status, 0, result.stderr);
    assert.equal(result.stdout.trim().split(/\r?\n/).length, 1, result.stdout);
    assert.match(result.stdout, /review plan .*\.review\.md/);
    const review = readFileSync(join(temporary, "solid-reactivity.review.md"), "utf8");
    for (const section of [
      "## exports with no summary",
      "## unknown export claims",
      "## callbacks with no execution row",
      "## callbacks with no owner row",
      "## owner requirements requiring review",
      "## inherited rows",
      "## environment-branching exports"
    ]) {
      assert.match(review, new RegExp(section));
    }
    assert.match(review, /generated evidence is inferred/i);
  } finally {
    rmSync(temporary, { recursive: true, force: true });
  }
});

test(
  "the review plan names the legacy field a dual-root contract came from",
  { skip: !canRun },
  () => {
    // `module` and `main` name different builds and only the ESM one is
    // analyzable. The contract is still generated -- refusing would reject
    // every legacy dual package -- so the review plan is the only place the
    // reviewer learns which artifact the summaries describe.
    const temporary = mkdtempSync(join(tmpdir(), "solid-checker-legacy-review-"));
    const output = join(temporary, "solid-reactivity.json");
    try {
      const result = spawnSync(
        process.execPath,
        [
          join(root, "packages/cli/bin/solid-checker.mjs"),
          "contract",
          "generate",
          "--package-root",
          join(root, "fixtures/package-contracts/legacy-dual-root"),
          "--output",
          output
        ],
        {
          cwd: root,
          env: {
            ...process.env,
            SOLID_CHECKER_NATIVE_BIN: native,
            SOLID_TYPEFACTS_BIN: typeFacts
          },
          encoding: "utf8"
        }
      );
      assert.equal(result.status, 0, result.stderr);
      const review = readFileSync(join(temporary, "solid-reactivity.review.md"), "utf8");
      assert.match(review, /## legacy entrypoint resolution/);
      assert.match(review, /resolved from the legacy "module" field \(\.\/dist\/browser\.js\)/);
      assert.match(review, /"main" names a different runtime artifact \(\.\/dist\/node\.cjs\)/);
    } finally {
      rmSync(temporary, { recursive: true, force: true });
    }
  }
);

test(
  "the machine-readable review plan carries stable ids and the closure it was derived from",
  { skip: !canRun },
  () => {
    // Two generations of the same unchanged package. The ids are what every
    // recorded review decision is keyed by, so a regeneration that renamed
    // them would silently move each decision to another item.
    const directories = [
      mkdtempSync(join(tmpdir(), "solid-checker-plan-a-")),
      mkdtempSync(join(tmpdir(), "solid-checker-plan-b-"))
    ];
    try {
      const plans = directories.map(directory => {
        const output = join(directory, "solid-reactivity.json");
        const result = runNative([
          "contract",
          "generate",
          "--package-root",
          join(root, "fixtures/package-contracts/unknown-callback-claim"),
          "--output",
          output
        ]);
        assert.equal(result.status, 0, result.stderr);
        assert.equal(result.stdout.trim().split(/\r?\n/).length, 1, result.stdout);
        assert.match(result.stdout, /review plan .*\.review\.md/);
        assert.match(result.stdout, /\.review\.json/);
        return JSON.parse(readFileSync(join(directory, "solid-reactivity.review.json"), "utf8"));
      });
      assert.equal(plans[0].schemaVersion, 1);
      assert.deepEqual(plans[0].package, { name: "unknown-callback-claim", version: "1.0.0" });
      assert.deepEqual(
        plans[0].items.map(item => [item.id, item.kind]),
        plans[1].items.map(item => [item.id, item.kind])
      );
      const sentinel = plans[0].items.find(item => item.kind === "unknown-sentinel");
      assert.deepEqual(sentinel.target, {
        entrypoint: ".",
        export: "schedule",
        field: "callbacks"
      });
      const negative = plans[0].items.find(item => item.kind === "no-callback-row");
      assert.deepEqual(negative.target, { entrypoint: ".", export: "plain", field: "callbacks" });
      // The summaries were derived from these exact bytes; the block records
      // them because schema v1's one artifact pair cannot.
      const closure = plans[0].generation.entrypoints["."];
      assert.deepEqual(closure.targets, ["./index.ts"]);
      assert.ok(closure.modules.length >= 1);
      for (const module of closure.modules) {
        assert.match(module.hash, /^sha256:[0-9a-f]{64}$/);
      }
    } finally {
      for (const directory of directories) rmSync(directory, { recursive: true, force: true });
    }
  }
);

// Which of the two closure fields a transfer reads, pinned directly on the
// comparison the transfer runs. The end-to-end tests below drive real
// generations, and no real generation can produce the pair of records that makes
// this decision visible: two byte-identical records where one carries a runtime
// claim and the other does not is not a shape a package has.
test("a transfer reads notes and deliberately ignores runtimeNotes", () => {
  const record = () => ({
    targets: ["./index.js"],
    modules: [{ path: "index.js", hash: "sha256:aa" }]
  });

  // The baseline: same targets, same modules, same hashes.
  assert.equal(closureDifference(record(), record()), "");

  // A `runtimeNotes` entry says the record is the analyzing program's own file
  // list and complete for what the analysis read, and that nothing bounds what
  // the runtime loads. The bytes on both sides are the same bytes and the
  // runtime is exactly as unbounded in both, so refusing the transfer would
  // refuse it for a reason that did not change. Promotion still refuses -- see
  // the attested-closure blocker in scripts/contract-verify.test.mjs, which is
  // the gate that question is about.
  const unbounded = { ...record(), runtimeNotes: ["index.js: the module record is attested --"] };
  assert.equal(closureDifference(unbounded, unbounded), "");
  assert.equal(closureDifference(record(), unbounded), "");
  assert.equal(closureDifference(unbounded, record()), "");

  // A `notes` entry is the other claim: the record does not establish which
  // bytes the summaries came from, so it cannot establish that two generations
  // came from the same ones. Either side carrying one refuses.
  const unattested = { ...record(), notes: ["./index.js: closure not attested: ..."] };
  assert.match(closureDifference(unattested, record()), /^its closure record is incomplete/);
  assert.match(closureDifference(record(), unattested), /^its closure record is incomplete/);

  // And the record still has to be a record: an empty module list transfers
  // nothing, note or no note.
  assert.match(
    closureDifference({ targets: ["./index.js"], modules: [] }, record()),
    /^its closure record names no module/
  );
  assert.match(
    closureDifference(record(), { ...record(), modules: [{ path: "index.js", hash: "sha256:bb" }] }),
    /^its runtime module closure changed/
  );
});

test("listing an unresolved plan is a gate", () => {
  const { directory, contract, identify } = sentinelWorkspace();
  try {
    const listed = run(["contract", "review", contract]);
    assert.equal(listed.status, 1, listed.stdout);
    assert.match(listed.stdout, /^\[open\] .* unknown-sentinel: \.:schedule: callbacks$/m);
    assert.match(listed.stdout, /2 review item\(s\).*0 resolved, 2 open, 0 stale/);
    // The directory containing the contract names it just as well.
    assert.equal(run(["contract", "review", directory]).status, 1);
    const resolved = run([
      "contract",
      "review",
      contract,
      "--resolve",
      `${identify("no-callback-row")}=absent`
    ]);
    assert.equal(resolved.status, 1, resolved.stdout);
    assert.match(resolved.stdout, /1 resolved, 1 open, 0 stale; 1 unknown claim\(s\) remaining/);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("a plan with no unknown claim exits 0 once every item is decided", () => {
  const { directory, contract, identify } = hermeticWorkspace(
    { function: { kind: "function" } },
    { function: ["plain"] }
  );
  try {
    // The only item is the negative claim itself, and certifying it is an
    // explicit decision rather than the silence a generated contract ships.
    const answers = join(directory, "answers.json");
    writeFileSync(answers, `${JSON.stringify({ [identify("no-callback-row")]: "absent" })}\n`);
    const resolved = run(["contract", "review", contract, "--answers", answers]);
    assert.equal(resolved.status, 0, resolved.stdout + resolved.stderr);
    assert.match(resolved.stdout, /1 resolved, 0 open, 0 stale; 0 unknown claim\(s\) remaining/);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("an unknown claim cannot be confirmed, only certified absent or edited away", () => {
  const { directory, contract, identify } = sentinelWorkspace();
  try {
    const confirmed = run([
      "contract",
      "review",
      contract,
      "--resolve",
      `${identify("unknown-sentinel")}=confirm`
    ]);
    assert.equal(confirmed.status, 2, confirmed.stdout);
    assert.match(confirmed.stderr, /unknown is not evidence and cannot be confirmed/);
    const absent = run([
      "contract",
      "review",
      contract,
      "--resolve",
      `${identify("unknown-sentinel")}=absent`,
      "--note",
      "audited: schedule invokes nothing the caller supplies"
    ]);
    assert.equal(absent.status, 1, absent.stdout);
    const state = JSON.parse(
      readFileSync(join(directory, "solid-reactivity.review-state.json"), "utf8")
    );
    const recorded = state.resolutions[identify("unknown-sentinel")];
    assert.equal(recorded.decision, "absent");
    assert.match(recorded.note, /^audited:/);
    assert.match(recorded.contract, /^sha256:[0-9a-f]{64}$/);
    // An id nobody generated is a typo, not a decision.
    const unknown = run(["contract", "review", contract, "--resolve", "no-such-item=confirm"]);
    assert.equal(unknown.status, 2);
    assert.match(unknown.stderr, /is not a review item/);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("promotion is refused while anything is open or undecided", () => {
  const { directory, contract, identify } = sentinelWorkspace();
  try {
    const open = run(["contract", "review", contract, "--promote", "reviewed"]);
    assert.equal(open.status, 1, open.stdout);
    assert.match(open.stderr, /not promoted: open review item/);
    // Everything decided *except* the sentinel: the negative claim is
    // certified, the unknown one is not, and promotion still refuses.
    run(["contract", "review", contract, "--resolve", `${identify("no-callback-row")}=absent`]);
    const undecided = run([
      "contract",
      "review",
      contract,
      "--resolve",
      `${identify("unknown-sentinel")}=confirm`
    ]);
    assert.equal(undecided.status, 2, undecided.stdout);
    const refused = run(["contract", "review", contract, "--promote", "reviewed"]);
    assert.equal(refused.status, 1, refused.stdout);
    assert.match(refused.stderr, /unknown claim .*remains in the contract/);
    // Parsed, not matched: every generated row carries its own
    // `{"kind": "inferred"}` marker, so the regular expression passed on a
    // document whose *evidence* had already been promoted.
    assert.equal(
      JSON.parse(readFileSync(contract, "utf8")).evidence.kind,
      "inferred",
      "a refused promotion must not touch the contract"
    );
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("promotion never writes verified, trusted, or attested", () => {
  const { directory, contract } = sentinelWorkspace();
  try {
    // `verified` exists and is reachable -- it just is not a decision, so it is
    // `solid-checker contract verify`'s and not this command's. The refusal
    // names the command rather than only saying no.
    const verified = run(["contract", "review", contract, "--promote", "verified"]);
    assert.equal(verified.status, 2, verified.stdout);
    assert.match(verified.stderr, /Run `solid-checker contract verify <contract>` instead/);
    for (const kind of ["trusted", "attested"]) {
      const refused = run(["contract", "review", contract, "--promote", kind]);
      assert.equal(refused.status, 2, refused.stdout);
      assert.match(refused.stderr, /promotes only to reviewed/);
    }
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("a resolution recorded against different contract bytes is stale, not resolved", () => {
  const { directory, contract, identify } = sentinelWorkspace();
  try {
    run(["contract", "review", contract, "--resolve", `${identify("no-callback-row")}=absent`]);
    const document = JSON.parse(readFileSync(contract, "utf8"));
    document.summaries.function.reactiveReads = [{ kind: "accessor", parameter: 0 }];
    writeFileSync(contract, `${JSON.stringify(document, null, 2)}\n`);
    const listed = run(["contract", "review", contract]);
    assert.equal(listed.status, 1, listed.stdout);
    assert.match(listed.stdout, /^\[stale\] .* no-callback-row:/m);
    const refused = run(["contract", "review", contract, "--promote", "reviewed"]);
    assert.equal(refused.status, 1, refused.stdout);
    assert.match(refused.stderr, /stale resolution for .*recorded against different contract bytes/);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test(
  "promotion certifies the negative, sets reviewed evidence, and validates",
  { skip: !canRun },
  () => {
    const temporary = mkdtempSync(join(tmpdir(), "solid-checker-promote-"));
    const contract = join(temporary, "solid-reactivity.json");
    try {
      const generated = runNative([
        "contract",
        "generate",
        "--package-root",
        join(root, "fixtures/package-contracts/unknown-callback-claim"),
        "--output",
        contract
      ]);
      assert.equal(generated.status, 0, generated.stderr);
      const plan = JSON.parse(readFileSync(join(temporary, "solid-reactivity.review.json"), "utf8"));
      const resolved = runNative([
        "contract",
        "review",
        contract,
        ...plan.items.flatMap(item => [
          "--resolve",
          `${item.id}=${item.kind === "unknown-sentinel" || item.kind === "no-callback-row" ? "absent" : "confirm"}`
        ])
      ]);
      assert.equal(resolved.status, 1, resolved.stdout);

      const promoted = runNative(["contract", "review", contract, "--promote", "reviewed"]);
      assert.equal(promoted.status, 0, promoted.stdout + promoted.stderr);
      assert.match(promoted.stdout, /1 unknown claim\(s\) certified absent/);
      const document = JSON.parse(readFileSync(contract, "utf8"));
      assert.equal(document.evidence.kind, "reviewed");
      assert.deepEqual(
        Object.keys(document.entrypoints),
        ["."],
        "promotion may delete a sentinel and change evidence, nothing else"
      );
      assert.deepEqual(
        Object.values(document.entrypoints["."].exports).flat().sort(),
        ["plain", "schedule"]
      );
      for (const summary of Object.values(document.summaries)) {
        assert.equal(summary.callbacks, undefined, "the sentinel field is deleted, not kept");
        // An inferred row inside a promoted contract is refused by
        // certification, so promotion drops the markers it resolved rather
        // than leaving a document that cannot certify.
        assert.equal(summary.evidence, undefined);
      }
      const validated = spawnSync(native, ["--validate-contract", contract], {
        env: { ...process.env, SOLID_TYPEFACTS_BIN: typeFacts },
        encoding: "utf8"
      });
      assert.equal(validated.status, 0, validated.stderr);
      const state = JSON.parse(
        readFileSync(join(temporary, "solid-reactivity.review-state.json"), "utf8")
      );
      assert.equal(state.promoted.evidence, "reviewed");
      // The promotion rewrote the bytes every resolution was recorded against.
      for (const resolution of Object.values(state.resolutions)) {
        assert.equal(resolution.contract, state.contract);
      }
      const again = runNative(["contract", "review", contract]);
      assert.equal(again.status, 0, again.stdout);
      assert.match(again.stdout, /0 open, 0 stale; 0 unknown claim\(s\) remaining; evidence reviewed/);
    } finally {
      rmSync(temporary, { recursive: true, force: true });
    }
  }
);

test(
  "resolved-by-edit is accepted only once the contract no longer raises the item",
  { skip: !canRun },
  () => {
    const temporary = mkdtempSync(join(tmpdir(), "solid-checker-edit-"));
    const contract = join(temporary, "solid-reactivity.json");
    try {
      assert.equal(
        runNative([
          "contract",
          "generate",
          "--package-root",
          join(root, "fixtures/package-contracts/unknown-callback-claim"),
          "--output",
          contract
        ]).status,
        0
      );
      const plan = JSON.parse(readFileSync(join(temporary, "solid-reactivity.review.json"), "utf8"));
      const sentinel = plan.items.find(item => item.kind === "unknown-sentinel");
      const premature = run([
        "contract",
        "review",
        contract,
        "--resolve",
        `${sentinel.id}=resolved-by-edit`
      ]);
      assert.equal(premature.status, 2, premature.stdout);
      assert.match(premature.stderr, /is still raised by the contract/);

      // One decision recorded before the hand edit, which is what a review
      // looks like: resolve what the generated document already answers, then
      // edit what it could not. It is also what keeps the plan usable across
      // the edit -- the review state is the only thing that can say the plan
      // was written for these bytes once the edit has moved them.
      const negative = plan.items.find(item => item.kind === "no-callback-row");
      assert.equal(
        run(["contract", "review", contract, "--resolve", `${negative.id}=absent`]).status,
        1
      );

      const document = JSON.parse(readFileSync(contract, "utf8"));
      for (const summary of Object.values(document.summaries)) {
        if (summary.callbacks?.status !== "unknown") continue;
        summary.callbacks = [{ parameter: 0, execution: "deferred", owner: "inherited" }];
      }
      writeFileSync(contract, `${JSON.stringify(document, null, 2)}\n`);
      const accepted = run([
        "contract",
        "review",
        contract,
        "--resolve",
        `${sentinel.id}=resolved-by-edit`
      ]);
      assert.equal(accepted.status, 1, accepted.stdout);
      assert.match(accepted.stdout, /0 unknown claim\(s\) remaining/);
    } finally {
      rmSync(temporary, { recursive: true, force: true });
    }
  }
);

/// One release of a package, generated with the contract *inside* the package
/// root -- the published-contract tier.
///
/// Each release is its own package root with its own contract beside it, so the
/// closure record's contract-directory-relative paths line up across releases
/// without either generation overwriting the other. That is what makes this
/// layout convenient here and unlike a real upgrade, where the contract is
/// regenerated in place; `projectWorkspace` below covers that path, and the
/// project-owned tier it belongs to.
function republish(directory, release, edit) {
  const packageRoot = join(directory, release);
  mkdirSync(packageRoot, { recursive: true });
  cpSync(join(root, "fixtures/package-contracts/unknown-callback-claim"), packageRoot, {
    recursive: true
  });
  const manifestPath = join(packageRoot, "package.json");
  const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
  edit?.(manifest, packageRoot);
  writeFileSync(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`);
  const contract = join(packageRoot, "solid-reactivity.json");
  const generated = runNative([
    "contract",
    "generate",
    "--package-root",
    packageRoot,
    "--output",
    contract
  ]);
  assert.equal(generated.status, 0, generated.stderr);
  const plan = JSON.parse(readFileSync(join(packageRoot, "solid-reactivity.review.json"), "utf8"));
  return { packageRoot, contract, plan, state: join(packageRoot, "solid-reactivity.review-state.json") };
}

function reviewEveryItem(release) {
  const answers = join(release.packageRoot, "answers.json");
  writeFileSync(
    answers,
    `${JSON.stringify(
      Object.fromEntries(
        release.plan.items.map(item => [
          item.id,
          item.kind === "unknown-sentinel" || item.kind === "no-callback-row"
            ? "absent"
            : "confirm"
        ])
      )
    )}\n`
  );
  assert.equal(runNative(["contract", "review", release.contract, "--answers", answers]).status, 1);
  const promoted = runNative(["contract", "review", release.contract, "--promote", "reviewed"]);
  assert.equal(promoted.status, 0, promoted.stdout + promoted.stderr);
}

test(
  "a version bump with unchanged bytes transfers every resolution and promotes with no new decision",
  { skip: !canRun },
  () => {
    const directory = mkdtempSync(join(tmpdir(), "solid-checker-transfer-"));
    try {
      const before = republish(directory, "1.0.0");
      reviewEveryItem(before);
      const after = republish(directory, "1.1.0", manifest => {
        manifest.version = "1.1.0";
      });
      assert.deepEqual(
        after.plan.items.map(item => item.id),
        before.plan.items.map(item => item.id),
        "stable ids are what a transfer is keyed by"
      );
      const readBytes = {
        oldContract: readFileSync(before.contract),
        oldState: readFileSync(before.state),
        newContract: readFileSync(after.contract)
      };

      const transferred = runNative([
        "contract",
        "review",
        after.contract,
        "--transfer-from",
        before.contract
      ]);
      // Exit 1 until promotion: every item is resolved, and the sentinel the
      // review certified absent is still in the contract's bytes.
      assert.equal(transferred.status, 1, transferred.stdout + transferred.stderr);
      assert.match(transferred.stdout, /transferred 2 of 2 review item\(s\).*0 remain open/);
      assert.match(transferred.stdout, /2 resolved, 0 open, 0 stale/);

      // Provenance, so the audit trail can say these conclusions were reached
      // about other bytes and why they apply to these.
      const state = JSON.parse(readFileSync(after.state, "utf8"));
      const previous = JSON.parse(readFileSync(before.state, "utf8"));
      for (const [id, resolution] of Object.entries(state.resolutions)) {
        assert.equal(resolution.decision, previous.resolutions[id].decision);
        assert.equal(resolution.contract, state.contract);
        assert.match(resolution.transferred.from, /^sha256:[0-9a-f]{64}$/);
        assert.equal(resolution.transferred.at, previous.resolutions[id].at);
      }
      assert.notEqual(state.resolutions[after.plan.items[0].id].transferred.from, state.contract);

      // A transfer writes exactly one file. Comparing a version string only
      // proved the old contract was not *regenerated*; these compare bytes,
      // which is what "never touches" has to mean for the document a review
      // was recorded against.
      assert.deepEqual(readFileSync(before.contract), readBytes.oldContract);
      assert.deepEqual(readFileSync(before.state), readBytes.oldState);
      assert.deepEqual(readFileSync(after.contract), readBytes.newContract);

      // Running the transfer again records the same decisions against the same
      // bytes; a second run must not rewrite the trail it already wrote.
      const bytes = readFileSync(after.state, "utf8");
      const again = runNative([
        "contract",
        "review",
        after.contract,
        "--transfer-from",
        before.contract
      ]);
      assert.equal(again.status, 1, again.stdout + again.stderr);
      assert.equal(readFileSync(after.state, "utf8"), bytes);

      // The fast path: zero new human decisions between a republish and a
      // promoted contract.
      const promoted = runNative(["contract", "review", after.contract, "--promote", "reviewed"]);
      assert.equal(promoted.status, 0, promoted.stdout + promoted.stderr);
      assert.match(promoted.stdout, /1 unknown claim\(s\) certified absent/);
      const document = JSON.parse(readFileSync(after.contract, "utf8"));
      assert.equal(document.evidence.kind, "reviewed");
      assert.equal(document.package.version, "1.1.0");
      for (const summary of Object.values(document.summaries)) {
        assert.equal(summary.callbacks, undefined);
      }
      assert.equal(
        JSON.parse(readFileSync(before.contract, "utf8")).package.version,
        "1.0.0",
        "a transfer never touches the contract it read"
      );
    } finally {
      rmSync(directory, { recursive: true, force: true });
    }
  }
);

test(
  "a changed runtime module keeps its entrypoint open and promotion still refuses",
  { skip: !canRun },
  () => {
    const directory = mkdtempSync(join(tmpdir(), "solid-checker-transfer-diff-"));
    try {
      const before = republish(directory, "1.0.0");
      reviewEveryItem(before);
      // One byte of implementation, under a version bump: the summaries the new
      // contract carries were derived from bytes nobody reviewed.
      const after = republish(directory, "2.0.0", (manifest, packageRoot) => {
        manifest.version = "2.0.0";
        appendFileSync(join(packageRoot, "index.ts"), "\n// republished\n");
      });
      const transferred = runNative([
        "contract",
        "review",
        after.contract,
        "--transfer-from",
        before.contract
      ]);
      assert.equal(transferred.status, 1, transferred.stdout);
      assert.match(
        transferred.stdout,
        /^open \.: 2 item\(s\) not transferable: its runtime module closure changed$/m
      );
      assert.match(transferred.stdout, /transferred 0 of 2 review item\(s\).*2 remain open/);
      assert.match(transferred.stdout, /0 resolved, 2 open, 0 stale/);
      const refused = runNative(["contract", "review", after.contract, "--promote", "reviewed"]);
      assert.equal(refused.status, 1, refused.stdout);
      assert.match(refused.stderr, /not promoted: open review item/);
      assert.equal(
        JSON.parse(readFileSync(after.contract, "utf8")).evidence.kind,
        "inferred",
        "nothing was reviewed, so nothing is promoted"
      );
    } finally {
      rmSync(directory, { recursive: true, force: true });
    }
  }
);

test(
  "transfer refuses an unreviewed source, another package, and a review already under way",
  { skip: !canRun },
  () => {
    const directory = mkdtempSync(join(tmpdir(), "solid-checker-transfer-refusal-"));
    try {
      const before = republish(directory, "1.0.0");
      const after = republish(directory, "1.1.0", manifest => {
        manifest.version = "1.1.0";
      });

      // No recorded review is no reviewed conclusion; a generated contract's
      // inferred claims are not something to carry anywhere.
      const unreviewed = runNative([
        "contract",
        "review",
        after.contract,
        "--transfer-from",
        before.contract
      ]);
      assert.equal(unreviewed.status, 2, unreviewed.stdout);
      assert.match(unreviewed.stderr, /records no review, so it has no reviewed conclusion/);

      reviewEveryItem(before);
      const other = republish(directory, "other", manifest => {
        manifest.name = "other-package";
      });
      const mismatched = runNative([
        "contract",
        "review",
        other.contract,
        "--transfer-from",
        before.contract
      ]);
      assert.equal(mismatched.status, 2, mismatched.stdout);
      assert.match(mismatched.stderr, /a review transfers within one package, across its versions/);

      // Transfer is the first step of a re-review: merged into decisions
      // already taken, the state could no longer say which is which.
      const sentinel = after.plan.items.find(item => item.kind === "unknown-sentinel");
      assert.equal(
        runNative(["contract", "review", after.contract, "--resolve", `${sentinel.id}=absent`])
          .status,
        1
      );
      const stateBytes = readFileSync(after.state, "utf8");
      const merged = runNative([
        "contract",
        "review",
        after.contract,
        "--transfer-from",
        before.contract
      ]);
      assert.equal(merged.status, 2, merged.stdout);
      assert.match(
        merged.stderr,
        /already records 1 resolution\(s\) against other bytes than the ones being transferred onto/
      );
      assert.equal(
        readFileSync(after.state, "utf8"),
        stateBytes,
        "a refused transfer leaves the review state byte-identical"
      );
    } finally {
      rmSync(directory, { recursive: true, force: true });
    }
  }
);

/// A project laid out the way a real one is: the package under `node_modules`,
/// the contract in the project's own `.solid-checker/contracts/` tree.
///
/// This is the tier `--transfer-from` exists for. It is also the only tier that
/// carries an `artifact-binding` item at all -- a project-owned contract sits
/// outside the package by construction, so schema v1 can never bind it -- which
/// makes it the tier where "artifact-binding never transfers" silently disabled
/// the whole version-bump fast path. Regeneration happens in place, so the
/// previous reviewed triple survives only through the generator's own snapshot.
function projectWorkspace(name = "barrel-package") {
  const directory = mkdtempSync(join(tmpdir(), "solid-checker-project-"));
  const packageRoot = join(directory, "node_modules", name);
  mkdirSync(packageRoot, { recursive: true });
  const contract = join(
    directory,
    ".solid-checker",
    "contracts",
    name,
    "solid-reactivity.json"
  );
  const sibling = suffix => contract.replace(/\.json$/, suffix);
  return {
    name,
    directory,
    packageRoot,
    contract,
    previous: sibling(".previous.json"),
    state: sibling(".review-state.json"),
    planPath: sibling(".review.json"),
    plan: () => JSON.parse(readFileSync(sibling(".review.json"), "utf8"))
  };
}

/// One publish of that package: a barrel entry whose semantics live in a
/// TypeScript sibling the entry names with a `.js` specifier -- the exact shape
/// whose closure the old walker recorded as the entry file alone.
function publish(workspace, { version, implementation }) {
  writeFileSync(
    join(workspace.packageRoot, "package.json"),
    `${JSON.stringify(
      { name: workspace.name, version, type: "module", exports: "./index.js" },
      null,
      2
    )}\n`
  );
  writeFileSync(
    join(workspace.packageRoot, "index.js"),
    'export { thing } from "./impl.js";\n'
  );
  writeFileSync(join(workspace.packageRoot, "impl.ts"), implementation);
  const generated = runNative([
    "contract",
    "generate",
    "--package-root",
    workspace.packageRoot,
    "--output",
    workspace.contract
  ]);
  assert.equal(generated.status, 0, generated.stderr);
  return generated;
}

function decisionFor(kind) {
  return kind === "unknown-sentinel" || kind === "no-callback-row" ? "absent" : "confirm";
}

function resolveEveryItem(workspace) {
  const answers = join(workspace.directory, "answers.json");
  writeFileSync(
    answers,
    `${JSON.stringify(
      Object.fromEntries(workspace.plan().items.map(item => [item.id, decisionFor(item.kind)]))
    )}\n`
  );
  return runNative(["contract", "review", workspace.contract, "--answers", answers]);
}

test(
  "the documented upgrade loop runs end to end on a project-owned contract",
  { skip: !canRun },
  () => {
    const workspace = projectWorkspace();
    try {
      publish(workspace, { version: "1.0.0", implementation: "export function thing(callback) {\n  callback();\n  return 1;\n}\n" });
      // The closure the summaries came from includes the TypeScript sibling the
      // `.js` specifier resolves to, which is the whole point: bind the review
      // to bytes that exclude it and rewriting it transfers a review nobody did.
      assert.deepEqual(
        workspace.plan().generation.entrypoints["."].modules.map(module => module.path),
        ["../../../node_modules/barrel-package/impl.ts", "../../../node_modules/barrel-package/index.js"]
      );
      // Every export is named by an item, so a promotion cannot rest on nothing.
      assert.ok(
        workspace.plan().items.some(
          item => item.kind === "generated-summary" && item.text.startsWith(".:thing ")
        )
      );
      assert.ok(workspace.plan().items.some(item => item.kind === "artifact-binding"));

      // Exit 0 or 1 depending on whether an unknown claim is left in the
      // bytes; 2 would mean a decision was refused.
      assert.notEqual(resolveEveryItem(workspace).status, 2);
      const promoted = runNative(["contract", "review", workspace.contract, "--promote", "reviewed"]);
      assert.equal(promoted.status, 0, promoted.stdout + promoted.stderr);

      // A metadata-only republish, regenerated in place. The generator keeps
      // the reviewed triple as `.previous` and prints the transfer command,
      // which is the step that used to be impossible: regenerating destroyed
      // the contract and plan `--transfer-from` needs to read.
      const regenerated = publish(workspace, {
        version: "1.1.0",
        implementation: "export function thing(callback) {\n  callback();\n  return 1;\n}\n"
      });
      assert.match(regenerated.stdout, /--transfer-from .*solid-reactivity\.previous\.json/);
      assert.equal(existsSync(workspace.previous), true);
      assert.equal(existsSync(workspace.previous.replace(/\.json$/, ".review-state.json")), true);
      assert.equal(existsSync(workspace.state), false, "the fresh triple carries no review state");

      const transferred = runNative([
        "contract",
        "review",
        workspace.contract,
        "--transfer-from",
        workspace.previous
      ]);
      assert.notEqual(transferred.status, 2, transferred.stdout + transferred.stderr);
      assert.match(transferred.stdout, /0 remain open/);
      // Including the artifact-binding item, whose transfer is what re-enables
      // the fast path for the project-owned tier.
      assert.match(transferred.stdout, /transferred artifact-binding-[0-9a-f]+ confirm/);

      const again = runNative(["contract", "review", workspace.contract, "--promote", "reviewed"]);
      assert.equal(again.status, 0, again.stdout + again.stderr);
      const document = JSON.parse(readFileSync(workspace.contract, "utf8"));
      assert.equal(document.evidence.kind, "reviewed");
      assert.equal(document.package.version, "1.1.0");
    } finally {
      rmSync(workspace.directory, { recursive: true, force: true });
    }
  }
);

test(
  "a rewritten barrel implementation transfers nothing even though the entry is byte-identical",
  { skip: !canRun },
  () => {
    // The attack the closure walker's hole made possible: the barrel entry --
    // the only file the contract's hash could ever cover -- is unchanged, while
    // every summary comes from an `impl.ts` that was rewritten. Recording the
    // entry alone made the closure hashes equal, so the entire review
    // transferred and the contract promoted to `reviewed` on zero decisions.
    const workspace = projectWorkspace();
    try {
      publish(workspace, { version: "1.0.0", implementation: "export function thing(callback) {\n  return 1;\n}\n" });
      assert.notEqual(resolveEveryItem(workspace).status, 2);
      assert.equal(
        runNative(["contract", "review", workspace.contract, "--promote", "reviewed"]).status,
        0
      );
      const entryBefore = readFileSync(join(workspace.packageRoot, "index.js"), "utf8");

      publish(workspace, {
        version: "2.0.0",
        implementation: "export function thing(callback) {\n  callback();\n  return 2;\n}\n"
      });
      assert.equal(
        readFileSync(join(workspace.packageRoot, "index.js"), "utf8"),
        entryBefore,
        "the entry artifact is byte-identical; only the module behind it changed"
      );

      const transferred = runNative([
        "contract",
        "review",
        workspace.contract,
        "--transfer-from",
        workspace.previous
      ]);
      assert.equal(transferred.status, 1, transferred.stdout);
      assert.match(transferred.stdout, /transferred 0 of \d+ review item\(s\)/);
      assert.match(transferred.stdout, /its runtime module closure changed/);

      const refused = runNative(["contract", "review", workspace.contract, "--promote", "reviewed"]);
      assert.equal(refused.status, 1, refused.stdout);
      assert.match(refused.stderr, /not promoted: open review item/);
      assert.equal(
        JSON.parse(readFileSync(workspace.contract, "utf8")).evidence.kind,
        "inferred",
        "nothing was reviewed, so nothing is promoted"
      );
    } finally {
      rmSync(workspace.directory, { recursive: true, force: true });
    }
  }
);

test("a values-only package cannot promote without a decision", { skip: !canRun }, () => {
  // A contract of plain values raises none of the exception-driven sections, so
  // its plan used to be empty -- and an empty plan promotes: every item
  // resolved, zero of them, `resolutions: {}` on disk.
  const directory = mkdtempSync(join(tmpdir(), "solid-checker-values-"));
  const packageRoot = join(directory, "package");
  mkdirSync(packageRoot, { recursive: true });
  writeFileSync(
    join(packageRoot, "package.json"),
    `${JSON.stringify(
      { name: "values-only", version: "1.0.0", type: "module", exports: "./index.js" },
      null,
      2
    )}\n`
  );
  writeFileSync(join(packageRoot, "index.js"), 'export const a = 1;\nexport const b = "two";\n');
  const contract = join(packageRoot, "solid-reactivity.json");
  try {
    assert.equal(
      runNative(["contract", "generate", "--package-root", packageRoot, "--output", contract])
        .status,
      0
    );
    const plan = JSON.parse(readFileSync(join(packageRoot, "solid-reactivity.review.json"), "utf8"));
    assert.deepEqual(
      plan.items.map(item => item.kind),
      ["generated-summary", "generated-summary"]
    );
    for (const item of plan.items) {
      assert.match(item.text, /are certified negative claims$/);
    }
    const refused = runNative(["contract", "review", contract, "--promote", "reviewed"]);
    assert.equal(refused.status, 1, refused.stdout);
    assert.match(refused.stderr, /not promoted: open review item .* generated-summary/);
    assert.equal(existsSync(join(packageRoot, "solid-reactivity.review-state.json")), false);

    // `confirm` is the only decision it takes: the item is raised for as long
    // as the export exists, so there is no negative to certify.
    const wrong = runNative([
      "contract",
      "review",
      contract,
      "--resolve",
      `${plan.items[0].id}=absent`
    ]);
    assert.equal(wrong.status, 2, wrong.stdout);
    assert.match(wrong.stderr, /absent does not apply to a generated-summary item/);

    const answers = join(directory, "answers.json");
    writeFileSync(
      answers,
      `${JSON.stringify(Object.fromEntries(plan.items.map(item => [item.id, "confirm"])))}\n`
    );
    assert.equal(runNative(["contract", "review", contract, "--answers", answers]).status, 0);
    const promoted = runNative(["contract", "review", contract, "--promote", "reviewed"]);
    assert.equal(promoted.status, 0, promoted.stdout + promoted.stderr);
    const state = JSON.parse(
      readFileSync(join(packageRoot, "solid-reactivity.review-state.json"), "utf8")
    );
    assert.equal(Object.keys(state.resolutions).length, 2);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("a review plan is refused beside a contract it was not written for", { skip: !canRun }, () => {
  const directory = mkdtempSync(join(tmpdir(), "solid-checker-plan-swap-"));
  try {
    const releases = ["1.0.0", "2.0.0"].map((version, index) => {
      const packageRoot = join(directory, version);
      mkdirSync(packageRoot, { recursive: true });
      writeFileSync(
        join(packageRoot, "package.json"),
        `${JSON.stringify(
          { name: "swap-package", version, type: "module", exports: "./index.js" },
          null,
          2
        )}\n`
      );
      writeFileSync(
        join(packageRoot, "index.js"),
        index === 0
          ? "export function f(callback) {\n  return 1;\n}\n"
          : "export function f(callback) {\n  callback();\n  return 1;\n}\n"
      );
      const contract = join(packageRoot, "solid-reactivity.json");
      assert.equal(
        runNative(["contract", "generate", "--package-root", packageRoot, "--output", contract])
          .status,
        0
      );
      return { packageRoot, contract, plan: join(packageRoot, "solid-reactivity.review.json") };
    });

    // Same package, same schema, different document. Validating the pairing on
    // package name alone accepted this and then resolved 2.0.0 by answering
    // questions asked about 1.0.0.
    cpSync(releases[0].plan, releases[1].plan);
    const swapped = runNative(["contract", "review", releases[1].contract]);
    assert.equal(swapped.status, 2, swapped.stdout);
    assert.match(swapped.stderr, /was written for contract bytes sha256:[0-9a-f]{64} and/);
    assert.match(swapped.stderr, /regenerate the contract to write a matching review plan/);
    assert.equal(
      existsSync(join(releases[1].packageRoot, "solid-reactivity.review-state.json")),
      false
    );
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("promoting an already promoted contract writes nothing", { skip: !canRun }, () => {
  const directory = mkdtempSync(join(tmpdir(), "solid-checker-repromote-"));
  try {
    const release = republish(directory, "1.0.0");
    reviewEveryItem(release);
    const contractBytes = readFileSync(release.contract);
    const stateBytes = readFileSync(release.state);

    const again = runNative(["contract", "review", release.contract, "--promote", "reviewed"]);
    // It used to refuse: promotion deletes the sentinels it certified absent,
    // which turns those exports into ones with no callback row -- a question
    // the plan, written before the deletion, does not list.
    assert.equal(again.status, 0, again.stdout + again.stderr);
    assert.match(again.stdout, /already promoted .* to reviewed evidence at /);
    assert.deepEqual(readFileSync(release.contract), contractBytes);
    assert.deepEqual(readFileSync(release.state), stateBytes);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test(
  "a promoted document the loader rejects leaves the contract and state untouched",
  { skip: !canRun },
  () => {
    const directory = mkdtempSync(join(tmpdir(), "solid-checker-promote-invalid-"));
    try {
      const release = republish(directory, "1.0.0");
      const answers = join(release.packageRoot, "answers.json");
      writeFileSync(
        answers,
        `${JSON.stringify(
          Object.fromEntries(
            release.plan.items.map(item => [item.id, decisionFor(item.kind)])
          )
        )}\n`
      );
      assert.equal(runNative(["contract", "review", release.contract, "--answers", answers]).status, 1);
      const contractBytes = readFileSync(release.contract);
      const stateBytes = readFileSync(release.state);

      // A native that refuses every `--validate-contract`. Writing the document
      // first and validating afterwards left `evidence: reviewed` and a
      // `promoted` state on disk for a document the loader rejects, and the
      // next listing then reported that as a completed review and exited 0.
      const rejecting = join(directory, "rejecting-native.mjs");
      writeFileSync(
        rejecting,
        "#!/usr/bin/env node\n" +
          'if (process.argv.includes("--validate-contract")) {\n' +
          '  process.stderr.write("solid-checker-rust: contract rejected by the loader\\n");\n' +
          "  process.exit(1);\n" +
          "}\nprocess.exit(0);\n"
      );
      chmodSync(rejecting, 0o755);
      const refused = run(["contract", "review", release.contract, "--promote", "reviewed"], {
        env: { SOLID_CHECKER_NATIVE_BIN: rejecting, SOLID_TYPEFACTS_BIN: typeFacts }
      });
      assert.equal(refused.status, 1, refused.stdout);
      assert.match(refused.stderr, /not promoted: the promoted document .* does not validate/);
      assert.deepEqual(readFileSync(release.contract), contractBytes);
      assert.deepEqual(readFileSync(release.state), stateBytes);
      assert.equal(
        JSON.parse(contractBytes.toString("utf8")).evidence.kind,
        "inferred",
        "a refused promotion never writes reviewed evidence"
      );
      // No temporary file survives the refusal.
      assert.deepEqual(
        readdirSync(release.packageRoot).filter(entry => entry.includes(".tmp-")),
        []
      );
    } finally {
      rmSync(directory, { recursive: true, force: true });
    }
  }
);

test("transfer onto a promoted review is refused", { skip: !canRun }, () => {
  const directory = mkdtempSync(join(tmpdir(), "solid-checker-transfer-promoted-"));
  try {
    const before = republish(directory, "1.0.0");
    reviewEveryItem(before);
    const after = republish(directory, "1.1.0", manifest => {
      manifest.version = "1.1.0";
    });
    assert.equal(
      runNative(["contract", "review", after.contract, "--transfer-from", before.contract]).status,
      1
    );
    assert.equal(
      runNative(["contract", "review", after.contract, "--promote", "reviewed"]).status,
      0
    );
    const stateBytes = readFileSync(after.state);
    const contractBytes = readFileSync(after.contract);

    // It used to succeed silently and delete `state.promoted`, so a promoted
    // contract quietly stopped recording that it had been promoted.
    const refused = runNative([
      "contract",
      "review",
      after.contract,
      "--transfer-from",
      before.contract
    ]);
    assert.equal(refused.status, 2, refused.stdout);
    assert.match(refused.stderr, /already promoted .* there is nothing to transfer/);
    assert.deepEqual(readFileSync(after.state), stateBytes);
    assert.deepEqual(readFileSync(after.contract), contractBytes);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("a transfer carries the note and never touches what it read", { skip: !canRun }, () => {
  const directory = mkdtempSync(join(tmpdir(), "solid-checker-transfer-note-"));
  try {
    const before = republish(directory, "1.0.0");
    const sentinel = before.plan.items.find(item => item.kind === "unknown-sentinel");
    const negative = before.plan.items.find(item => item.kind === "no-callback-row");
    assert.equal(
      runNative([
        "contract",
        "review",
        before.contract,
        "--resolve",
        `${sentinel.id}=absent`,
        "--note",
        "audited against the 1.0.0 tarball"
      ]).status,
      1
    );
    assert.equal(
      runNative(["contract", "review", before.contract, "--resolve", `${negative.id}=absent`])
        .status,
      1
    );
    assert.equal(
      runNative(["contract", "review", before.contract, "--promote", "reviewed"]).status,
      0
    );

    const after = republish(directory, "1.1.0", manifest => {
      manifest.version = "1.1.0";
    });
    const oldContractBytes = readFileSync(before.contract);
    const oldStateBytes = readFileSync(before.state);
    const newContractBytes = readFileSync(after.contract);

    const transferred = runNative([
      "contract",
      "review",
      after.contract,
      "--transfer-from",
      before.contract
    ]);
    assert.equal(transferred.status, 1, transferred.stdout + transferred.stderr);
    const state = JSON.parse(readFileSync(after.state, "utf8"));
    assert.equal(state.resolutions[sentinel.id].note, "audited against the 1.0.0 tarball");
    // The state carries what the reviewer saw, so the next transfer can compare
    // the questions and not only their ids.
    assert.equal(state.resolutions[sentinel.id].text, sentinel.text);

    assert.deepEqual(readFileSync(before.contract), oldContractBytes);
    assert.deepEqual(readFileSync(before.state), oldStateBytes);
    assert.deepEqual(readFileSync(after.contract), newContractBytes);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test(
  "a hand-edited conclusion does not transfer onto a contract that raises it again",
  { skip: !canRun },
  () => {
    // The `resolved-by-edit` acceptance check runs against the *new* bytes, but
    // it is a backstop rather than the condition that fires here: an edit that
    // answers an item necessarily changes that entrypoint's summaries, so the
    // projection comparison rejects the entrypoint first. Both refusals mean
    // the same thing -- the edit lived in the old document and nobody made it
    // in this one -- and this pins the outcome rather than which check reached
    // it.
    const directory = mkdtempSync(join(tmpdir(), "solid-checker-transfer-edit-"));
    try {
      const before = republish(directory, "1.0.0");
      const sentinel = before.plan.items.find(item => item.kind === "unknown-sentinel");
      const negative = before.plan.items.find(item => item.kind === "no-callback-row");
      // One decision first, then the hand edit: the review state is what says
      // the plan was written for these bytes once the edit has moved them.
      assert.equal(
        runNative(["contract", "review", before.contract, "--resolve", `${negative.id}=absent`])
          .status,
        1
      );
      const edited = JSON.parse(readFileSync(before.contract, "utf8"));
      for (const summary of Object.values(edited.summaries)) {
        if (summary.callbacks?.status !== "unknown") continue;
        summary.callbacks = [{ parameter: 0, execution: "deferred", owner: "inherited" }];
      }
      writeFileSync(before.contract, `${JSON.stringify(edited, null, 2)}\n`);
      assert.equal(
        runNative([
          "contract",
          "review",
          before.contract,
          "--resolve",
          `${sentinel.id}=resolved-by-edit`
        ]).status,
        1
      );

      // The regenerated contract carries the generated sentinel again, because
      // the edit lived in the old document and nobody made it here.
      const after = republish(directory, "1.1.0", manifest => {
        manifest.version = "1.1.0";
      });
      const transferred = runNative([
        "contract",
        "review",
        after.contract,
        "--transfer-from",
        before.contract
      ]);
      assert.equal(transferred.status, 1, transferred.stdout + transferred.stderr);
      assert.match(transferred.stdout, /transferred 0 of 2 review item\(s\)/);
      const state = JSON.parse(readFileSync(after.state, "utf8"));
      assert.equal(state.resolutions[sentinel.id], undefined);
      const refused = runNative(["contract", "review", after.contract, "--promote", "reviewed"]);
      assert.equal(refused.status, 1, refused.stdout);
      assert.match(refused.stderr, /not promoted: open review item/);
    } finally {
      rmSync(directory, { recursive: true, force: true });
    }
  }
);

test(
  "a legacy root that newly diverges main from module does not inherit its confirmation",
  { skip: !canRun },
  () => {
    // `module` is byte-identical across the two releases, so every closure hash
    // matches and the entrypoint transfers. What changed is the *question*: the
    // 1.0.0 item said the root came from `module`, and the 2.0.0 item says
    // `main` now names a different runtime artifact whose behavior nobody
    // compared. Only the item text carries that difference.
    const directory = mkdtempSync(join(tmpdir(), "solid-checker-legacy-transfer-"));
    const packageRoot = join(directory, "package");
    const distribution = join(packageRoot, "dist");
    const contract = join(packageRoot, "solid-reactivity.json");
    mkdirSync(distribution, { recursive: true });
    try {
      const manifest = extra =>
        writeFileSync(
          join(packageRoot, "package.json"),
          `${JSON.stringify(
            { name: "legacy-package", type: "module", module: "./dist/browser.js", ...extra },
            null,
            2
          )}\n`
        );
      writeFileSync(
        join(distribution, "browser.js"),
        "export function f(callback) {\n  return 1;\n}\n"
      );
      manifest({ version: "1.0.0" });
      assert.equal(
        runNative(["contract", "generate", "--package-root", packageRoot, "--output", contract])
          .status,
        0
      );
      const plan = JSON.parse(readFileSync(join(packageRoot, "solid-reactivity.review.json"), "utf8"));
      const legacy = plan.items.find(item => item.kind === "legacy-root-field");
      assert.doesNotMatch(legacy.text, /"main"/);
      const answers = join(directory, "answers.json");
      writeFileSync(
        answers,
        `${JSON.stringify(
          Object.fromEntries(plan.items.map(item => [item.id, decisionFor(item.kind)]))
        )}\n`
      );
      assert.notEqual(runNative(["contract", "review", contract, "--answers", answers]).status, 2);
      assert.equal(runNative(["contract", "review", contract, "--promote", "reviewed"]).status, 0);

      writeFileSync(join(distribution, "node.cjs"), "module.exports = {};\n");
      manifest({ version: "2.0.0", main: "./dist/node.cjs" });
      assert.equal(
        runNative(["contract", "generate", "--package-root", packageRoot, "--output", contract])
          .status,
        0
      );
      const previous = contract.replace(/\.json$/, ".previous.json");
      const transferred = runNative([
        "contract",
        "review",
        contract,
        "--transfer-from",
        previous
      ]);
      assert.equal(transferred.status, 1, transferred.stdout + transferred.stderr);
      assert.match(
        transferred.stdout,
        /the legacy manifest field the root resolves from changed/
      );
      const refused = runNative(["contract", "review", contract, "--promote", "reviewed"]);
      assert.equal(refused.status, 1, refused.stdout);
      assert.match(refused.stderr, /not promoted: open review item .* legacy-root-field/);
    } finally {
      rmSync(directory, { recursive: true, force: true });
    }
  }
);

test("every argument is validated before anything is written", { skip: !canRun }, () => {
  const directory = mkdtempSync(join(tmpdir(), "solid-checker-argument-order-"));
  try {
    const before = republish(directory, "1.0.0");
    reviewEveryItem(before);
    const after = republish(directory, "1.1.0", manifest => {
      manifest.version = "1.1.0";
    });

    // A transfer in the first position and a typo in the last. The transfer used
    // to write the review state before the parse of the later flag failed, so a
    // run that exited non-zero had still changed the audit trail.
    const rejected = runNative([
      "contract",
      "review",
      after.contract,
      "--transfer-from",
      before.contract,
      "--resolve",
      "no-such-item=confirm"
    ]);
    assert.equal(rejected.status, 2, rejected.stdout);
    assert.match(rejected.stderr, /is not a review item/);
    assert.equal(existsSync(after.state), false, "nothing was written");
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("malformed and empty arguments are refused rather than ignored", () => {
  const { directory, contract, identify } = sentinelWorkspace();
  const statePath = join(directory, "solid-reactivity.review-state.json");
  try {
    for (const args of [
      ["--transfer-from", ""],
      ["--promote", ""],
      ["--answers", ""],
      ["--resolve", ""]
    ]) {
      const result = run(["contract", "review", contract, ...args]);
      assert.equal(result.status, 2, `${args[0]}: ${result.stdout}`);
      assert.match(result.stderr, new RegExp(`${args[0]} needs a non-empty value`));
    }
    const noted = run([
      "contract",
      "review",
      contract,
      "--resolve",
      `${identify("no-callback-row")}=absent`,
      "--note",
      ""
    ]);
    assert.equal(noted.status, 2, noted.stdout);
    assert.match(noted.stderr, /--note needs a non-empty value/);

    // Two decisions about one item in one invocation: the reviewer said two
    // different things and last-wins guessed which.
    const duplicated = run([
      "contract",
      "review",
      contract,
      "--resolve",
      `${identify("no-callback-row")}=absent`,
      "--resolve",
      `${identify("no-callback-row")}=confirm`
    ]);
    assert.equal(duplicated.status, 2, duplicated.stdout);
    assert.match(duplicated.stderr, /--resolve names .* more than once/);
    assert.equal(existsSync(statePath), false);

    // A review state this command cannot read is a refusal, never a silent
    // reset that reports "recorded" for decisions it discarded.
    writeFileSync(
      statePath,
      `${JSON.stringify({ schemaVersion: 1, contract: "", resolutions: ["nonsense"] }, null, 2)}\n`
    );
    const malformed = run([
      "contract",
      "review",
      contract,
      "--resolve",
      `${identify("no-callback-row")}=absent`
    ]);
    assert.equal(malformed.status, 2, malformed.stdout);
    assert.match(malformed.stderr, /is not an \{id: resolution\} object/);

    writeFileSync(
      statePath,
      `${JSON.stringify(
        { schemaVersion: 1, contract: "", resolutions: { "some-id": "absent" } },
        null,
        2
      )}\n`
    );
    const untyped = run(["contract", "review", contract]);
    assert.equal(untyped.status, 2, untyped.stdout);
    assert.match(untyped.stderr, /without a string "decision"/);

    // `plan` decides whether a plan beside the contract is accepted at all, so
    // a shape this cannot read is a refusal rather than a field to ignore.
    writeFileSync(
      statePath,
      `${JSON.stringify({ schemaVersion: 1, contract: "", plan: 7, resolutions: {} }, null, 2)}\n`
    );
    const badPlan = run(["contract", "review", contract]);
    assert.equal(badPlan.status, 2, badPlan.stdout);
    assert.match(badPlan.stderr, /has a "plan" field that is not a contract hash string/);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test(
  "a hand edit and its re-review survive across separate invocations",
  { skip: !canRun },
  () => {
    // Each step is its own process, which is the only way this proves anything:
    // the defect it pins was invisible whenever two steps shared one invocation.
    //
    // A hand edit moves the contract's bytes away from the hash its plan
    // carries, and so does the promotion at the end. Keying the plan binding off
    // `state.contract` -- which every write moves to the bytes on disk -- meant
    // the *first* invocation after an edit still matched and the second no
    // longer did, so re-making the stale resolutions, which is exactly what an
    // edit obliges a reviewer to do, dead-ended on "regenerate the contract".
    // `state.plan` records which plan this review answers and never moves.
    //
    // The edit is an owner row, deliberately: it answers its item without
    // changing what the export's summary certifies, so the generated-summary
    // question stays the one the plan asked. An edit that writes a *new*
    // positive claim raises a question the plan never asked, and the promotion
    // gate refuses it on those grounds -- correctly, and separately from this.
    const temporary = mkdtempSync(join(tmpdir(), "solid-checker-edit-flow-"));
    const packageRoot = join(temporary, "package");
    mkdirSync(packageRoot, { recursive: true });
    writeFileSync(
      join(packageRoot, "package.json"),
      `${JSON.stringify(
        { name: "owner-row-package", version: "1.0.0", type: "module", exports: "./index.js" },
        null,
        2
      )}\n`
    );
    writeFileSync(
      join(packageRoot, "index.js"),
      "export function thing(callback) {\n  callback();\n  return 1;\n}\n"
    );
    const contract = join(packageRoot, "solid-reactivity.json");
    const planPath = join(packageRoot, "solid-reactivity.review.json");
    const statePath = join(packageRoot, "solid-reactivity.review-state.json");
    try {
      assert.equal(
        runNative(["contract", "generate", "--package-root", packageRoot, "--output", contract])
          .status,
        0
      );
      const plan = JSON.parse(readFileSync(planPath, "utf8"));
      const ownerRow = plan.items.find(item => item.kind === "callback-without-owner-row");
      const others = plan.items
        .filter(item => item.id !== ownerRow.id)
        .flatMap(item => ["--resolve", `${item.id}=${decisionFor(item.kind)}`]);
      assert.ok(others.length, "the plan must ask something besides the owner row");

      assert.equal(runNative(["contract", "review", contract, ...others]).status, 1);
      assert.equal(JSON.parse(readFileSync(statePath, "utf8")).plan, plan.contract);

      const edited = JSON.parse(readFileSync(contract, "utf8"));
      for (const summary of Object.values(edited.summaries)) {
        for (const callback of summary.callbacks ?? []) callback.owner = "inherited";
      }
      writeFileSync(contract, `${JSON.stringify(edited, null, 2)}\n`);

      // Invocation two: the edit is answered.
      const byEdit = runNative([
        "contract",
        "review",
        contract,
        "--resolve",
        `${ownerRow.id}=resolved-by-edit`
      ]);
      assert.equal(byEdit.status, 1, byEdit.stdout + byEdit.stderr);
      // The edit made the earlier decisions stale, which is the whole safety
      // argument for accepting the plan at all.
      assert.match(byEdit.stdout, /^\[stale\] /m);

      // Invocation three: re-make the stale decisions against the edited bytes.
      const remade = runNative(["contract", "review", contract, ...others]);
      assert.equal(remade.status, 0, remade.stdout + remade.stderr);
      assert.match(remade.stdout, new RegExp(`${plan.items.length} resolved, 0 open, 0 stale`));

      // Invocation four: promote.
      const promoted = runNative(["contract", "review", contract, "--promote", "reviewed"]);
      assert.equal(promoted.status, 0, promoted.stdout + promoted.stderr);
      assert.equal(JSON.parse(readFileSync(contract, "utf8")).evidence.kind, "reviewed");

      // Invocation five: the plain CI listing gate, on a contract whose bytes
      // have now moved twice since the plan was written.
      const listed = runNative(["contract", "review", contract]);
      assert.equal(listed.status, 0, listed.stdout + listed.stderr);
      assert.match(listed.stdout, /0 open, 0 stale; 0 unknown claim\(s\) remaining; evidence reviewed/);
      const state = JSON.parse(readFileSync(statePath, "utf8"));
      assert.equal(state.plan, plan.contract, "the plan binding never moves");
      assert.notEqual(state.contract, plan.contract, "the byte pointer does");

      // And a foreign plan is still refused against that same state: `plan` only
      // ever gets a value from a write that already passed the binding.
      const foreign = JSON.parse(readFileSync(planPath, "utf8"));
      foreign.contract = `sha256:${"0".repeat(64)}`;
      writeFileSync(planPath, `${JSON.stringify(foreign, null, 2)}\n`);
      const swapped = runNative(["contract", "review", contract]);
      assert.equal(swapped.status, 2, swapped.stdout);
      assert.match(swapped.stderr, /regenerate the contract to write a matching review plan/);
    } finally {
      rmSync(temporary, { recursive: true, force: true });
    }
  }
);

test("a contract edited before any decision has nothing to bind its plan", { skip: !canRun }, () => {
  // The stated cost of keying the exception off the review state: with no
  // recorded decision there is no state, so nothing says this plan is this
  // contract's own, and the edit is indistinguishable from a swapped plan.
  const temporary = mkdtempSync(join(tmpdir(), "solid-checker-edit-first-"));
  const contract = join(temporary, "solid-reactivity.json");
  try {
    assert.equal(
      runNative([
        "contract",
        "generate",
        "--package-root",
        join(root, "fixtures/package-contracts/unknown-callback-claim"),
        "--output",
        contract
      ]).status,
      0
    );
    const document = JSON.parse(readFileSync(contract, "utf8"));
    for (const summary of Object.values(document.summaries)) {
      if (summary.callbacks?.status !== "unknown") continue;
      summary.callbacks = [{ parameter: 0, execution: "deferred", owner: "inherited" }];
    }
    writeFileSync(contract, `${JSON.stringify(document, null, 2)}\n`);
    const refused = runNative(["contract", "review", contract]);
    assert.equal(refused.status, 2, refused.stdout);
    assert.match(refused.stderr, /regenerate the contract to write a matching review plan/);
    assert.equal(existsSync(join(temporary, "solid-reactivity.review-state.json")), false);
  } finally {
    rmSync(temporary, { recursive: true, force: true });
  }
});
