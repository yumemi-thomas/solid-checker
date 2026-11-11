// ADR 0950: the what-if checker (`--assume <hazard>`) is reachable only by building it on purpose. The cargo feature `what-if` is in no `default` list, no
// workspace or CI file, and no make target but `build-what-if` and `test-what-if` turns it on, each into a target directory of its own; the one seam in the
// engine and the module behind it are compiled only with the feature. A default build is shown to hold none of it by the process test
// `rust/crates/solid-facts-backend/tests/what_if_default_process.rs` (a default binary answers `--assume` as an unknown argument and holds none of the what-if
// strings); this file pins the places a build could be made to take the feature, and where the code that reaches the seam may stand.
import assert from "node:assert/strict";
import { readFileSync, readdirSync, statSync } from "node:fs";
import { join } from "node:path";
import { test } from "vitest";

const ROOT = join(import.meta.dirname, "..");
const read = (path) => readFileSync(join(ROOT, path), "utf8");
const SKIPPED = new Set([".git", "node_modules", "dist", "target", ".claude"]);

/** The files under `directory` (relative to the repository) that `keep` accepts, build outputs and dependencies left out. */
function files(directory, keep, found = []) {
  for (const name of readdirSync(join(ROOT, directory))) {
    if (SKIPPED.has(name) || name.startsWith("target-")) continue;
    const path = join(directory, name);
    const stats = statSync(join(ROOT, path));
    if (stats.isDirectory()) files(path, keep, found);
    else if (keep(path)) found.push(path);
  }
  return found;
}

