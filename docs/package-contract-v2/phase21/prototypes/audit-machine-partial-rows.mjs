import { readFileSync, existsSync, writeFileSync } from 'node:fs';
import { join, dirname, relative } from 'node:path';
import { createHash } from 'node:crypto';
import assert from 'node:assert/strict';
import { finiteEntrypoints, finiteConditionPartitions, finiteArtifactCandidates, artifactCaseDisposition } from '../../../../packages/cli/scripts/generate-package-contract.mjs';
import { selectPackageExportTarget } from '../../../../packages/cli/scripts/artifact-resolution.mjs';
import { isCompleteCoverage } from '../../../../scripts/ecosystem-benchmark/lib/certified-coverage.mjs';

// Audit only: no certificates are issued and no coverage denominator changes.
const baseline = 'rust/target/ecosystem-investigations/2026-09-09-machine-full.json';
const read = p => JSON.parse(readFileSync(p));
const hash = p => 'sha256:' + createHash('sha256').update(readFileSync(p)).digest('hex');
const scoped = new Map();
const scopedProbes = new Set(['@tanstack/charts', '@tanstack/devtools-a11y', '@tanstack/devtools-utils']);

function inventory(root, row) {
  const pointerPath = join(root, 'accepted-contract-case-set.json');
  const references = [];
  let catalogs;
  if (existsSync(pointerPath)) {
    const pointer = read(pointerPath), document = join(root, pointer.document);
    assert.equal(hash(document), pointer.documentDigest);
    references.push({ path: pointerPath, digest: hash(pointerPath) }, { path: document, digest: hash(document) });
    catalogs = read(document).cases.map(c => ({ path: join(dirname(document), c.catalog), digest: c.catalogDigest }));
  } else catalogs = [{ path: join(root, 'accepted-contracts.json') }];
  const cases = [];
  let packageRoot;
  for (const catalog of catalogs) {
    if (catalog.digest) assert.equal(hash(catalog.path), catalog.digest);
    references.push({ path: catalog.path, digest: hash(catalog.path) });
    for (const entry of read(catalog.path).contracts) {
      const doc = join(dirname(catalog.path), entry.document), receipt = join(dirname(catalog.path), entry.receipt);
      assert.equal(hash(doc), entry.documentDigest); assert.equal(hash(receipt), entry.receiptDigest);
      const main = read(doc);
      if (main.package.name !== row.package || main.package.version !== row.version) continue;
      const installedRoot = entry.import.packageRoot;
      assert.equal(hash(join(installedRoot, 'package.json')), 'sha256:' + main.package.manifest.sha256);
      packageRoot ??= installedRoot;
      for (const [entrypoint, value] of Object.entries(main.entrypoints)) for (const artifactCase of value.cases) {
        cases.push({ entrypoint, artifactCase, document: doc, documentDigest: entry.documentDigest, receiptDigest: entry.receiptDigest });
      }
    }
  }
  assert.ok(packageRoot && cases.length, row.probeId);
  return { packageRoot, cases, references };
}

