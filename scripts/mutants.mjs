#!/usr/bin/env node
// Seeded recall probe against real rc.13 projects. Never edits the corpus.
// node scripts/mutants.mjs --oracle <runtime-oracle.json> --out <fresh-directory>
//   --apps-root <corpus/apps> --checker <binary> --typefacts <absolute-binary>
//   [--scratch /tmp/sc-scratch.noindex/mutants] [--limit 5] [--seed 20261019]
//   [--apps app,...] [--typescript <installed typescript directory>]
// First: --write-patterns <fresh-directory>, then run runtime-oracle.mjs on it
// with --install pointing to an APFS-cloned rc.13 app, using /private/tmp on macOS.
// Hold the shared lock for the entire sweep (avoids contention between passes):
// sc-heavy node scripts/mutants.mjs ... --heavy /usr/bin/env
// Resume an interrupted sweep with the same options and --resume true.
// APFS clones, one batch per project, disjoint function bodies; baseline and mutant
// use identical project settings. TypeScript diagnostics are differenced, not
// ignored: any new error excludes that project batch before native analysis.
// Runtime confirms templates only; corpus execution/reachability is not proved.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, realpathSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, isAbsolute, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const script = fileURLToPath(import.meta.url);
const repo = resolve(dirname(script), "..");
const patterns = {
  P1: { rule: "strict-read-untracked", code: "STRICT_READ_UNTRACKED" },
  P2: { rule: "missing-owner", code: "NO_OWNER_CLEANUP" },
  P3: { rule: "reactive-write-in-owned-scope", code: "REACTIVE_WRITE_IN_OWNED_SCOPE" },
};
const sha = value => createHash("sha256").update(value).digest("hex");
const json = path => JSON.parse(readFileSync(path, "utf8"));
const save = (path, value) => writeFileSync(path, JSON.stringify(value, null, 2) + "\n");
const scratchPath = path => ["/tmp/sc-scratch.noindex/mutants", "/private/tmp/sc-scratch.noindex/mutants"]
  .some(root => path === root || path.startsWith(root + "/"));
function command(bin, args, options = {}) {
  const started = Date.now();
  const result = spawnSync(bin, args, { encoding: "utf8", maxBuffer: 128 * 1024 * 1024,
    timeout: 900_000, ...options });
  assert(!result.error && !result.signal, `${bin}: ${result.error ?? result.signal}`);
  return { status: result.status, stdout: result.stdout, stderr: result.stderr, wallMs: Date.now() - started };
}
function originalOffset(offset, edits) {
  let delta = 0;
  for (const edit of edits) {
    const start = edit.pos + delta;
    if (offset < start) break;
    if (offset < start + edit.text.length) return null;
    delta += edit.text.length;
  }
  return offset - delta;
}
function classify(findings, rule) {
  const expected = findings.filter(f => f.rule === rule);
  return expected.some(f => f.kind === "violation") ? "proven"
    : expected.some(f => f.kind === "uncertifiable") ? "uncertifiable"
    : findings.length ? "other" : "missed";
}
function mutationBody(pattern, id) {
  return pattern === "P1" ? `const [${id}Read] = __scMutantSignal(0);\nconsole.log(${id}Read());`
    : pattern === "P2" ? "__scMutantCleanup(() => {});"
    : `const [, ${id}Set] = __scMutantSignal(0);\nconst ${id}Memo = __scMutantMemo(() => { ${id}Set(1); return 0; });\nconsole.log(${id}Memo());`;
}
function patternSource(pattern) {
  const imports = pattern === "P2" ? "onCleanup as __scMutantCleanup"
    : pattern === "P1" ? "createSignal as __scMutantSignal"
      : "createSignal as __scMutantSignal, createMemo as __scMutantMemo";
  const body = mutationBody(pattern, "__scMutant0");
  return `import { ${imports} } from "solid-js";\nexport function App() {\n${pattern === "P2"
    ? `return <button onClick={() => {\n${body}\n}}>cleanup</button>;`
    : `${body}\nreturn <div>probe</div>;`}\n}\n`;
}
if (process.argv[2] === "--write-patterns") {
  const root = resolve(process.argv[3]);
  assert(scratchPath(root), "patterns must be under /tmp/sc-scratch.noindex/mutants");
  assert(!existsSync(root), "use a fresh patterns directory");
  for (const pattern of Object.keys(patterns)) {
    const dir = join(root, pattern), source = patternSource(pattern);
    mkdirSync(dir, { recursive: true });
    writeFileSync(join(dir, "App.tsx"), source);
    save(join(dir, "oracle.json"), { note: `seeded-mutant-template-sha256:${sha(source)}` });
  }
  console.log(root);
  process.exit(0);
}
if (process.argv[2] === "--self-check") {
  assert.equal(originalOffset(12, [{ pos: 5, text: "abc" }]), 9);
  assert.equal(originalOffset(6, [{ pos: 5, text: "abc" }]), null);
  assert.equal(classify([{ rule: "x", kind: "uncertifiable" }], "x"), "uncertifiable");
  assert.equal(classify([{ rule: "y", kind: "violation" }], "x"), "other");
  assert.equal(classify([{ rule: "x", kind: "violation" }], "x"), "proven");
  assert.equal(classify([], "x"), "missed");
  assert.equal(Buffer.byteLength("é"), 2);
  console.log("mutants self-check passed");
  process.exit(0);
}

