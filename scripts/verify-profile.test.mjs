import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "vitest";

const cargoManifest = readFileSync("rust/Cargo.toml", "utf8");
const verifyScript = readFileSync("scripts/verify.sh", "utf8");

test("full verification uses a disk-lean Cargo profile", () => {
  assert.match(
    cargoManifest,
    /\[profile\.verify\][\s\S]*?inherits\s*=\s*"dev"[\s\S]*?debug\s*=\s*0[\s\S]*?incremental\s*=\s*false/,
  );
  assert.match(verifyScript, /cargo_profile=verify/);
  assert.match(verifyScript, /--cargo-profile "\$cargo_profile"/);
  assert.match(verifyScript, /--profile "\$cargo_profile"/);
  assert.match(verifyScript, /checker_bin="\$PWD\/rust\/target\/\$cargo_profile\/solid-checker-rust"/);
  assert.doesNotMatch(verifyScript, /rust\/target\/debug\/solid-checker-rust/);
});
