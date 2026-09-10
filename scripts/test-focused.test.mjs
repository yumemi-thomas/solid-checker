import assert from "node:assert/strict";
import { test } from "vitest";
import { focusedTestArguments, main } from "./test-focused.mjs";

test("focused tests reject an empty filter before starting any build", () => {
  assert.throws(() => main({}, () => { throw new Error("must not spawn"); }), /set TEST/);
  assert.throws(() => focusedTestArguments({ TEST: "--ignored" }), /set TEST/);
});

test("focused tests forward an exact name as one argument and keep pins on both Cargo calls", () => {
  const calls = [];
  const pinned = { SOLID_CHECKER_EXPECT_PROBE_PINS: "1", SOLID_TYPEFACTS_CERTIFICATION_SHA256: "pin" };
  const status = main({ TEST: "module::a_test", TEST_EXACT: "1" }, (command, args, options) => {
    calls.push({ command, args, options });
    return { status: 0, stdout: "module::a_test: test\n" };
  }, () => pinned);
  assert.equal(status, 0);
  assert.equal(calls[0].command, "make");
  assert.deepEqual(calls[1].args.slice(-4), ["module::a_test", "--", "--exact", "--list"]);
  assert.deepEqual(calls[2].args.slice(-3), ["module::a_test", "--", "--exact"]);
  assert.equal(calls[1].options.env, pinned);
  assert.equal(calls[2].options.env, pinned);
});

test("a misspelled filter and a failed build cannot report passing tests", () => {
  let calls = 0;
  assert.throws(() => main({ TEST: "typo" }, () => {
    calls++;
    return { status: 0, stdout: "0 tests, 0 benchmarks\n" };
  }, () => ({})), /matched no tests/);
  assert.equal(calls, 2);
  assert.equal(main({ TEST: "test" }, () => ({ status: 7 })), 7);
});