const report = read(baseline), rows = [];
for (const row of report.results) {
  const coverage = row.certificationAttempt?.coverage;
  if (row.certificationAttempt?.status !== 'certified' || !coverage || isCompleteCoverage(coverage)) continue;
  const oldRoot = join(row.retainedArtifacts.outputDir, row.probeId.replaceAll('/', '__').replaceAll('|', '--') + '.json.accepted-catalog');
  const before = inventory(oldRoot, row), overlay = scoped.get(row.probeId);
  assert.equal(new Set(before.cases.map(c => c.entrypoint)).size, coverage.certifiedEntrypoints);
  const current = overlay ? inventory(overlay.catalog, row) : before;
  if (overlay) {
    assert.deepEqual(read(overlay.audit).ordinaryAnalysis, { receiptAuthenticated: true, exactCaseSelected: true });
    assert.ok(before.cases.every(c => current.cases.some(n => n.documentDigest === c.documentDigest)));
  }
  const manifestPath = join(current.packageRoot, 'package.json'), manifest = read(manifestPath);
  const auditPath = join(row.retainedArtifacts.outputDir, row.probeId.replaceAll('/', '__').replaceAll('|', '--') + '.json.certification-audit.json');
  const certificationRefusals = [];
  function visit(value) {
    if (!value || typeof value !== 'object') return;
    if (typeof value.entrypoint === 'string' && typeof value.reason === 'string') certificationRefusals.push(value);
    for (const child of Object.values(value)) visit(child);
  }
  if (existsSync(auditPath)) visit(read(auditPath).graphPreparation);
  const census = finiteEntrypoints(manifest, [], current.packageRoot);
  const candidates = finiteArtifactCandidates(manifest, census.entrypoints, finiteConditionPartitions(manifest, []), current.packageRoot);
  const accepted = [...new Set(current.cases.map(c => c.entrypoint))].sort();
  const examined = candidates.map(c => {
    const item = { ...c };
    for (const axis of ['runtime', 'declarations']) {
      try {
        const selected = selectPackageExportTarget({ packageRoot: current.packageRoot, manifest, ...c, axis });
        item[axis] = { path: './' + relative(current.packageRoot, selected.path), exists: selected.exists, branch: selected.trace.branch, digest: selected.exists ? hash(selected.path) : null };
      } catch (e) { item[axis] = { error: e.message }; }
    }
    // Declaration files are not executable even when their import/export
    // syntax would emit if it appeared in an ordinary .ts source file.
    const declarationTarget = item.runtime.path && /\.d\.(?:ts|mts|cts)$/.test(item.runtime.path);
    const exportValue = manifest.exports?.[c.entrypoint];
    const typesOnlyExport = item.runtime.error && exportValue &&
      Object.keys(exportValue).length === 1 && typeof exportValue.types === 'string' &&
      /\.d\.(?:ts|mts|cts)$/.test(exportValue.types) && item.declarations.exists;
    const disposition = declarationTarget || typesOnlyExport
      ? { class: 'declaration-only-target', reason: typesOnlyExport ? 'manifest explicitly exposes only a types condition selecting a retained declaration file' : 'selected target is a declaration file', nativeReplayPerformed: false }
      : artifactCaseDisposition({ packageRoot: current.packageRoot, manifest, ...c });
    item.disposition = disposition;
    item.accepted = current.cases.some(a => a.entrypoint === c.entrypoint && a.artifactCase.artifact.path === item.runtime.path && 'sha256:' + a.artifactCase.artifact.sha256 === item.runtime.digest && a.artifactCase.declarations.path === item.declarations.path && 'sha256:' + a.artifactCase.declarations.sha256 === item.declarations.digest && a.artifactCase.resolution.runtimeBranch === item.runtime.branch && a.artifactCase.resolution.typesBranch === item.declarations.branch);
    item.recordedRefusals = (row.artifactCaseRefusals ?? []).filter(r => r.entrypoint === c.entrypoint);
    item.recordedCertificationRefusals = certificationRefusals.filter(r => r.entrypoint === c.entrypoint);
    return item;
  });
  const missing = census.entrypoints.filter(e => !accepted.includes(e)).map(entrypoint => {
    const selections = examined.filter(c => c.entrypoint === entrypoint);
    const executable = selections.filter(c => c.runtime.exists && !c.disposition);
    const unresolved = selections.filter(c => !c.runtime.exists && !c.disposition);
    return { entrypoint, kind: executable.length ? 'executable' : unresolved.length ? 'unresolved-selection' : 'inapplicable', selections };
  });
  const category = scopedProbes.has(row.package) ? 'scoped-probe' : row.declaredWildcard ? 'wildcard' : missing.some(m => m.kind === 'executable') ? 'executable-gap' : missing.some(m => m.kind === 'unresolved-selection') ? 'unresolved-selection' : 'inapplicable-only';
  rows.push({ probeId: row.probeId, package: row.package, version: row.version, category, baselineCoverage: coverage, retainedArtifacts: row.retainedArtifacts, overlay: overlay ?? null, manifest: { path: manifestPath, digest: hash(manifestPath), exports: manifest.exports }, census, beforeCases: before.cases, currentCases: current.cases, publicationReferences: current.references, acceptedEntrypoints: accepted, missing, uncoveredSelectionsOnAcceptedEntrypoints: examined.filter(c => accepted.includes(c.entrypoint) && !c.accepted && !c.disposition), legacyCompleteRowCeiling: category === 'executable-gap' && !missing.some(m => m.kind !== 'executable') ? 1 : 0 });
}
assert.equal(rows.length, 64);
const counts = Object.fromEntries([...new Set(rows.map(r => r.category))].sort().map(k => [k, rows.filter(r => r.category === k).length]));
const output = { format: 'partial-row-audit', authoritativeCoverageMeasurement: false, baseline: { path: baseline, digest: hash(baseline), finishedAt: report.finishedAt }, method: 'Published-pointer catalogs and digest-checked installed manifests; finite import-condition discovery from retained installed files. No native inapplicability replay or new consumer certification performed by this audit. Scoped overlays require their existing ordinary-consumer audit. Conditional gaps remain separate from missing-entrypoint gaps.', counts, rows };
writeFileSync('docs/package-contract-v2/phase21/2026-09-09-machine-partial-row-audit.json', JSON.stringify(output, null, 2) + '\n');
console.log(JSON.stringify(counts));
for (const r of rows) console.log(r.category, r.probeId, 'missing', r.missing.map(m => `${m.entrypoint}:${m.kind}`).join(','), 'conditional', r.uncoveredSelectionsOnAcceptedEntrypoints.length, 'ceiling', r.legacyCompleteRowCeiling);
