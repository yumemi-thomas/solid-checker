// Validate and compare observations. This report does not grant proof authority.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { catalogSurface, surfaceClosed as closed } from "./catalog-surface.mjs";

const baseline = resolve(process.argv[2]), prefix = resolve(process.argv[3]);
const key = item => JSON.stringify([item.entrypoint, item.export]);
const hash = bytes => `sha256:${createHash("sha256").update(bytes).digest("hex")}`;
const report = { authority: false, kind: "automatic-composition-whole-corpus-comparison", hosts: [], allHostComplete: [] };
const complete = new Map();
let checker, producer;
for (const host of ["none", "browser", "node"]) {
  const before = JSON.parse(readFileSync(join(baseline, host === "none" ? "measure.json" : `measure-${host}.json`)));
  const after = JSON.parse(readFileSync(`${prefix}-${host}-1/results.json`));
  assert.equal(after.authority, false); assert.equal(after.authoredProposals, 0);
  assert.equal(after.completed, true); assert.equal(after.host, host);
  assert.equal(after.results.length, 97); assert.equal(before.packages.length, 97);
  assert.equal(new Set(after.results.map(row => row.package)).size, 97);
  assert.equal(hash(readFileSync(after.input)), after.inputSha256);
  checker ??= after.checkerSha256; producer ??= after.producerSha256;
  assert.equal(after.checkerSha256, checker); assert.equal(after.producerSha256, producer);
  const summary = { host, expectedExports: 0, baselineClean: 0, observedExports: 0, clean: 0,
    completePackages: [], refusedPackages: [], missingExports: [], addedExports: [], gains: [], losses: [], packages: [] };
  for (const previous of before.packages) {
    const raw = after.results.find(item => item.package === previous.package);
    assert(raw);
    const row = { ...raw };
    assert(row); assert.equal(row.version, previous.version);
    assert(["observed", "refused"].includes(row.status));
    let selfAdmitted = true;
    if (after.kind === "automatic-composition-surface-census") {
      const directories = [`${prefix}-${host}-1`, `${prefix}-${host}-supplement-1`];
      const combined = new Map(), admissions = [];
      for (const directory of directories) {
        const dir = join(directory, row.package.replaceAll("/", "__"));
        const nativePath = join(dir, "native.json");
        if (!existsSync(nativePath)) continue;
        const native = JSON.parse(readFileSync(nativePath));
        if (native.status === 0) {
          // Initial run-1 readers handled plain catalogs only. Recover the
          // publisher's named case-set catalogs without repeating certification
          // or modifying any original observation or proof input.
          for (const item of catalogSurface(join(dir, "accepted"), row.package, row.version)) {
            const prior = combined.get(key(item));
            combined.set(key(item), prior && !closed(prior.state) ? prior : item);
          }
          row.status = "observed";
          admissions.push(...native.stdout.split("\n").filter(line => line.startsWith("solid-checker:self-admission="))
            .map(line => JSON.parse(line.slice("solid-checker:self-admission=".length)))
            .filter(item => item.package === row.package));
        }
      }
      if (combined.size > 0) row.surface = [...combined.values()];
      selfAdmitted = [...new Set(previous.exports.map(item => item.entrypoint))].every(entrypoint => admissions.some(item => item.admitted === true
        && item.specifier === row.package + (entrypoint === "." ? "" : entrypoint.slice(1))));
    }
    const expected = new Map(previous.exports.map(item => [key(item), item]));
    const actual = new Map((row.surface ?? []).map(item => [key(item), item]));
    assert.equal(expected.size, previous.exports.length); assert.equal(actual.size, row.surface?.length ?? 0);
    const package_ = { package: row.package, version: row.version, status: row.status,
      runnerStatus: raw.status, selfAdmitted, expected: expected.size, observed: actual.size,
      clean: 0, gains: [], losses: [], missing: [], added: [] };
    summary.expectedExports += expected.size; summary.observedExports += actual.size;
    summary.baselineClean += previous.exports.filter(item => item.bucket === "clean").length;
    for (const [identity, prior] of expected) {
      const current = actual.get(identity);
      const address = { package: row.package, entrypoint: prior.entrypoint, export: prior.export };
      if (!current) { summary.missingExports.push(address); package_.missing.push(prior.export); }
      if (current && closed(current.state)) {
        summary.clean++; package_.clean++;
        if (prior.bucket !== "clean") { summary.gains.push(address); package_.gains.push(prior.export); }
      } else if (prior.bucket === "clean") {
        summary.losses.push({ ...address, state: current?.state ?? "unobserved" }); package_.losses.push(prior.export);
      }
    }
    for (const [identity, current] of actual)
      if (!expected.has(identity)) { summary.addedExports.push({ package: row.package, ...current }); package_.added.push(current.export); }
    // An accepted partial surface, or a package with no measured exports,
    // cannot count as complete even if every export present is clean.
    // Recompute from raw states rather than trusting the runner's convenience
    // counters. Initial run-1 counters counted callable `clean` only; proven
    // noncallable `value` exports are closed in the checkpoint too.
    package_.complete = row.status === "observed" && selfAdmitted && expected.size > 0
      && package_.missing.length === 0 && package_.added.length === 0 && package_.clean === expected.size;
    if (package_.complete) {
      summary.completePackages.push(row.package);
      const hosts = complete.get(row.package) ?? []; hosts.push(host); complete.set(row.package, hosts);
    }
    if (row.status === "refused") summary.refusedPackages.push({ package: row.package, reason: row.reason });
    summary.packages.push(package_);
  }
  assert.equal(summary.expectedExports, 721);
  summary.observedLosses = summary.losses.filter(item => item.state !== "unobserved");
  report.hosts.push(summary);
}
report.checkerSha256 = checker; report.producerSha256 = producer;
report.allHostComplete = [...complete].filter(([, hosts]) => hosts.length === 3).map(([package_]) => package_);
const output = `${prefix}-comparison.json`;
writeFileSync(output, JSON.stringify(report, null, 2) + "\n");
console.log(JSON.stringify({ output, allHostComplete: report.allHostComplete,
  hosts: report.hosts.map(({ host, expectedExports, baselineClean, observedExports, clean,
    completePackages, refusedPackages, missingExports, addedExports, gains, losses, observedLosses }) =>
    ({ host, expectedExports, baselineClean, observedExports, clean, completePackages,
      refusedPackages: refusedPackages.length, missingExports: missingExports.length,
      addedExports: addedExports.length, gains: gains.length, losses: losses.length,
      observedLosses: observedLosses.length })) }, null, 2));