// A fresh process per project bounds TypeScript's program memory.
if (process.argv[2] === "--prepare") {
  const spec = json(process.argv[3]);
  assert(scratchPath(realpathSync(spec.clone)), "prepare only writes mutant clones");
  const ts = createRequire(import.meta.url)(spec.typescript);
  const project = join(spec.clone, spec.project);
  const load = () => {
    const config = ts.readConfigFile(project, ts.sys.readFile);
    assert(!config.error, "unreadable tsconfig");
    const parsed = ts.parseJsonConfigFileContent(config.config, ts.sys, dirname(project), {}, project);
    assert(!parsed.errors.length, ts.formatDiagnosticsWithColorAndContext(parsed.errors, {
      getCanonicalFileName: p => p, getCurrentDirectory: () => spec.clone, getNewLine: () => "\n" }));
    return ts.createProgram({ rootNames: parsed.fileNames, options: { ...parsed.options, noEmit: true },
      projectReferences: parsed.projectReferences });
  };
  const edits = new Map();
  const bomOffsets = new Map();
  const bomOffset = file => {
    if (!bomOffsets.has(file)) bomOffsets.set(file, readFileSync(file, "utf8").startsWith("\uFEFF") ? 1 : 0);
    return bomOffsets.get(file);
  };
  const diagnostics = program => ts.getPreEmitDiagnostics(program)
    .filter(d => d.category === ts.DiagnosticCategory.Error)
    .map(d => {
      const file = d.file ? relative(spec.clone, d.file.fileName) : null;
      const start = d.start === undefined ? null : originalOffset(d.start + bomOffset(d.file.fileName), edits.get(file) ?? []);
      return { file, start, code: d.code, message: ts.flattenDiagnosticMessageText(d.messageText, "\n") };
    });
  const before = load();
  const baselineErrors = diagnostics(before);
  const candidates = [];
  const functions = node => ts.isFunctionDeclaration(node) || ts.isFunctionExpression(node) || ts.isArrowFunction(node);
  const jsx = node => ts.isJsxElement(node) || ts.isJsxSelfClosingElement(node) || ts.isJsxFragment(node);
  function ownJSX(node) {
    if (jsx(node)) return true;
    if (functions(node)) return false;
    return ts.forEachChild(node, ownJSX) ?? false;
  }
  function insertion(body, sf) {
    let pos = body.getStart(sf) + 1;
    for (const statement of body.statements) {
      if (ts.isExpressionStatement(statement) && ts.isStringLiteral(statement.expression)) pos = statement.end;
      else break;
    }
    return pos;
  }
  function packageAt(file, options) {
    const found = ts.resolveModuleName("solid-js", file, options, ts.sys).resolvedModule;
    assert(found, `${file}: solid-js does not resolve`);
    let dir = dirname(found.resolvedFileName);
    while (dirname(dir) !== dir) {
      const path = join(dir, "package.json");
      if (existsSync(path) && json(path).name === "solid-js") {
        assert.equal(json(path).version, "2.0.0-rc.13", path);
        return { version: json(path).version, packageSha256: sha(readFileSync(path)),
          typesSha256: sha(readFileSync(found.resolvedFileName)), resolved: realpathSync(found.resolvedFileName) };
      }
      dir = dirname(dir);
    }
    throw new Error(`no solid-js package for ${file}`);
  }
  for (const sf of before.getSourceFiles()) {
    const file = relative(spec.clone, sf.fileName);
    if (sf.isDeclarationFile || file.startsWith("..") || file.split("/").includes("node_modules") || !/\.[jt]sx$/.test(file)) continue;
    const rawSource = readFileSync(sf.fileName, "utf8"), bom = bomOffset(sf.fileName);
    assert.equal(rawSource.slice(bom), sf.text, `AST/source text differs: ${file}`);
    function visit(node) {
      if (functions(node) && node.body && ts.isBlock(node.body) && !node.asteriskToken
        && !node.modifiers?.some(m => m.kind === ts.SyntaxKind.AsyncKeyword)) {
        const name = node.name?.text ?? (ts.isVariableDeclaration(node.parent) ? node.parent.name.getText(sf) : "default");
        let parent = node.parent;
        while (parent && !functions(parent)) parent = parent.parent;
        if (!parent && (/^[A-Z]/.test(name) || node.modifiers?.some(m => m.kind === ts.SyntaxKind.DefaultKeyword)
          || (ts.isExportAssignment(node.parent) && !node.parent.isExportEquals))
          && node.body.statements.some(s => ts.isReturnStatement(s) && s.expression && ownJSX(s.expression))) {
          candidates.push({ file, pos: insertion(node.body, sf) + bom, shape: `component ${ts.SyntaxKind[node.kind]} ${name}`,
            family: "component", owner: `${file}:${node.getStart(sf)}`, line: sf.getLineAndCharacterOfPosition(node.getStart(sf)).line + 1 });
        }
      }
      if (ts.isJsxAttribute(node) && /^on[A-Z]/.test(node.name.text ?? "")) {
        const tag = node.parent.parent.tagName.getText(sf);
        const fn = node.initializer && ts.isJsxExpression(node.initializer) ? node.initializer.expression : null;
        if (/^[a-z][a-z0-9-]*$/.test(tag) && fn && (ts.isArrowFunction(fn) || ts.isFunctionExpression(fn))
          && ts.isBlock(fn.body) && !fn.asteriskToken && !fn.modifiers?.some(m => m.kind === ts.SyntaxKind.AsyncKeyword)) {
          // Do not combine a handler mutation with a component mutation in its owner.
          let owner = null, ancestor = node.parent;
          while (ancestor) {
            if (functions(ancestor)) owner = ancestor;
            ancestor = ancestor.parent;
          }
          candidates.push({ file, pos: insertion(fn.body, sf) + bom, shape: `${tag} ${node.name.text} inline ${ts.SyntaxKind[fn.kind]}`,
            family: "handler", owner: `${file}:${owner?.getStart(sf) ?? fn.getStart(sf)}`,
            line: sf.getLineAndCharacterOfPosition(fn.getStart(sf)).line + 1 });
        }
      }
      ts.forEachChild(node, visit);
    }
    visit(sf);
  }
  const ranked = candidates.sort((a, b) => sha(`${spec.seed}:${spec.app}:${a.file}:${a.pos}`)
    .localeCompare(sha(`${spec.seed}:${spec.app}:${b.file}:${b.pos}`)));
  const sites = [], used = new Set();
  // Round-robin component patterns so a small app can contribute to both.
  for (let i = 0; i < spec.limit; i++) {
    for (const pattern of ["P1", "P3", "P2"]) {
      if (sites.filter(s => s.pattern === pattern).length >= (spec.remaining?.[pattern] ?? spec.limit)) continue;
      const candidate = ranked.find(c => c.family === (pattern === "P2" ? "handler" : "component") && !used.has(c.owner));
      if (!candidate) continue;
      used.add(candidate.owner);
      const id = `__scMutant${sites.length}`;
      const body = mutationBody(pattern, id);
      sites.push({ ...candidate, id, pattern, text: `\n/* ${id} ${pattern} */\n${body}\n`, ...patterns[pattern] });
    }
  }
  const resolution = [];
  for (const file of new Set(sites.map(s => s.file))) {
    const path = join(spec.clone, file), originalPath = join(spec.original, file);
    assert(realpathSync(path).startsWith(spec.clone + "/"), `refuse writing source symlink outside clone: ${path}`);
    const sf = before.getSourceFile(path);
    const source = readFileSync(path, "utf8");
    assert.equal(source, readFileSync(originalPath, "utf8"), `clone input differs from corpus: ${file}`);
    assert(!source.includes("__scMutant"), `reserved mutation name already used: ${file}`);
    const cloned = packageAt(path, before.getCompilerOptions());
    const original = packageAt(originalPath, before.getCompilerOptions());
    assert.equal(original.packageSha256, cloned.packageSha256);
    assert.equal(original.typesSha256, cloned.typesSha256);
    resolution.push({ file, originalSourceSha256: sha(source), original, cloned });
    const list = sites.filter(s => s.file === file).map(s => ({ pos: s.pos, text: s.text, site: s }));
    // Keep directive prologues first; bind only the APIs used in this file.
    let importPos = bomOffset(path);
    for (const statement of sf.statements) {
      if (ts.isExpressionStatement(statement) && ts.isStringLiteral(statement.expression)) importPos = statement.end + bomOffset(path);
      else break;
    }
    const imports = [];
    if (sites.some(s => s.file === file && s.pattern !== "P2")) imports.push("createSignal as __scMutantSignal");
    if (sites.some(s => s.file === file && s.pattern === "P2")) imports.push("onCleanup as __scMutantCleanup");
    if (sites.some(s => s.file === file && s.pattern === "P3")) imports.push("createMemo as __scMutantMemo");
    list.push({ pos: importPos, text: `\nimport { ${imports.join(", ")} } from "solid-js";\n` });
    list.sort((a, b) => a.pos - b.pos);
    let result = "", cursor = 0;
    for (const edit of list) {
      result += source.slice(cursor, edit.pos);
      if (edit.site) edit.site.start = result.length;
      result += edit.text;
      if (edit.site) edit.site.end = result.length;
      cursor = edit.pos;
    }
    result += source.slice(cursor);
    edits.set(file, list.map(({ pos, text }) => ({ pos, text })));
    // Generated mutant sources are artifacts; the harness source itself uses apply_patch.
    writeFileSync(path, result);
    for (const site of sites.filter(s => s.file === file)) {
      site.startByte = Buffer.byteLength(result.slice(0, site.start));
      site.endByte = Buffer.byteLength(result.slice(0, site.end));
      site.mutantLine = result.slice(0, site.start).split("\n").length;
    }
  }
  const afterErrors = diagnostics(load());
  const counts = new Map();
  for (const error of baselineErrors) {
    const key = JSON.stringify(error);
    counts.set(key, (counts.get(key) ?? 0) + 1);
  }
  const introduced = afterErrors.filter(error => {
    const key = JSON.stringify(error), count = counts.get(key) ?? 0;
    if (count) { counts.set(key, count - 1); return false; }
    return true;
  });
  save(spec.prepared, { sites, edits: Object.fromEntries(edits), resolution, candidates: candidates.length,
    typescriptVersion: ts.version, baselineErrorCount: baselineErrors.length, mutantErrorCount: afterErrors.length, baselineErrors, afterErrors, introduced });
  process.exit(0);
}

