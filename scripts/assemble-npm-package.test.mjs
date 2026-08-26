import assert from "node:assert/strict";
import {
  chmodSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  statSync,
  writeFileSync
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { spawnSync } from "node:child_process";
import { test } from "vitest";

const targets = [
  "linux-x64",
  "linux-arm64",
  "darwin-arm64",
  "win32-x64"
];

test("restores executable permissions after artifact download", () => {
  const directory = mkdtempSync(join(tmpdir(), "solid-checker-assemble-"));
  const input = join(directory, "artifacts");
  const output = join(directory, "packages");

  try {
    for (const target of targets) {
      const [platform, arch] = target.split("-");
      const extension = platform === "win32" ? ".exe" : "";
      const targetRoot = join(input, target);
      const nativeRoot = join(targetRoot, "native", target);
      mkdirSync(nativeRoot, { recursive: true });

      const binaries = {};
      for (const name of ["solid-checker", "solid-typefacts"]) {
        const path = `native/${target}/${name}${extension}`;
        const binary = join(targetRoot, path);
        writeFileSync(binary, "test binary");
        chmodSync(binary, 0o644);
        binaries[name] = { path, sha256: "test", bytes: 11 };
      }
      writeFileSync(
        join(targetRoot, "native-manifest.json"),
        `${JSON.stringify({ schema: 1, platform, arch, binaries })}\n`
      );
    }

    writeFileSync(join(input, targets[0], "package.json"), `${JSON.stringify({
      name: "solid-checker",
      version: "0.0.0",
      license: "MIT",
      repository: { type: "git", url: "https://example.com/solid-checker.git" },
      publishConfig: { access: "public" }
    })}\n`);

    const result = spawnSync(process.execPath, [
      new URL("./assemble-npm-package.mjs", import.meta.url).pathname,
      input,
      output,
      "1.2.3"
    ], { encoding: "utf8" });
    assert.equal(result.status, 0, result.stderr);

    for (const target of targets) {
      const platform = target.split("-")[0];
      const suffix = {
        "linux-x64": "linux-x64-gnu",
        "linux-arm64": "linux-arm64-gnu",
        "darwin-arm64": "darwin-arm64",
        "win32-x64": "win32-x64-msvc"
      }[target];
      const extension = platform === "win32" ? ".exe" : "";
      const binary = join(
        output,
        `binding-${suffix}`,
        "native",
        target,
        `solid-checker${extension}`
      );
      const mode = statSync(binary).mode & 0o777;
      assert.equal(mode, platform === "win32" ? 0o644 : 0o755);
    }

    const packageJson = JSON.parse(
      readFileSync(join(output, "solid-checker", "package.json"), "utf8")
    );
    assert.equal(packageJson.version, "1.2.3");
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});
