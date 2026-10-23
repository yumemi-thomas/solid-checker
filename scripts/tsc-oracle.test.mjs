import assert from "node:assert/strict";
import { test } from "vitest";

import { nativeCompilerProblems, oracleSubjectSpan } from "./tsc-oracle.mjs";

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

test("a native compiler package that is not pinned is refused", () => {
  const pinned = { "@typescript/typescript-linux-x64": "sha512-a" };
  const lock = (...names) => names.map((name) => ({ resolved: `${name}@7.1.0-beta`, integrity: "sha512-a" }));
  const x64 = "@typescript/typescript-linux-x64";
  const riscv = "@typescript/typescript-linux-riscv64";
  assert.deepEqual(nativeCompilerProblems(pinned, lock(x64), "linux-x64"), []);
  assert.equal(nativeCompilerProblems(pinned, lock(x64, riscv), "linux-x64").length, 1);
  assert.equal(nativeCompilerProblems(pinned, lock(riscv), "linux-riscv64").length, 2);
  assert.equal(nativeCompilerProblems(pinned, lock(), "linux-x64").length, 1);
});