const options = { "apps-root": "/Users/thomas/Documents/Github/solid-checker-rc13-corpus/apps",
  checker: "/tmp/sc-scratch.noindex/agents/checker-6a4940bc9",
  typefacts: "/Users/thomas/Documents/Github/solid-checker/bin/solid-typefacts",
  manifest: join(repo, "scripts/corpus/rc13-manifest.json"), scratch: "/tmp/sc-scratch.noindex/mutants",
  heavy: "/Users/thomas/.local/bin/sc-heavy", limit: "5", seed: "20261019", apps: "", typescript: null, oracle: null, out: null, resume: "false" };
for (let i = 2; i < process.argv.length; i += 2) {
  const key = process.argv[i].replace(/^--/, "");
  assert(key in options && process.argv[i + 1] !== undefined, `unknown/incomplete ${process.argv[i]}`);
  options[key] = process.argv[i + 1];
}
assert(options.out && options.oracle, "--out and --oracle are required");
assert(Number.isInteger(Number(options.limit)) && Number(options.limit) > 0, "positive integer --limit required");
assert(isAbsolute(options.typefacts), "producer must be absolute");
const output = resolve(options.out);
assert(!output.startsWith(realpathSync(options["apps-root"]) + "/"), "output must not be inside the corpus");
assert(!existsSync(output) || options.resume === "true", "use a fresh output directory, or --resume true");
mkdirSync(output, { recursive: true });
const scratch = resolve(options.scratch);
assert(scratchPath(scratch), "scratch must be under /tmp/sc-scratch.noindex/mutants");
mkdirSync(scratch, { recursive: true });
const oracle = json(options.oracle);
assert.equal(oracle.solid, "2.0.0-rc.13", "calibrate against rc.13");
for (const [id, pattern] of Object.entries(patterns)) {
  const row = oracle.results.find(r => r.id === id);
  assert(row && row.typingErrors === 0 && row.diagnostics.some(d => d.code === pattern.code), `runtime template ${id} unconfirmed`);
  assert.equal(row.note, `seeded-mutant-template-sha256:${sha(patternSource(id))}`, `runtime template ${id} changed; reconfirm`);
}
save(join(output, "calibration.json"), oracle);
const manifest = json(options.manifest);
assert.deepEqual(manifest.configuration, { runtimeTarget: "browser", runtimeBuild: "development", rendering: "csr" });
const checkpoint = options.resume === "true" && existsSync(join(output, "progress.json")) ? json(join(output, "progress.json")) : {};
const started = checkpoint.startedAt ?? Date.now(), results = checkpoint.results ?? [], sites = checkpoint.sites ?? [], blockers = checkpoint.blockers ?? [];
const identities = { checkerSha256: sha(readFileSync(options.checker)), producerSha256: sha(readFileSync(options.typefacts)) };
const env = { ...process.env, SOLID_CHECKER_DAEMON: "0", SOLID_TYPEFACTS_BIN: resolve(options.typefacts) };
const typescript = resolve(options.typescript ?? join(options["apps-root"], "helge-dev/node_modules/typescript"));
const rows = manifest.projects.filter(row => !options.apps || options.apps.split(",").includes(row.app));
const identityPath = join(output, "identity.json");
const runIdentity = { ...identities, seed: options.seed, limit: options.limit, manifestSha256: sha(readFileSync(options.manifest)),
  appsRoot: realpathSync(options["apps-root"]), typescript, apps: options.apps, harnessSha256: sha(readFileSync(script)) };
