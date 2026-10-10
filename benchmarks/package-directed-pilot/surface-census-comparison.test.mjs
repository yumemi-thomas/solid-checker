import { test } from "bun:test";
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdtempSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

test("whole-corpus comparison counts values and refuses missing or added surfaces", () => {
  const root = mkdtempSync(join(tmpdir(), "solid-checker-surface-comparison-"));
  const baseline = join(root, "before"), prefix = join(root, "after"); mkdirSync(baseline);
  const input = join(root, "input.json"); writeFileSync(input, "{}\n");
  const inputSha256 = "sha256:" + createHash("sha256").update(readFileSync(input)).digest("hex");
  for (const host of ["none", "browser", "node"]) {
    const packages = Array.from({ length: 97 }, (_, index) => ({ package: `package-${index}`, version: "1.0.0",
      exports: Array.from({ length: index === 0 ? 49 : 7 }, (_, position) =>
        ({ entrypoint: ".", export: `value${position}`, bucket: "clean" })) }));
    writeFileSync(join(baseline, host === "none" ? "measure.json" : `measure-${host}.json`), JSON.stringify({ packages }));
    const results = packages.map(package_ => ({ package: package_.package, version: package_.version,
      status: "observed", complete: true, clean: 0,
      surface: package_.exports.map(({ entrypoint, export: name }) => ({ entrypoint, export: name, state: "value" })) }));
    results[0].surface.pop(); // Complete=true must not conceal a missing export.
    results[1].surface.push({ entrypoint: ".", export: "unexpected", state: "clean" });
    results[2].surface[0].state = "every-import";
    results[3].status = "refused"; results[3].reason = "unavailable exact archive"; delete results[3].surface;
    const directory = `${prefix}-${host}-1`; mkdirSync(directory);
    writeFileSync(join(directory, "results.json"), JSON.stringify({ authority: false, authoredProposals: 0,
      completed: true, host, input, inputSha256, checkerSha256: "checker", producerSha256: "producer", results }));
  }
  const response = Bun.spawnSync([process.execPath, fileURLToPath(new URL("./check-surface-census.mjs", import.meta.url)), baseline, prefix]);
  assert.equal(response.exitCode, 0, response.stderr.toString());
  const report = JSON.parse(readFileSync(`${prefix}-comparison.json`));
  assert.equal(report.authority, false);
  assert.equal(report.allHostComplete.length, 93);
  for (const host of report.hosts) {
    assert.equal(host.expectedExports, 721); assert.equal(host.baselineClean, 721);
    assert.equal(host.clean, 712); assert.equal(host.missingExports.length, 8);
    assert.equal(host.addedExports.length, 1); assert.equal(host.losses.length, 9);
    assert.equal(host.gains.length, 0); assert.equal(host.refusedPackages.length, 1);
    assert(!host.completePackages.includes("package-0")); assert(!host.completePackages.includes("package-1"));
    assert(!host.completePackages.includes("package-2")); assert(!host.completePackages.includes("package-3"));
  }
});