const codeLines = (text) => text.split("\n").map((line, at) => ({ at: at + 1, line })).filter(({ line }) => !/^\s*(#|\/\/)/.test(line));

// The two manifests that name the feature, and the lines they may name it in.
const ALLOWED_MANIFEST_LINES = {
  "rust/crates/solid-reactive-ir/Cargo.toml": [/^what-if = \[\]$/],
  "rust/crates/solid-facts-backend/Cargo.toml": [/^what-if = \["solid-reactive-ir\/what-if"\]$/, /^required-features = \["what-if"\]$/],
};

test("the feature is named by two manifests only, as itself and as the one test target that needs it, and enables nothing else", () => {
  const manifests = files(".", (path) => path.endsWith("Cargo.toml"));
  assert.ok(manifests.length > 10, "the search found the workspace's manifests");
  for (const manifest of manifests) {
    const allowed = ALLOWED_MANIFEST_LINES[manifest] ?? [];
    for (const { at, line } of codeLines(read(manifest))) {
      if (!line.includes("what-if")) continue;
      assert.ok(allowed.some((pattern) => pattern.test(line.trim())), `${manifest}:${at} names the what-if feature: ${line.trim()}`);
    }
  }
  for (const manifest of Object.keys(ALLOWED_MANIFEST_LINES)) {
    for (const pattern of ALLOWED_MANIFEST_LINES[manifest]) assert.ok(read(manifest).split("\n").some((line) => pattern.test(line.trim())), `${manifest} lost a line this test pins: ${pattern}`);
  }
});

test("no `default` feature list holds the feature, in any manifest", () => {
  for (const manifest of files(".", (path) => path.endsWith("Cargo.toml"))) {
    for (const [, list] of read(manifest).matchAll(/^default\s*=\s*\[([^\]]*)\]/gm)) assert.ok(!list.includes("what-if"), `${manifest}: ${list}`);
  }
});

/** A recipe's commands, a line continued with a backslash joined to the next. */
const commands = (recipe) => recipe.join("\n").replaceAll(/\\\n\s*/g, " ").split("\n");

/** The rules of the Makefile: `{ names, prerequisites, recipe }`, one per rule. */
function rules(text) {
  const found = [];
  let current = null;
  for (const line of text.split("\n")) {
    const head = /^([A-Za-z0-9_./%-]+(?: [A-Za-z0-9_./%-]+)*)\s*:(?!=)\s*(.*)$/.exec(line);
    if (head && !line.startsWith("\t")) {
      current = { names: head[1].split(" "), prerequisites: head[2].split(/\s+/).filter(Boolean), recipe: [] };
      found.push(current);
    } else if (line.startsWith("\t") && current) current.recipe.push(line);
    else if (!line.startsWith("\t") && line.trim() !== "") current = null;
  }
  return found;
}

const MAKEFILE = read("Makefile");
const TARGETS = ["build-what-if", "test-what-if"];

test("only `make build-what-if` and `make test-what-if` turn the feature on, and nothing depends on either", () => {
  for (const rule of rules(MAKEFILE)) {
    const names = rule.names.join(" ");
    const turnsOn = rule.recipe.some((line) => /what-if/.test(line));
    if (names === ".PHONY") continue;
    if (TARGETS.some((target) => rule.names.includes(target))) {
      assert.ok(rule.recipe.length > 0, `${names} has no recipe`);
      continue;
    }
    assert.ok(!turnsOn, `the recipe of \`${names}\` names the what-if feature or binary`);
    for (const target of TARGETS) assert.ok(!rule.prerequisites.includes(target), `\`${names}\` depends on \`${target}\`: the feature would be built by another target`);
  }
  for (const target of TARGETS) assert.ok(rules(MAKEFILE).some((rule) => rule.names.includes(target)), `the Makefile has no \`${target}\``);
});

test("each what-if recipe builds into the what-if target directory, which is not the default one, and the binary has its own name", () => {
  assert.match(MAKEFILE, /^WHAT_IF_TARGET_DIR \?= \$\(CURDIR\)\/rust\/target-what-if$/m);
  for (const rule of rules(MAKEFILE).filter((candidate) => TARGETS.some((target) => candidate.names.includes(target)))) {
    const cargo = commands(rule.recipe).filter((line) => /\bcargo\b/.test(line));
    assert.ok(cargo.length > 0, rule.names.join(" "));
    for (const line of cargo) {
      assert.match(line, /CARGO_TARGET_DIR="\$\(WHAT_IF_TARGET_DIR\)"/, `${rule.names.join(" ")}: ${line.trim()}`);
      assert.match(line, /--features what-if|--features=what-if/, `${rule.names.join(" ")}: ${line.trim()}`);
    }
  }
  const build = rules(MAKEFILE).find((rule) => rule.names.includes("build-what-if"));
  assert.ok(build.recipe.some((line) => /cp "\$\(WHAT_IF_TARGET_DIR\)\/release\/solid-checker-rust" "\$\(WHAT_IF_TARGET_DIR\)\/release\/solid-checker-what-if"/.test(line)), "the binary is copied to its own name");
  assert.ok(!/^\s*(CARGO_TARGET_DIR|WHAT_IF_TARGET_DIR)\s*[?:]?=\s*\$\(CURDIR\)\/rust\/target\s*$/m.test(MAKEFILE), "the what-if target directory is the default one");
});

test("no workflow, config, script or package script asks cargo for the feature", () => {
  const places = [
    ...files(".github", () => true),
    ...files("scripts", (path) => /\.(mjs|cjs|js|ts|sh|json|yml|yaml)$/.test(path) && !path.endsWith("what-if-gates.test.mjs")),
    ...files("packages", (path) => /(^|\/)package\.json$/.test(path)),
    ".cargo/config.toml",
    "package.json",
    "rust/Cargo.toml",
  ];
  for (const path of places) {
    let text;
    try {
      text = read(path);
    } catch {
      continue;
    }
    assert.ok(!/--features[= ]+["']?[^\s"']*what-if/.test(text), `${path} builds with the what-if feature`);
    assert.ok(!/solid-reactive-ir\/what-if/.test(text), `${path} names the what-if feature of the engine`);
  }
});

const RUST_SOURCES = files("rust", (path) => path.endsWith(".rs") && /\/src\//.test(path));

test("the module is declared once, behind the feature, and the one seam is `hazard_present` alone", () => {
  const lib = codeLines(read("rust/crates/solid-reactive-ir/src/lib.rs")).filter(({ line }) => /\bwhat_if\b/.test(line));
  assert.equal(lib.length, 1, JSON.stringify(lib));
  const libText = read("rust/crates/solid-reactive-ir/src/lib.rs").split("\n");
  assert.equal(libText[lib[0].at - 2].trim(), '#[cfg(feature = "what-if")]', "the module is not behind the feature");
  assert.equal(lib[0].line.trim(), "pub mod what_if;");

  const calls = [];
  for (const path of RUST_SOURCES) {
    if (path.endsWith("/what_if.rs")) continue;
    const lines = read(path).split("\n");
    lines.forEach((line, at) => {
      if (/assumed_absent\(/.test(line) && !/^\s*\/\//.test(line)) calls.push({ path, at, lines });
    });
  }
  assert.equal(calls.length, 1, `the seam is reached from ${calls.map(({ path, at }) => `${path}:${at + 1}`).join(", ")}`);
  const [{ path, at, lines }] = calls;
  assert.equal(path, "rust/crates/solid-reactive-ir/src/indexes.rs");
  assert.equal(lines[at - 1].trim(), '#[cfg(feature = "what-if")]', "the seam is not behind the feature");
  const owner = [...lines.slice(0, at)].reverse().find((line) => /^\s*(pub(\([a-z]+\))? )?fn /.test(line));
  assert.match(owner, /fn hazard_present\(/, "the seam is not in `hazard_present`");
});

test("the engine's other what-if reaches are the process boundary's and the hazard ids', each behind the feature", () => {
  // Every line that names the module or its helpers, outside the module: where it may stand.
  const reaches = new Set();
  for (const path of RUST_SOURCES) {
    if (path.endsWith("/what_if.rs")) continue;
    for (const { line } of codeLines(read(path))) {
      if (/\bwhat_if::|\bwhat_if_binary\(\)|hazard_of_id\(/.test(line)) reaches.add(path);
    }
  }
  assert.deepEqual([...reaches].sort(), [
    "rust/crates/solid-facts-backend/src/main.rs",
    "rust/crates/solid-reactive-ir/src/base/contributions/blocked_by.rs",
    "rust/crates/solid-reactive-ir/src/indexes.rs",
  ]);
  // `hazard_of_id` itself is compiled only with the feature: the attribute stands above it (past its doc comment).
  const blockedBy = read("rust/crates/solid-reactive-ir/src/base/contributions/blocked_by.rs").split("\n");
  const at = blockedBy.findIndex((line) => /fn hazard_of_id\(/.test(line));
  assert.ok(at > 0, "`hazard_of_id` is gone");
  const above = blockedBy.slice(0, at).reverse().filter((line) => !/^\s*\/\/\//.test(line));
  assert.equal(above[0].trim(), '#[cfg(feature = "what-if")]', "`hazard_of_id` is not behind the feature");
});

/**
 * Whether the statement or item at `lines[at]` is compiled only with the feature: a `#[cfg(...feature = "what-if"...)]` stands directly above it (past
 * comments and other attributes), or above the header of a block that encloses it, however deep.
 */
function gated(lines, at) {
  const indent = (line) => line.match(/^\s*/)[0].length;
  for (let current = at; current >= 0; ) {
    let above = current - 1;
    while (above >= 0 && /^\s*(\/\/|#\[(?!cfg))/.test(lines[above])) above -= 1;
    if (above >= 0 && /^\s*#\[cfg\(.*feature = "what-if"/.test(lines[above])) return true;
    if (indent(lines[current]) === 0) return false;
    let header = current - 1;
    while (header >= 0 && (lines[header].trim() === "" || /^\s*[)\]}]/.test(lines[header]) || indent(lines[header]) >= indent(lines[current]))) header -= 1;
    current = header;
  }
  return false;
}

test("the process boundary takes the feature as one thing: every use of the module or of `what_if_binary` is cfg-gated", () => {
  const lines = read("rust/crates/solid-facts-backend/src/main.rs").split("\n");
  let checked = 0;
  for (const [at, line] of lines.entries()) {
    if (/^\s*\/\//.test(line) || !/what_if::|what_if_binary\(|UNTAINTED_LOGS/.test(line)) continue;
    checked += 1;
    assert.ok(gated(lines, at), `main.rs:${at + 1} reaches the what-if build with no cfg(feature = "what-if") above it or its block: ${line.trim()}`);
  }
  assert.ok(checked >= 6, `the scan found ${checked} uses: it no longer looks where they are`);
});