if (existsSync(identityPath)) assert.deepEqual(json(identityPath), runIdentity, "resume inputs changed");
else {
  assert(!results.length, "cannot resume a run without an identity checkpoint");
  save(identityPath, runIdentity);
}
const runId = `${options.seed}-${Date.now()}`;
for (const [app, appRows] of Map.groupBy(rows, r => r.app)) {
  const source = realpathSync(join(options["apps-root"], appRows[0].root));
  const remaining = Object.fromEntries(Object.keys(patterns).map(p => [p, Number(options.limit)]));
  for (const site of sites.filter(s => s.app === app)) remaining[site.pattern]--;
  for (let index = 0; index < appRows.length; index++) {
    if (Object.values(remaining).every(n => n === 0)) break;
    const row = appRows[index], prefix = `${app}-${index}`;
    if (results.some(r => r.app === app && r.project === row.project)) continue;
    try {
    const previousSpecPath = join(output, `${prefix}-spec.json`);
    const previousSpec = options.resume === "true" && existsSync(previousSpecPath) ? json(previousSpecPath) : null;
    const resumeProject = previousSpec && existsSync(previousSpec.prepared) && existsSync(join(output, `${prefix}-baseline.json`));
    const target = resumeProject ? previousSpec.clone : join(scratch, `${runId}-${prefix}`);
    let copy = { wallMs: 0 };
    if (!resumeProject) {
    assert(!existsSync(target));
    console.error(`clone ${prefix}`);
    copy = command("cp", ["-cR", source, target]);
    assert.equal(copy.status, 0, copy.stderr);
    } else console.error(`resume ${prefix}`);
    const clone = realpathSync(target);
    const runChecker = phase => {
      const checkpointPath = join(output, `${prefix}-${phase}.json`);
      if (resumeProject && existsSync(checkpointPath)) {
        const cached = json(checkpointPath);
        if (cached.payload) return cached;
      }
      const run = command(resolve(options.heavy), ["/usr/bin/time", "-p", resolve(options.checker), "--format", "json", "--project", join(clone, row.project),
        "--runtime-target", "browser", "--runtime-build", "development", "--rendering", "csr"], { cwd: clone, env });
      const timing = run.stderr.match(/(?:^|\n)real\s+(\d+(?:\.\d+)?)/);
      assert(timing, `missing checker wall time: ${run.stderr}`);
      run.wrapperWallMs = run.wallMs;
      run.wallMs = Math.round(Number(timing[1]) * 1000);
      assert.equal(sha(readFileSync(options.checker)), identities.checkerSha256, "checker changed during run");
      assert.equal(sha(readFileSync(options.typefacts)), identities.producerSha256, "producer changed during run");
      // Preserve evidence even when the checker fails before emitting JSON.
      save(checkpointPath, run);
      let payload;
      try { payload = JSON.parse(run.stdout); }
      catch (cause) { throw new Error(`${prefix} ${phase}: invalid JSON, exit ${run.status}: ${run.stderr.slice(-4000)}`, { cause }); }
      assert(Array.isArray(payload.findings) && [0, 1].includes(run.status), `${prefix} ${phase}: invalid analysis ${run.status} ${run.stderr}`);
      save(join(output, `${prefix}-${phase}.json`), { ...run, stdout: undefined, payload });
      return { ...run, payload };
    };
    const baseline = runChecker("baseline");
    const spec = resumeProject ? previousSpec : { app, project: row.project, clone, original: source, seed: options.seed, limit: Number(options.limit),
      remaining, typescript, prepared: join(output, `${prefix}-sites.json`) };
    const specPath = join(output, `${prefix}-spec.json`);
    save(specPath, spec);
    const preparation = resumeProject ? { status: 0, wallMs: 0 } : command(process.execPath, [script, "--prepare", specPath]);
    assert.equal(preparation.status, 0, `${prefix}: ${preparation.stderr}`);
    const prepared = json(spec.prepared);
    const analyzedMutant = prepared.sites.length > 0 && prepared.introduced.length === 0;
    const mutant = analyzedMutant ? runChecker("mutant") : baseline;
    for (const resolution of prepared.resolution) assert.equal(sha(readFileSync(join(source, resolution.file))),
      resolution.originalSourceSha256, `corpus input changed during run: ${resolution.file}`);
    const baselineKeys = new Set(baseline.payload.findings.map(f => {
      const loc = f.primaryLocation;
      return JSON.stringify([relative(clone, loc.path), loc.startByte, f.rule, f.kind]);
    }));
    const added = analyzedMutant ? mutant.payload.findings.filter(f => {
      const loc = f.primaryLocation, file = relative(clone, loc.path);
      if (!prepared.edits[file]) return !baselineKeys.has(JSON.stringify([file, loc.startByte, f.rule, f.kind]));
      const sourceText = readFileSync(join(clone, file), "utf8");
      const utf16 = Buffer.from(sourceText).subarray(0, loc.startByte).toString("utf8").length;
      const offset = originalOffset(utf16, prepared.edits[file] ?? []);
      if (offset === null) return true;
      // Map back to original byte offsets, including non-ASCII source text.
      const original = readFileSync(join(source, file), "utf8");
      return !baselineKeys.has(JSON.stringify([file, Buffer.byteLength(original.slice(0, offset)), f.rule, f.kind]));
    }) : [];
    for (const site of prepared.sites) {
      const covering = added.filter(f => relative(clone, f.primaryLocation.path) === site.file
        && f.primaryLocation.startByte >= site.startByte && f.primaryLocation.startByte < site.endByte);
      const typing = prepared.introduced;
      const classification = typing.length ? "type-error" : classify(covering, site.rule);
      sites.push({ app, project: row.project, clone, ...site, classification, typing, findings: covering });
    }
    const result = { app, project: row.project, clone, siteCount: prepared.sites.length,
      candidateCount: prepared.candidates,
      typescriptVersion: prepared.typescriptVersion, cloneWallMs: copy.wallMs,
      baselineWallMs: baseline.wallMs, mutantWallMs: analyzedMutant ? mutant.wallMs : 0,
      preparationWallMs: preparation.wallMs, baselineStatus: baseline.payload.status, mutantStatus: mutant.payload.status,
      baselineErrorCount: prepared.baselineErrorCount, mutantErrorCount: prepared.mutantErrorCount, introduced: prepared.introduced };
    results.push(result);
    for (const site of prepared.sites) remaining[site.pattern]--;
    save(join(output, "progress.json"), { startedAt: started, results, sites, blockers });
    console.error(`${app} ${row.project}: ${prepared.sites.length} sites, ${prepared.introduced.length} new typing errors, ${baseline.wallMs + result.mutantWallMs}ms checker`);
    } catch (error) {
      blockers.push({ app, project: row.project, error: String(error.stack ?? error) });
      save(join(output, "progress.json"), { startedAt: started, results, sites, blockers });
      console.error(`${prefix}: BLOCKED ${error.message}`);
    }
  }
}
const totals = Object.fromEntries(Object.keys(patterns).map(pattern => {
  const rows = sites.filter(s => s.pattern === pattern);
  const counts = Object.fromEntries(["proven", "uncertifiable", "missed", "type-error", "other"].map(c => [c, rows.filter(s => s.classification === c).length]));
  const denominator = counts.proven + counts.uncertifiable + counts.missed;
  return [pattern, { sites: rows.length, ...counts, denominator, recall: denominator ? counts.proven / denominator : null,
    conservativeRecall: denominator + counts.other ? counts.proven / (denominator + counts.other) : null,
    uncertifiableShare: denominator ? counts.uncertifiable / denominator : null }];
}));
const groups = [];
for (const site of sites.filter(s => ["uncertifiable", "missed", "other"].includes(s.classification))) {
  const reason = site.classification === "uncertifiable"
    ? site.findings.filter(f => f.rule === site.rule).map(f => f.message).join("; ")
    : site.classification === "other" ? `different rule: ${site.findings.map(f => f.rule).join(", ")}`
      : `no finding at inserted span; ${site.family === "component" ? site.shape.split(" ").slice(0, 2).join(" ") : site.shape}`;
  const normalized = reason.replace(/__scMutant\d+\w*/g, "<mutant>");
  let group = groups.find(g => g.pattern === site.pattern && g.classification === site.classification && g.reason === normalized);
  if (!group) groups.push(group = { pattern: site.pattern, classification: site.classification, reason: normalized, count: 0, examples: [] });
  group.count++;
  if (group.examples.length < 3) group.examples.push({ app: site.app, file: site.file, line: site.mutantLine,
    originalLine: site.line, shape: site.shape, snippet: site.text.trim(), clone: site.clone });
}
groups.sort((a, b) => b.count - a.count);
const report = { seed: options.seed, limitPerPatternPerApp: Number(options.limit),
  patterns: Object.fromEntries(Object.entries(patterns).map(([id, pattern]) => [id,
    { ...pattern, template: patternSource(id), templateSha256: sha(patternSource(id)) }])), totals, groups,
  checker: resolve(options.checker), producer: options.typefacts, ...identities, node: process.version,
  manifestSha256: sha(readFileSync(options.manifest)), harnessSha256: sha(readFileSync(script)),
  typescript, oracle: resolve(options.oracle), configuration: { ...manifest.configuration, daemon: false },
  selectedProjects: rows, skippedAfterAppQuota: rows.filter(row => ![...results, ...blockers]
    .some(r => r.app === row.app && r.project === row.project)),
  wallMs: Date.now() - started, checkerWallMs: results.reduce((n, r) => n + r.baselineWallMs + r.mutantWallMs, 0),
  limitations: ["Templates executed in isolation, corpus components/handlers not executed; reachability is unproved.",
    "Batched mutants use separate function owners; P3 also contains a strict read after the memo write.",
    "Only synchronous block bodies in TSX/JSX; expression bodies, imported handlers and async are unsampled.",
    "Typing delta against each real project excludes batches with new errors; existing errors remain and can affect proof quality.",
    "Other is excluded from the requested denominator; also report conservative recall counting other as a miss.",
    "Miss shape groups describe observed syntax, not a demonstrated root cause."], blockers, results, sites };
