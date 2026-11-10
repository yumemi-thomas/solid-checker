import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import { basename, join } from "node:path";
import { describe, test } from "vitest";

import { ROOT } from "./ir-read-lint.mjs";
import { baselineFor, endIds, hitsOf, PENDING, premiseKinds, problems, SCANNED, seenLists, seenMutators, silentRoleValues, SITE_ACCOUNTING } from "./proof-lint.mjs";

const kinds = (text, producer = true) => hitsOf(text, producer).map((hit) => hit.kind);

describe("a silent role is produced, compared or matched", () => {
  test("a return, a match arm result, a binding and a Some produce a role", () => {
    assert.equal(silentRoleValues("return ExecutionRole::TrackedJsx;").length, 1);
    assert.equal(silentRoleValues("Some(Execution::Tracked) => Some(ExecutionRole::TrackedJsx),").length, 1);
    assert.equal(silentRoleValues("let role = ExecutionRole::EventCallback;").length, 1);
    assert.equal(silentRoleValues(".then_some(ExecutionRole::DeferredCallback)").length, 1);
  });

  test("a comparison, a pattern, an alternation and an import produce none", () => {
    assert.equal(silentRoleValues("if role == ExecutionRole::TrackedJsx {").length, 0);
    assert.equal(silentRoleValues("if role != crate::ExecutionRole::EventCallback {").length, 0);
    assert.equal(silentRoleValues("ExecutionRole::TrackedJsx => true,").length, 0);
    assert.equal(silentRoleValues("| ExecutionRole::EventCallback").length, 0);
    assert.equal(silentRoleValues("matches!(role, ExecutionRole::EffectApply)").length, 0);
    assert.equal(silentRoleValues("use crate::ExecutionRole::TrackedJsx;").length, 0);
  });

  test("a role inside a role_arm! call, on one line or several, is tagged", () => {
    assert.deepEqual(kinds('return role_arm!("a.b", ExecutionRole::TrackedJsx);'), []);
    assert.deepEqual(kinds('return role_arm!(\n    "a.b",\n    ExecutionRole::TrackedJsx\n);'), []);
    assert.deepEqual(kinds("return ExecutionRole::TrackedJsx;"), ["untagged-silent-role"]);
  });

  test("a file that produces no roles is not asked", () => {
    assert.deepEqual(kinds("return ExecutionRole::TrackedJsx;", false), []);
  });
});

describe("the other shapes", () => {
  test("a decision read from text, a spelling match and a default on a missing fact", () => {
    assert.deepEqual(kinds('if file.source_text(span).is_some_and(|text| text.starts_with("on")) {', false), ["text-premise"]);
    assert.deepEqual(kinds('.any(|property| file.source_text(*property) == Some("keyed"))', false), ["name-only-match"]);
    assert.deepEqual(kinds("source.is_none_or(|symbol| !accessors.contains_key(symbol))", false), ["default-on-missing"]);
    assert.deepEqual(kinds("None => true,", false), ["default-on-missing"]);
  });

  test("test modules and comments are left out", () => {
    assert.deepEqual(kinds("// source.is_none_or(|x| x)\n#[cfg(test)]\nmod tests {\n    fn t() { let _ = x.unwrap_or(true); }\n}\n", false), []);
  });
});

