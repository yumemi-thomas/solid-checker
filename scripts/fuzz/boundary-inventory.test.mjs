import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, test } from "vitest";

import { armKind, dialectQuestions, fnCount, taggedArms } from "./boundary-inventory.mjs";

// ADR 0995: the inventories read the Rust source with patterns, so a source shape the pattern misses is an item no shape covers.
const rootWith = (file, text) => {
  const root = mkdtempSync(join(tmpdir(), "inventory-"));
  writeFileSync(join(root, file), text);
  return root;
};

describe("the dialect's questions are every fn of the impl", () => {
  const impl = (methods) => `pub struct Solid2;\nimpl Dialect for Solid2 {\n${methods}\n}\n`;

  test("a method the scan's indentation and spelling miss makes the inventory throw instead of dropping the question", () => {
    // The laundering: `pub fn` (or a generic, or another indent) is a question of the dialect that dialectQuestions never lists, so no pair
    // or stated gap is ever asked for it and boundary.test.mjs stays green.
    const root = rootWith("solid_2.rs", impl("    fn kept(&self) -> bool { true }\n    pub fn missed(&self) -> bool { true }"));
    expect(() => dialectQuestions(root)).toThrow(/declares 2 fn items but 1 were parsed/);
    const generic = rootWith("solid_2.rs", impl("    fn kept(&self) -> bool { true }\n    fn other<T>(&self, x: T) -> bool { true }"));
    expect(() => dialectQuestions(generic)).toThrow(/declares 2 fn items but 1/);
    const indented = rootWith("solid_2.rs", impl("    fn kept(&self) -> bool { true }\n  fn missed(&self) -> bool { true }"));
    expect(() => dialectQuestions(indented)).toThrow(/declares 2 fn items but 1/);
  });

  test("every fn at the scanned shape is listed; comments and strings that say fn are not counted", () => {
    const root = rootWith("solid_2.rs", impl('    // fn in a comment\n    fn one(&self) -> &str { "fn two" }\n    fn two(&self) {}'));
    expect(dialectQuestions(root).map((question) => question.id)).toEqual(["one", "two"]);
    expect(fnCount('fn a() {} /* fn b() */ // fn c()\nlet s = "fn d()";')).toBe(1);
  });

  test("a second impl block is refused: the second's methods are read by no one", () => {
    const root = rootWith("solid_2.rs", `${impl("    fn one(&self) {}")}\n${impl("    fn two(&self) {}")}`);
    expect(() => dialectQuestions(root)).toThrow(/2 `impl Dialect for Solid2` blocks/);
  });

  test("the real impl parses whole", () => {
    expect(dialectQuestions().length).toBeGreaterThan(100);
  });
});

describe("an arm with any non-literal role expression is computed", () => {
  test("armKind: literals classify; anything else is computed, a literal beside it or not", () => {
    // The laundering: `if c { ExecutionRole::EventCallback } else { role_of(x) }` held a literal silent role, so it was classed `read-silent`
    // (a literal) and a pair was never asked for the transported branch.
    expect(armKind("ExecutionRole::TrackedJsx", ["TrackedJsx"])).toBe("read-silent");
    expect(armKind("Some(ExecutionRole::TrackedJsx),", ["TrackedJsx"])).toBe("read-silent");
    expect(armKind("ExecutionRole::EffectApply", ["EffectApply"])).toBe("write-silent-only");
    expect(armKind("ExecutionRole::Unknown", ["Unknown"])).toBe("unknown");
    expect(armKind("ExecutionRole::TrackedUntracked", ["TrackedUntracked"])).toBe("reporting");
    expect(armKind("if c { ExecutionRole::EventCallback } else { role_of(x) }", ["EventCallback"])).toBe("computed");
    expect(armKind("inherited.unwrap_or(ExecutionRole::Unknown)", ["Unknown"])).toBe("computed");
    expect(armKind("if c { ExecutionRole::EventCallback } else { ExecutionRole::TrackedJsx }", ["EventCallback", "TrackedJsx"])).toBe("computed");
    expect(armKind("first_run", [])).toBe("computed");
    expect(armKind("ExecutionRole::TrackedJsx // a comment", ["TrackedJsx"])).toBe("read-silent");
  });

  test("a tagged arm in a source is classed through armKind", () => {
    const root = rootWith("execution_role.rs", 'fn f() {\n  role_arm!("execution_role.literal", ExecutionRole::TrackedJsx);\n  role_arm!("execution_role.mixed", if c { ExecutionRole::EventCallback } else { other(x) });\n}\n');
    expect(taggedArms(root).map((arm) => [arm.id, arm.kind])).toEqual([["execution_role.literal", "read-silent"], ["execution_role.mixed", "computed"]]);
  });
});