save(join(output, "report.json"), report);
const lines = ["# Seeded mutant recall", "", `Seed ${options.seed}; ${results.length} projects; ${(report.wallMs / 1000).toFixed(1)}s elapsed; ${(report.checkerWallMs / 1000).toFixed(1)}s checker.`, "",
  "| Pattern | Sites | Proven | Uncertifiable | Missed | Type error | Other | Recall | Uncertifiable share |", "| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |"];
for (const [id, t] of Object.entries(totals)) lines.push(`| ${id} | ${t.sites} | ${t.proven} | ${t.uncertifiable} | ${t.missed} | ${t["type-error"]} | ${t.other} | ${t.recall === null ? "n/a" : (100 * t.recall).toFixed(1) + "%"} | ${t.uncertifiableShare === null ? "n/a" : (100 * t.uncertifiableShare).toFixed(1) + "%"} |`);
lines.push("", "Recall = proven / (proven + uncertifiable + missed). Other is reported separately.", "", "## Ranked gaps", "");
for (const group of groups) {
  lines.push(`- **${group.pattern} ${group.classification}, ${group.count}**: ${group.reason}`);
  for (const e of group.examples) lines.push(`  - ${e.app} \`${e.file}:${e.line}\` (original function line ${e.originalLine}): \`${e.snippet.replace(/\n/g, " ")}\``);
}
lines.push("", "## Validity limits", "", ...report.limitations.map(s => `- ${s}`), "");
if (blockers.length) lines.push("## Excluded projects", "", ...blockers.map(b => `- ${b.app} ${b.project}: ${b.error.split("\n")[0]}`), "");
writeFileSync(join(output, "summary.md"), lines.join("\n"));
console.log(JSON.stringify({ totals, wallMs: report.wallMs, checkerWallMs: report.checkerWallMs, output }, null, 2));
if (blockers.length) process.exitCode = 1;
