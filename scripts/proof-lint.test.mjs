import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import { basename, join } from "node:path";
import { describe, test } from "vitest";

import { ROOT } from "./ir-read-lint.mjs";
import { baselineFor, endIds, FINDING_FILTER_ONLY, HAND_OFF_WINDOW, hitsOf, KIND_NAMES, PENDING, premiseKinds, problems, proseHandOffs, SCANNED, seenLists, seenMutators, silentRoleValues, SITE_ACCOUNTING } from "./proof-lint.mjs";

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

  // ADR 0945, G3: a primitive identified by its enum name outside the dialect crate.
  test("a primitive compared or matched by its name is a hit, another type's variant of the same suffix is none", () => {
    assert.deepEqual(kinds("if lookup.primitive_at_call(file, call.span) == Some(solid_dialect::Primitive::Untrack) {", false), ["primitive-name-match"]);
    assert.deepEqual(kinds("if primitive != Primitive::CreateSignal {", false), ["primitive-name-match"]);
    assert.deepEqual(kinds("if kind == Some(Primitive::Resolve)", false), ["primitive-name-match"]);
    assert.deepEqual(kinds("!matches!(kind, Primitive::OnCleanup | Primitive::Flush)", false), ["primitive-name-match"]);
    assert.deepEqual(kinds("|| matches!(role.effect_apply_primitive, EffectApplyPrimitive::Several);", false), []);
    assert.deepEqual(kinds("let primitive = Primitive::Untrack;", false), []);
    assert.deepEqual(kinds("if primitive == self.untrack {", false), []);
  });

  test("the file that owns the vocabulary is not asked for a primitive name, and is asked for the rest", () => {
    const line = "if primitive == Primitive::Untrack {";
    assert.deepEqual(hitsOf(line, false, true).map((hit) => hit.kind), []);
    assert.deepEqual(hitsOf(`${line}\n_ => {}\n`, false, true).map((hit) => hit.kind), ["silent-unmatched-slot"]);
  });

  // ADR 0945, G4: the arm for the case no arm names ends the walk with no result.
  test("an unmatched arm that does nothing, continues, breaks or returns none is a hit, one that answers is none", () => {
    for (const arm of ["_ => {}", "None => {},", "_ => continue,", "None => continue", "_ => break,", "_ => return None,", "None => return None,", "_ => return,", "_ => {  }"]) {
      assert.deepEqual(kinds(arm, false), ["silent-unmatched-slot"], arm);
    }
    assert.deepEqual(kinds("match slot { Some(x) => visit(x), _ => {} }", false), ["silent-unmatched-slot"]);
    for (const arm of ["_ => Unknown,", "None => Some(role),", "_ => false,", "_ => return Some(x),", "Some(_) => {}", "Foo => {}", "_ => { record(slot); }", "x_ => {}"]) {
      assert.deepEqual(kinds(arm, false), [], arm);
    }
  });

  // ADR 0958: code that discards findings or sites by predicate or equality.
  test("a retain, an extract_if and a dedup of rows or sites is a finding-filter hit, a dedup of names is none", () => {
    for (const line of [
      "findings.retain(|finding| keep(finding));",
      "draft.writes.retain_mut(|write| gate(write));",
      "rows.extract_if(.., |row| row.dropped());",
      "sites.dedup();",
      "claims.dedup_by(|left, right| left.location == right.location && left.id == right.id);",
      "findings = findings.into_iter().filter(|finding| keep(finding)).collect();",
    ]) {
      assert.deepEqual(kinds(line, false), ["finding-filter"], line);
    }
    for (const line of ["names.dedup();", "symbols.dedup();", "let kept = findings.iter().filter(|f| keep(f));", "findings.sort();", "findings.push(row);"]) {
      assert.deepEqual(kinds(line, false), [], line);
    }
  });

  test("the backend files where findings leave the report are asked for finding-filter only", () => {
    const text = "findings.retain(|finding| keep(finding));\nif primitive == Primitive::Untrack {}\nx.is_none_or(|y| y);\n_ => {}\n";
    assert.deepEqual(hitsOf(text, false, false, "finding-filter").map((hit) => hit.kind), ["finding-filter"]);
    assert.ok(FINDING_FILTER_ONLY.includes("rust/crates/solid-facts-backend/src/package_drop.rs"));
    assert.ok(FINDING_FILTER_ONLY.includes("rust/crates/solid-facts-backend/src/diagnostics.rs"));
  });

  test("test modules and comments are left out", () => {
    assert.deepEqual(kinds("// source.is_none_or(|x| x)\n#[cfg(test)]\nmod tests {\n    fn t() { let _ = x.unwrap_or(true); }\n}\n", false), []);
  });
});

