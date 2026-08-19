import assert from "node:assert/strict";
import test from "node:test";

import { oracleSubjectSpan } from "./tsc-oracle.mjs";

test("oracle subject parsing follows a .ts source extension", () => {
  const firstCall = "createEffect(() => 1, <Apply><unknown>5)";
  const code = [
    'import { createEffect } from "solid-js";',
    "type Apply = (value: number) => void;",
    `${firstCall};`,
    "createEffect(() => 1, 5 as unknown as Apply);",
    "",
  ].join("\n");
  const startByte = Buffer.byteLength(code.slice(0, code.indexOf(firstCall)), "utf8");
  const subject = oracleSubjectSpan(
    code,
    startByte,
    startByte + Buffer.byteLength("createEffect", "utf8"),
    "case.ts",
  );

  assert.deepEqual(subject, {
    startByte,
    endByte: startByte + Buffer.byteLength(firstCall, "utf8"),
  });
});
