import assert from "node:assert/strict";
import { test } from "node:test";
import { runtimeFeedback } from "./runtime-feedback.mjs";

test("runtime observer ignores type/shape diagnostics and unsubscribes", () => {
  let listener, stopped = false;
  const observe = { diagnostics: { subscribe(fn) { listener = fn; return () => { stopped = true; }; } } };
  const run = runtimeFeedback(observe, { appRoot: "/consumer" });
  listener({ code: "MISSING_EFFECT_FN", severity: "error", message: "arity" });
  assert.equal(run.feedback.length, 0);
  listener({ code: "NO_OWNER_CLEANUP", severity: "warn", message: "ownership" });
  listener({ code: "NO_OWNER_CLEANUP", severity: "warn", message: "ownership" });
  assert.equal(run.feedback.length, 1); assert.equal(run.feedback[0].location, null);
  assert.equal(run.feedback[0].basis, "runtime-observation"); assert.equal(run.feedback[0].certification, false);
  run.clear(); listener({ code: "NO_OWNER_CLEANUP", severity: "warn", message: "ownership" });
  assert.equal(run.feedback.length, 1); run.stop(); assert.equal(stopped, true);
  assert.throws(() => runtimeFeedback({}, { appRoot: "/consumer" }), /no diagnostic/);
});