// ADR 0942, batch 11 slice 10b: a comment that hands a case to another check, above code that drops it, with no typed `Delegate`.
describe("a prose hand-off is a comment that hands a case over, above a drop", () => {
  const lines = (text) => proseHandOffs(text).map((hit) => hit.line);

  test("a comment naming another rule's claim above a `continue` is a hit, at the comment's line", () => {
    const text = "fn walk() {\n    // Flush first (SC2006's claim), so that kind is not this walk's.\n    if kind == Flush {\n        continue;\n    }\n}\n";
    assert.deepEqual(lines(text), [2]);
    assert.deepEqual(proseHandOffs(text).map((hit) => hit.kind), ["prose-hand-off"]);
    assert.ok(KIND_NAMES.includes("prose-hand-off"));
    assert.deepEqual(hitsOf(text, false).map((hit) => hit.kind).filter((kind) => kind === "prose-hand-off"), ["prose-hand-off"]);
  });

  test("each phrase of the claim, the judging, the pass and the walk is read, in a comment or a doc comment", () => {
    for (const phrase of ["// SC4001's claim", "/// that function's to judge", "// stored-throw's to report", "// the owner graph's to answer", "// left to the stored-throw pass", "// not this walk's", "//! left to the other pass"]) {
      assert.deepEqual(lines(`${phrase}\nreturn None;\n`), [1], phrase);
    }
    for (const phrase of ["// the claim is that the bindings are read once", "// SC4001 claims it", "// this walk's own claim", "let text = \"SC2006's claim\";", "// left to chance"]) {
      assert.deepEqual(lines(`${phrase}\nreturn None;\n`), [], phrase);
    }
  });

  test("each drop is read: continue, return None, return false, a None arm, a retain, a kind filter beside a record or a push", () => {
    for (const drop of ["continue;", "return None;", "return false;", "Some(_) => None,", "findings.retain(|row| keep(row));", "findings.retain_mut(|row| keep(row));"]) {
      assert.deepEqual(lines(`// SC4001's claim\n${drop}\n`), [1], drop);
    }
    assert.deepEqual(lines("// SC2006's claim\nif kind != LeafOwnerOperationKind::Flush {\n    record(kinds, kind);\n}\n"), [1]);
    assert.deepEqual(lines("// SC2006's claim\nif matches!(kind, Kind::Flush) {\n    rows.push(row);\n}\n"), [1]);
    // a kind filter with nothing it guards, and code that only reads, are no drop
    assert.deepEqual(lines("// SC2006's claim\nif kind != LeafOwnerOperationKind::Flush {\n    return Some(kind);\n}\n"), []);
    assert.deepEqual(lines("// SC4001's claim\nlet owner = lookup.owner(span)?;\nreturn Some(owner);\n"), []);
  });

  test("a typed Delegate in the window is the hand-off, so the comment is none", () => {
    assert.deepEqual(lines("// SC3001's claim: the hand-off\nreturn ends.delegate(Delegate::LeafOwnerForbiddenCall);\ncontinue;\n"), []);
    assert.deepEqual(lines("// SC3001's claim\nif owner.leaf() {\n    continue;\n}\nlet end = Delegate::LeafOwnerForbiddenCall;\n"), []);
  });

  test("the window is twenty code lines: comments and blank lines do not count, the drop past it is not seen", () => {
    const filler = (count) => Array.from({ length: count }, (_, index) => `let step_${index} = ${index};`).join("\n");
    assert.equal(HAND_OFF_WINDOW, 20);
    assert.deepEqual(lines(`// SC4001's claim\n${filler(HAND_OFF_WINDOW - 1)}\ncontinue;\n`), [1]);
    assert.deepEqual(lines(`// SC4001's claim\n${filler(HAND_OFF_WINDOW)}\ncontinue;\n`), []);
    assert.deepEqual(lines(`// SC4001's claim\n\n// a note\n${filler(HAND_OFF_WINDOW - 1)}\n\n// another note\ncontinue;\n`), [1]);
  });

  test("a comment in a test item is left out, one after it is read, and a single-kind scan does not run the pass", () => {
    const text = "#[cfg(test)]\nmod tests {\n    // SC4001's claim\n    fn t() { continue; }\n}\n// SC2006's claim\nreturn None;\n";
    assert.deepEqual(lines(text), [6]);
    assert.deepEqual(hitsOf("// SC4001's claim\ncontinue;\n", false, false, "finding-filter"), []);
    assert.deepEqual(hitsOf("// SC4001's claim\ncontinue;\n", false, false, "prose-hand-off").map((hit) => hit.kind), ["prose-hand-off"]);
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