describe("the typed premises", () => {
  const log = "pub(crate) enum PremiseKind {\n    /// doc\n    UserRuleOption,\n    /// doc\n    CallEdgePlaces,\n}\n";

  test("the variants of the premise enum are read from proof_log.rs", () => {
    assert.deepEqual(premiseKinds(log), ["UserRuleOption", "CallEdgePlaces"]);
  });

  const list = "site_silences! {\n    /// doc\n    A => \"local_access.discover_member.props-static\",\n    /// doc\n    B => \"shared_reactivity.unreached_write.cannot-run\",\n}\n";
  const stems = new Set(["local_access", "shared_reactivity"]);

  test("the ids of the named ends are read from the site_silences! list, with their variants", () => {
    assert.deepEqual(endIds(list, stems).map((end) => [end.variant, end.id]), [
      ["A", "local_access.discover_member.props-static"],
      ["B", "shared_reactivity.unreached_write.cannot-run"],
    ]);
  });

  test("a missing list, a duplicate id and an id with no producer file throw", () => {
    assert.throws(() => endIds("fn main() {}\n", stems), /no `site_silences!` list/);
    assert.throws(() => endIds(list.replace("B =>", "A2 =>").replace("shared_reactivity.unreached_write.cannot-run", "local_access.discover_member.props-static"), stems), /listed twice/);
    assert.throws(() => endIds(list, new Set(["local_access"])), /no producer file shared_reactivity/);
  });

  test("the real list holds at least 30 ends, each with a producer file", () => {
    const stemsOf = new Set(SCANNED.flatMap((directory) => readdirSync(join(ROOT, directory), { recursive: true })).filter((path) => path.endsWith(".rs")).map((path) => basename(path, ".rs")));
    const ends = endIds(readFileSync(join(ROOT, SITE_ACCOUNTING), "utf8"), stemsOf);
    assert.ok(ends.length >= 30, `found ${ends.length} named ends`);
  });
});

describe("the baseline", () => {
  const counted = { "a.rs": { "name-only-match": 2 } };
  const stated = "the attribute name is the dialect's own spelling of a prop, not a binding resolution";

  test("a new hit fails, a hit that went fails until the baseline shrinks, and an equal count passes", () => {
    const allowed = { "a.rs": { "name-only-match": { count: 2, why: PENDING } } };
    assert.deepEqual(problems(allowed, counted).found, []);
    assert.deepEqual(problems(allowed, { "a.rs": { "name-only-match": 3 } }).found.map((p) => p.kind), ["new"]);
    assert.deepEqual(problems(allowed, { "a.rs": { "name-only-match": 1 } }).found.map((p) => p.kind), ["stale"]);
    assert.deepEqual(problems({}, counted).found.map((p) => p.kind), ["new"]);
  });

  test("a reason states a fact or is pending review; the pending count is the progress metric", () => {
    assert.deepEqual(problems({ "a.rs": { "name-only-match": { count: 2, why: stated } } }, counted), { found: [], pending: 0 });
    assert.equal(problems({ "a.rs": { "name-only-match": { count: 2, why: PENDING } } }, counted).pending, 2);
    for (const why of ["", "known", "harmless", "short", undefined]) {
      assert.deepEqual(problems({ "a.rs": { "name-only-match": { count: 2, why } } }, counted).found.map((p) => p.kind), ["invalid"], String(why));
    }
  });

  test("an update keeps a stated reason and marks anything new pending review", () => {
    const previous = { "a.rs": { "name-only-match": { count: 5, why: stated } } };
    const next = baselineFor({ "a.rs": { "name-only-match": 2, "default-on-missing": 1 } }, previous);
    assert.deepEqual(next["a.rs"], { "default-on-missing": { count: 1, why: PENDING }, "name-only-match": { count: 2, why: stated } });
  });
});

describe("a closed seen list exempts no mutating builtin (ADR 0803)", () => {
  const list = 'const SEEN_STANDARD: &[(&str, bool, &str)] = &[\n    ("ObjectConstructor.keys", false, "returns the own keys"),\n    ("ObjectConstructor.assign", false, "returns its target"),\n];\n';

  test("the qualified names of a *SEEN* list are read, whatever the reasons say", () => {
    assert.deepEqual(seenLists(list).map((entry) => [entry.name, entry.entries.map((one) => one.qualified)]), [["SEEN_STANDARD", ["ObjectConstructor.keys", "ObjectConstructor.assign"]]]);
  });

  test("a mutating builtin in a list is flagged, a reading one is not", () => {
    assert.deepEqual(seenMutators(list).map((entry) => entry.qualified), ["ObjectConstructor.assign"]);
    assert.deepEqual(seenMutators(list.replace('"ObjectConstructor.assign"', '"ObjectConstructor.is"')), []);
  });

  test("another constant, a number and a list not named for being seen are no list", () => {
    assert.deepEqual(seenLists('const SEEN_DEPTH: usize = 3;\nconst LISTENER_ADDERS: [&str; 1] = ["Window.addEventListener"];\n'), []);
  });
});
