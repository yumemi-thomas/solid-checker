// Recounts retained checkpoint evidence. Does not predict proof-rule unlocks.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";

const checkpoint = resolve(process.argv[2]);
const output = resolve(process.argv[3]);
assert(!existsSync(output), "Preserve prior inventory evidence");
const results = [];
for (const host of ["none", "browser", "node"]) {
  const input = join(checkpoint, `measure${host === "none" ? "" : `-${host}`}.json`);
  const bytes = readFileSync(input);
  const measurement = JSON.parse(bytes);
  assert.equal(measurement.host, host);
  assert.equal(measurement.headline.packages, 97);
  assert.equal(measurement.headline.exports, 721);
  const domains = Object.fromEntries(["callbacks", "reads", "returns", "creates"].map(domain =>
    [domain, { exports: 0, packages: new Set(), soleOpenDomain: 0 }]));
  const families = new Map(), domainCombinations = {};
  let cleanExports = 0, exports = 0, unclassifiedExports = 0;
  const seen = new Set(), completePackages = [];
  for (const row of measurement.packages) {
    if (row.total > 0 && row.exports.every(item => item.bucket === "clean")) completePackages.push(row.package);
    for (const item of row.exports) {
      const identity = JSON.stringify([row.package, item.entrypoint, item.export]);
      assert(!seen.has(identity), `Duplicate export ${identity}`);
      seen.add(identity);
      exports++;
      if (item.bucket === "clean") { cleanExports++; continue; }
      const open = new Set(item.causes.map(cause => cause.domain));
      const combination = [...open].map(domain => domain ?? "unclassified").sort().join("+") || "unclassified";
      domainCombinations[combination] = (domainCombinations[combination] ?? 0) + 1;
      for (const family of new Set(item.causes.map(cause => `${cause.class}: ${cause.key}`))) {
        const counts = families.get(family) ?? { exports: 0, packages: new Set() };
        counts.exports++;
        counts.packages.add(row.package);
        families.set(family, counts);
      }
      // Artifact/graph failures have no semantic-domain classification. Keep
      // them visible; never count one as a domain-only unlock candidate.
      if (open.has(null) || open.size === 0) unclassifiedExports++;
      for (const domain of open) {
        if (domain === null) continue;
        assert(domains[domain], `Unknown domain ${domain}`);
        domains[domain].exports++;
        domains[domain].packages.add(row.package);
        if (open.size === 1) domains[domain].soleOpenDomain++;
      }
    }
  }
  assert.equal(exports, measurement.headline.exports);
  assert.equal(cleanExports, measurement.headline.cleanExports);
  results.push({ host, input, sha256: createHash("sha256").update(bytes).digest("hex"), exports, cleanExports,
    completePackages, unclassifiedExports, domainCombinations,
    causeFamilies: [...families].map(([family, counts]) => ({ family, exports: counts.exports, packages: counts.packages.size }))
      .sort((a, b) => b.exports - a.exports || a.family.localeCompare(b.family)),
    domains: Object.fromEntries(Object.entries(domains).map(([domain, counts]) =>
      [domain, { ...counts, packages: counts.packages.size }])) });
}
const document = { authority: false, kind: "retained-domain-inventory", results };
writeFileSync(output, JSON.stringify(document, null, 2) + "\n");
console.log(JSON.stringify(document));
