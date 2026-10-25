"use strict";

const { existsSync, readFileSync, readdirSync, readlinkSync } = require("node:fs");
const { dirname, isAbsolute, join, parse, resolve } = require("node:path");
const { spawnSync } = require("node:child_process");
const { createHash } = require("node:crypto");

const packageVersion = require("./package.json").version;
const snapshotCache = new Map();
/**
 * Subject kinds whose `relatedLocations` are further sites of the finding
 * itself: the checker collapses one unavailable-package-behavior obligation
 * per package over the whole project (`projection.rs`,
 * `collapse_unanalyzed_package_defects`). `package-export` is retained for a
 * finding that names one export.
 */
const SITE_SUBJECTS = new Set(["package", "package-export"]);

/**
 * The native analysis's `solid-checker: note:` stderr lines, per snapshot.
 *
 * A note is not a finding: it says something about the run itself -- today,
 * that a package's source could not be analyzed, so its behaviour stays
 * unknown.
 * The standalone CLI prints it to the terminal; the adapter captures the
 * child's stderr, so without this the note died in a pipe ESLint never shows
 * and the user saw uncertified imports with no reason given.
 */
const snapshotNotices = new WeakMap();

const NOTE_PREFIX = "solid-checker: note: ";

function stderrNotices(stderr) {
  return (stderr ?? "")
    .split(/\r?\n/)
    .filter(line => line.startsWith(NOTE_PREFIX))
    .map(line => line.slice(NOTE_PREFIX.length).trim())
    .filter(line => line.length > 0);
}

/** A note as an ESLint message. */
function noticeMessage(notice) {
  return `[solid-checker note] ${notice}`;
}

/**
 * Per-file registry of the diagnostic identities that enabled per-rule rules
 * own during the current lint pass, so `certification` can skip them and
 * report each finding exactly once regardless of config order.
 *
 * ESLint builds every enabled rule's listener map — calling each rule's
 * `create` — before it emits a single traversal event for the file. By the
 * time `certification`'s `Program` listener fires, every enabled per-rule
 * rule has therefore registered, whether its config was listed before or
 * after `recommended`. Each per-rule rule releases its registration in
 * `Program:exit` (enter events always precede exit events), so a later pass
 * over the same file — an autofix iteration, or a persistent server whose
 * config dropped the per-rule rules — starts from a clean registry.
 */
const ownedRules = new Map();

/**
 * Whether a finding is shown to users. Uncertifiable findings are an internal
 * measure -- a user cannot act on "not proven either way" -- except a
 * project-level notice (`subjectKind: "project"`: the installed Solid runtime
 * is refused or its release is unaudited), without which a lint over a
 * project that was never analyzed as stated would look clean. The native
 * `--format default` applies the same rule (`snapshot_emission::user_facing`).
 */
function userFacing(finding) {
  return finding.kind !== "uncertifiable" || finding.subjectKind === "project";
}

/**
 * The registry key of a rule's findings: its name, or `name#code` for a former
 * name that covered one code of a merged rule (ADR 0384), which reports and so
 * owns that code's findings only.
 */
function ownedKey(ruleName, code) {
  return code === undefined ? ruleName : `${ruleName}#${code}`;
}

function registerOwnedRule(filename, ruleName) {
  let owned = ownedRules.get(filename);
  if (!owned) {
    owned = new Set();
    ownedRules.set(filename, owned);
  }
  owned.add(ruleName);
}

function releaseOwnedRule(filename, ruleName) {
  const owned = ownedRules.get(filename);
  if (!owned) return;
  owned.delete(ruleName);
  if (owned.size === 0) ownedRules.delete(filename);
}

function contextFilename(context) {
  return (
    context.physicalFilename ??
    context.filename ??
    context.getPhysicalFilename?.() ??
    context.getFilename?.() ??
    "<input>"
  );
}

function configuration(context) {
  const settings = context.settings?.solidChecker ?? {};
  const options = context.options?.[0] ?? {};
  return { ...settings, ...options };
}

function findProject(start) {
  let directory = resolve(start);
  for (;;) {
    const candidate = join(directory, "tsconfig.json");
    if (existsSync(candidate)) return candidate;
    const parent = dirname(directory);
    if (parent === directory || directory === parse(directory).root) return undefined;
    directory = parent;
  }
}

function configuredProject(context, config) {
  const filename = contextFilename(context);
  const cwd = resolve(config.cwd ?? process.cwd());
  const parserProject = context.languageOptions?.parserOptions?.project;
  const selected = config.project ?? (
    typeof parserProject === "string"
      ? parserProject
      : Array.isArray(parserProject)
        ? parserProject[0]
        : undefined
  );
  if (selected) return isAbsolute(selected) ? selected : resolve(cwd, selected);
  const start = filename === "<input>" ? cwd : dirname(resolve(filename));
  const discovered = findProject(start);
  if (!discovered) {
    throw new Error(
      `solid-checker adapter could not find tsconfig.json from ${start}; ` +
      "set settings.solidChecker.project"
    );
  }
  return discovered;
}

function runtimeConfiguration(config) {
  const runtime = config.runtime;
  if (runtime == null) return null;
  if (typeof runtime !== "object" || Array.isArray(runtime)) {
    throw new Error("settings.solidChecker.runtime must be an object");
  }
  const list = (value, name) => {
    if (value == null) return [];
    if (!Array.isArray(value) || !value.every(item => typeof item === "string" && item.length > 0)) {
      throw new Error(`settings.solidChecker.runtime.${name} must be a non-empty string array`);
    }
    return [...new Set(value)].sort();
  };
  const allowed = (value, name) => {
    if (value == null) return null;
    if (typeof value !== "string" || value.length === 0) {
      throw new Error(`settings.solidChecker.runtime.${name} must be a non-empty string`);
    }
    return value;
  };
  return {
    target: allowed(runtime.target, "target"),
    build: allowed(runtime.build, "build"),
    rendering: allowed(runtime.rendering, "rendering"),
    programBoundary: allowed(runtime.programBoundary, "programBoundary"),
    conditions: list(runtime.conditions, "conditions"),
    frameworkTransforms: list(runtime.frameworkTransforms, "frameworkTransforms")
  };
}

/**
 * A configured path, resolved exactly as `project` and `snapshotPath` are:
 * against `settings.solidChecker.cwd`, else the ESLint process's working
 * directory. Passed to the native checker absolute, because the checker runs
 * in the tsconfig's directory and a daemon may run somewhere else again.
 */
function configuredPath(config, name) {
  const value = config[name];
  if (value == null) return null;
  if (typeof value !== "string" || value.length === 0) {
    throw new Error(`settings.solidChecker.${name} must be a non-empty string`);
  }
  return resolve(config.cwd ?? process.cwd(), value);
}

/**
 * Settings retired with package-contract consumption (ADR 0312). Each is refused rather than ignored, so a
 * configuration that relied on it learns that it no longer applies.
 */
const RETIRED_SETTINGS = ["acceptedContracts", "receiptTrustConfiguration", "dependencySource"];

function refuseRetiredSettings(config) {
  for (const name of RETIRED_SETTINGS) {
    if (config[name] !== undefined) {
      throw new Error(
        `settings.solidChecker.${name} was removed: package contracts are no longer ` +
        "consumed, so a package is analyzed from source or its behaviour stays uncertifiable"
      );
    }
  }
}

// Identity only; native owns recognition and host proof. Inventory/presence is
// recomputed before every lookup, including directory additions and symlinks.
// No config execution, second checker run, parser or adapter host policy.
function inferenceFingerprint(project, runtime) {
  if (runtime?.target || runtime?.conditions?.length || runtime?.frameworkTransforms?.length ||
      runtime?.build || runtime?.rendering || runtime?.programBoundary === "open") return null;
  const directory = dirname(project);
  const configs = ["ts", "js", "mts", "mjs", "cts", "cjs"].map(ext => `vite.config.${ext}`);
  if (!["index.html", ...configs].some(name => existsSync(join(directory, name)))) return null;
  const hash = createHash("sha256");
  hash.update("inferred-host-inputs-v1\0");
  const record = (path, bytes, kind = "file") => {
    hash.update(JSON.stringify([path, kind, bytes.length]));
    hash.update(bytes);
  };
  const file = path => {
    try { record(path, readFileSync(path)); }
    catch (error) { record(path, Buffer.from(error.code ?? "unreadable"), "unreadable"); }
  };
  const visit = path => {
    let entries;
    try { entries = readdirSync(path, { withFileTypes: true }); }
    catch (error) { record(path, Buffer.from(error.code ?? "unreadable"), "unreadable-directory"); return; }
    for (const entry of entries.sort((a, b) => a.name.localeCompare(b.name))) {
      const child = join(path, entry.name);
      if ([".git", "node_modules"].includes(entry.name)) continue;
      if (entry.isSymbolicLink()) {
        try { record(child, Buffer.from(readlinkSync(child)), "symlink"); }
        catch (error) { record(child, Buffer.from(error.code ?? "unreadable"), "unreadable-symlink"); }
      } else if (entry.isDirectory()) {
        visit(child);
      } else if (entry.isFile() && /\.(?:[cm]?[jt]sx?|tsrx|json|html)$/.test(entry.name)) {
        file(child);
      }
    }
  };
  visit(directory);
  for (let ancestor = dirname(directory); ; ancestor = dirname(ancestor)) {
    const path = join(ancestor, "package.json");
    if (existsSync(path)) file(path);
    else record(path, Buffer.alloc(0), "absent");
    if (dirname(ancestor) === ancestor) break;
  }
  for (const name of ["package.json", "dist/esm/index.mjs", "dist/cjs/index.cjs"]) {
    const path = join(directory, "node_modules", "@solidjs", "vite-plugin", name);
    if (existsSync(path)) file(path);
    else record(path, Buffer.alloc(0), "absent");
  }
  return hash.digest("hex");
}

/**
 * The native options that consume the next argument as their value, exactly
 * as `request_from_args` (rust/crates/solid-facts-backend/src/main.rs) reads
 * them, and its value-less flags. A value is never an option: in
 * `--runtime-condition --certify` the second token is a condition (review
 * round 1 of ADR 0297).
 */
const NATIVE_VALUE_OPTIONS = new Set([
  "--project", "-project", "--typefacts", "--dialect", "--conditions", "--preset",
  "--enable-rule", "--format", "--runtime-resolution", "--daemon-runtime",
  "--dependency-source", "--runtime-realm", "--runtime-target", "--runtime-build",
  "--rendering", "--program-boundary", "--runtime-condition", "--runtime-conditions",
  "--framework-transform"
]);
const NATIVE_FLAGS = new Set([
  "--certify", "--internal", "--feedback-facts", "--serve", "--help", "-h", "--no-package-contracts"
]);
/**
 * One analysis per lint pass over a file.
 *
 * A cache across passes would key on settings alone, and nothing in that key
 * moves when a source, a manifest, a config or an installed package does, so
 * a persistent editor session would serve a snapshot of bytes that are gone.
 * Repeat checks are fast because native answers them from its daemon, which
 * validates every input. The pass is ESLint's
 * per-pass file context: `runRules` (lib/linter/linter.js) builds one
 * `FileContext` per pass and every rule's context of that pass is
 * `fileContext.extend(...)`, an object whose prototype it is. It is found as
 * the nearest object on the context's prototype chain owning `sourceCode`.
 * The `SourceCode` object itself is not a pass: `Linter.verify` accepts one
 * and lints it again (review round 1). A context without such an object
 * simply analyzes again.
 *
 * Every adapter rule also drops its pass's entry at `Program:exit`
 * (`endPass`). The traversal fires every `Program` listener of the pass
 * before any `Program:exit`, and the adapter's rules read only in `Program`,
 * so the drop is after every adapter read of the pass; a later read misses
 * and analyzes again rather than reusing.
 */
const passSnapshots = new WeakMap();

function passOf(context) {
  for (let object = context; object != null; object = Object.getPrototypeOf(object)) {
    if (Object.prototype.hasOwnProperty.call(object, "sourceCode")) return object;
  }
  return null;
}

function passCache(context) {
  const pass = passOf(context);
  if (pass == null) return null;
  let cache = passSnapshots.get(pass);
  if (!cache) {
    cache = new Map();
    passSnapshots.set(pass, cache);
  }
  return cache;
}

function endPass(context) {
  const pass = passOf(context);
  if (pass != null) passSnapshots.delete(pass);
}

/**
 * The native executable the launcher would run, resolved once per process
 * through the launcher's own search (override, packaged binding, repository
 * `bin/`), so each lint pass spawns native directly instead of a Node
 * launcher. Resolution failures surface as an analysis failure.
 */
let resolvedNative;
function defaultNative() {
  if (resolvedNative) return resolvedNative;
  const launcher = require("node:url").pathToFileURL(join(__dirname, "bin", "launcher.mjs")).href;
  const result = spawnSync(
    process.execPath,
    [
      "--input-type=module",
      "-e",
      `import { nativeInvocationFor } from ${JSON.stringify(launcher)};\n` +
        `process.stdout.write(JSON.stringify(nativeInvocationFor("solid-checker")));`
    ],
    { encoding: "utf8", env: process.env }
  );
  if (result.status !== 0 || result.error) {
    throw new Error(
      `solid-checker adapter could not resolve the native binary: ${
        result.error?.message ?? result.stderr.trim()
      }`
    );
  }
  const invocation = JSON.parse(result.stdout);
  const env = Object.fromEntries(
    Object.entries(invocation.env).filter(([, value]) => typeof value === "string")
  );
  resolvedNative = { executable: invocation.executable, env };
  return resolvedNative;
}

function loadSnapshot(context) {
  const config = configuration(context);
  if (config.snapshot != null) return config.snapshot;
  if (config.snapshotPath != null) {
    const path = resolve(config.cwd ?? process.cwd(), config.snapshotPath);
    const key = `file:${path}`;
    if (!snapshotCache.has(key)) {
      snapshotCache.set(key, JSON.parse(readFileSync(path, "utf8")));
    }
    return snapshotCache.get(key);
  }

  const project = configuredProject(context, config);
  const custom = Boolean(config.command || process.env.SOLID_CHECKER_BIN);
  const native = custom ? null : defaultNative();
  const command = config.command ?? process.env.SOLID_CHECKER_BIN ?? native.executable;
  const userArgs = custom ? [...(config.commandArgs ?? [])] : [];
  const commandArgs = userArgs;
  refuseRetiredSettings(config);
  const dialect = config.dialect ?? null;
  const presets = [...new Set(Array.isArray(config.preset) ? config.preset : [])].sort();
  const runtime = runtimeConfiguration(config);
  const runtimeResolution = config.runtimeResolution ?? "off";
  if (!["required", "off"].includes(runtimeResolution)) {
    throw new Error("settings.solidChecker.runtimeResolution must be required or off");
  }
  const observesRuntime = runtimeResolution === "required" || commandArgs.includes("--runtime-resolution");
  const inferredHostInputs = inferenceFingerprint(project, runtime);
  const configuredRules = Array.isArray(config.enableRule) ? config.enableRule : [];
  const activeDefaultDisabled = [...(ownedRules.get(contextFilename(context)) ?? [])]
    .map(key => key.split("#")[0])
    .filter(rule => manifestEntriesByRule.get(rule)?.defaultEnabled === false);
  const enableRules = [...new Set([...configuredRules, ...activeDefaultDisabled])].sort();
  const key = JSON.stringify({
    command,
    commandArgs,
    project,
    dialect,
    presets,
    enableRules,
    runtime,
    runtimeResolution,
    inferredHostInputs
  });
  // A snapshot is reused within one file's pass only (`passCache`); every
  // pass asks native again. The adapter does not know the analyzed inputs
  // (sources, manifests, configs, installed packages), so a cross-pass cache
  // keyed on settings served a long-lived ESLint process (an editor server)
  // the first snapshot through every later edit. Native answers a repeat
  // check from its daemon, which validates those inputs, in milliseconds.
  // An observed runtime is never reused, not even within a pass.
  const pass = observesRuntime ? new Map() : passCache(context) ?? new Map();
  const store = (value) => pass.set(key, value);
  if (pass.has(key)) {
    const cached = pass.get(key);
    if (cached instanceof Error) throw cached;
    return cached;
  }

  // A failure is kept for the pass alone, so a fixed binary or install
  // recovers on the next pass.
  const failure = message => {
    const error = new Error(message);
    store(error);
    return error;
  };

  const args = [
    ...commandArgs,
    "--project",
    project,
    "--format",
    "json"
  ];
  if (dialect) args.push("--dialect", dialect);
  if (runtimeResolution === "required") args.push("--runtime-resolution", "required");
  for (const preset of presets) args.push("--preset", preset);
  for (const rule of enableRules) args.push("--enable-rule", rule);
  if (runtime?.target) args.push("--runtime-target", runtime.target);
  if (runtime?.build) args.push("--runtime-build", runtime.build);
  if (runtime?.rendering) args.push("--rendering", runtime.rendering);
  if (runtime?.programBoundary) {
    args.push("--program-boundary", runtime.programBoundary);
  }
  for (const condition of runtime?.conditions ?? []) {
    args.push("--runtime-condition", condition);
  }
  for (const transform of runtime?.frameworkTransforms ?? []) {
    args.push("--framework-transform", transform);
  }
  const result = spawnSync(command, args, {
    cwd: dirname(project),
    encoding: "utf8",
    env: {
      ...process.env,
      ...(native?.env ?? {}),
      SOLID_CHECKER_RUNTIME_RESOLVER: process.env.SOLID_CHECKER_RUNTIME_RESOLVER
        ?? native?.env.SOLID_CHECKER_RUNTIME_RESOLVER
        ?? join(__dirname, "scripts", "runtime-resolver.mjs"),
      ...(process.env.SOLID_TYPEFACTS_BIN ? { SOLID_TYPEFACTS_BIN: process.env.SOLID_TYPEFACTS_BIN } : {})
    }
  });
  if (result.error) {
    throw failure(`solid-checker adapter could not start analysis: ${result.error.message}`);
  }
  if (result.status !== 0) {
    const stderr = result.stderr.trim();
    throw failure(`solid-checker adapter analysis failed (${result.status}): ${stderr}`);
  }
  let snapshot;
  try {
    snapshot = JSON.parse(result.stdout);
  } catch (error) {
    throw failure(`solid-checker adapter received invalid JSON: ${error.message}`);
  }
  const notices = stderrNotices(result.stderr);
  if (inferredHostInputs !== inferenceFingerprint(project, runtime)) {
    throw failure("solid-checker adapter inferred-host inputs changed during analysis; retry lint");
  }
  if (notices.length > 0 && snapshot && typeof snapshot === "object") {
    snapshotNotices.set(snapshot, notices);
  }
  store(snapshot);
  return snapshot;
}

function samePath(left, right) {
  const normalize = value => resolve(value).replaceAll("\\", "/");
  return normalize(left) === normalize(right);
}

function byteOffsetToIndex(text, byteOffset) {
  if (byteOffset <= 0) return 0;
  let bytes = 0;
  let index = 0;
  for (const character of text) {
    const width = Buffer.byteLength(character);
    if (bytes + width > byteOffset) break;
    bytes += width;
    index += character.length;
  }
  return index;
}

function findingRange(sourceCode, location) {
  return [
    byteOffsetToIndex(sourceCode.text, location.startByte),
    byteOffsetToIndex(sourceCode.text, location.endByte)
  ];
}

function findingMessage(finding) {
  const hint = finding.hint ? `\n\n${finding.hint}` : "";
  const docsUrl = finding.documentationUrl ?? docsUrlsByRule.get(finding.rule);
  const docs = docsUrl ? `\n\nDocs: ${docsUrl}` : "";
  const host = (finding.evidence ?? [])
    .filter(step => step.message?.startsWith("inferred browser:"))
    .map(step => `\n\n${step.message}`)
    .join("");
  return `[${finding.id}] ${finding.message}${hint}${host}${docs}`;
}

function fixForFinding(fixer, finding, sourceCode, filename) {
  const fix = finding.fixes?.find(candidate =>
    candidate.applicability === "safe" &&
    candidate.edits?.every(edit => samePath(edit.location.path, filename))
  );
  if (!fix) return null;
  return fix.edits.map(edit =>
    fixer.replaceTextRange(
      findingRange(sourceCode, edit.location),
      edit.newText
    )
  );
}

const adapterSchema = [{
  type: "object",
  additionalProperties: false,
  properties: {
    command: { type: "string" },
    commandArgs: { type: "array", items: { type: "string" } },
    project: { type: "string" },
    cwd: { type: "string" },
    acceptedContracts: { type: "string" },
    receiptTrustConfiguration: { type: "string" },
    dialect: { type: "string" },
    preset: { type: "array", items: { type: "string" } },
    enableRule: { type: "array", items: { type: "string" } },
    snapshotPath: { type: "string" }
  }
}];

/**
 * Project findings into ESLint reports: byte-range conversion, same-file
 * filtering, safe fixes. Shared by `certification` and every per-rule rule,
 * so the two surfaces render one finding identically.
 */
function projectFindings(context, program, findings) {
  const sourceCode = context.sourceCode ?? context.getSourceCode();
  const filename = contextFilename(context);
  for (const finding of findings) {
    const location = finding.primaryLocation;
    // A project-scoped finding is about the project, not about a file in it:
    // the refusal to analyze an unsupported Solid runtime is located at the
    // deciding `node_modules/solid-js/package.json`, which ESLint never lints.
    // Matching it by path would drop it silently and leave the user with a
    // clean run over a project that was never analyzed at all -- exactly the
    // false certification the finding exists to prevent. So it is reported on
    // every linted file, and its span is this file's origin rather than an
    // offset into some other file's bytes.
    const projectScoped = finding.subjectKind === "project";
    // A finding about a package or one package export is collapsed over the
    // project: its related locations are further *sites* of the same finding,
    // not supporting context. Report it in every file holding a site, at that
    // file's first one, so the collapse never hides a file's sites from ESLint.
    const collapsed = SITE_SUBJECTS.has(finding.subjectKind);
    const site = collapsed
      ? [location, ...(finding.relatedLocations ?? [])]
        .filter(candidate => candidate?.path && samePath(candidate.path, filename))
        .reduce((first, candidate) =>
          first && first.startByte <= candidate.startByte ? first : candidate, undefined)
      : location;
    if (!projectScoped && collapsed && !site) continue;
    if (!projectScoped && site?.path && !samePath(site.path, filename)) continue;
    const range = site && !projectScoped ? findingRange(sourceCode, site) : [0, 0];
    context.report({
      node: program,
      loc: {
        start: sourceCode.getLocFromIndex(range[0]),
        end: sourceCode.getLocFromIndex(range[1])
      },
      messageId: "finding",
      data: {
        message: findingMessage(finding)
      },
      fix: finding.fixes?.length
        ? fixer => fixForFinding(fixer, finding, sourceCode, filename)
        : undefined
    });
  }
}

/**
 * Report the snapshot's run notes on this file.
 *
 * A note is about the run, not about a file, so it is reported the way a
 * project-scoped finding is: on every linted file, at the file's origin. Once
 * per ESLint process would pin it to whichever file happened to be linted
 * first -- an editor, which lints the open file alone, would show it on one
 * buffer and never again, and `eslint --cache` would replay it on an
 * arbitrary file. ESLint gives a plugin no project-level message and no
 * warning channel its formatters or editors display; `process.emitWarning`
 * reaches only a terminal, which is the invisibility this replaces.
 */
function reportNotices(context, program, snapshot) {
  const notices = snapshotNotices.get(snapshot);
  if (!notices) return;
  const sourceCode = context.sourceCode ?? context.getSourceCode();
  const origin = sourceCode.getLocFromIndex(0);
  for (const notice of notices) {
    context.report({
      node: program,
      loc: { start: origin, end: origin },
      messageId: "notice",
      data: { message: noticeMessage(notice) }
    });
  }
}

const certification = {
  meta: {
    type: "problem",
    docs: {
      description: "Report canonical solid-checker project findings",
      recommended: true
    },
    fixable: "code",
    schema: adapterSchema,
    messages: { finding: "{{message}}" }
  },
  create(context) {
    return {
      Program(program) {
        const snapshot = loadSnapshot(context);
        // Skip findings a per-rule rule registered for during this pass:
        // that rule reports them at its own severity, so certification
        // reporting them again would duplicate every one of its findings.
        const owned = ownedRules.get(contextFilename(context));
        const findings = (snapshot.findings ?? []).filter(
          finding =>
            !owned?.has(finding.rule) &&
            !owned?.has(ownedKey(finding.rule, finding.id)) &&
            userFacing(finding)
        );
        projectFindings(context, program, findings);
      },
      "Program:exit"() {
        endPass(context);
      }
    };
  }
};

/**
 * The run's notes, and nothing else, as their own rule.
 *
 * A note never fails a lint by itself: it names a configuration the run fell
 * back from, not a defect, so every shipped config enables this rule at
 * `warn`, and a project that sets it to `off` has chosen not to see notes.
 * It is its own rule rather than a message of `certification` or of a
 * per-rule rule because a message takes its rule's severity, and those are
 * errors under `recommended`.
 *
 * It loads the snapshot every other rule of the pass loads (the cache key
 * reads only the settings, the options, and the per-file registry, all fixed
 * before the first `Program`), so enabling it spawns no second analysis. It
 * keeps no state across passes, so a pass that ended in a thrown analysis
 * cannot cost a later pass its note.
 */
const packageNote = {
  meta: {
    type: "suggestion",
    docs: {
      description:
        "Report solid-checker run notes, including runtime-host decisions and unavailable dependency source",
      recommended: true
    },
    schema: adapterSchema,
    messages: { notice: "{{message}}" }
  },
  create(context) {
    return {
      Program(program) {
        reportNotices(context, program, loadSnapshot(context));
      },
      "Program:exit"() {
        endPass(context);
      }
    };
  }
};

/**
 * One ESLint rule per diagnostic identity, so a project can disable
 * `solid-checker/strict-read-untracked` without losing every other finding.
 *
 * The rule owns no analysis: it narrows the shared snapshot to its own rule
 * name and reuses `certification`'s projection rather than copying it. Every
 * rule of one dialect shares one analysis run — the module-level snapshot
 * cache keys on the dialect, so 38 v1 rules over a project still spawn the
 * binary once.
 *
 * `create` registers the rule's identity for the linted file before any
 * traversal event fires, which is how `certification` knows to leave these
 * findings alone whichever config order enabled both surfaces.
 */
function reportingRule(entry, catalog, codes = null) {
  const covered = codes ?? entry.codes ?? [entry.code];
  return {
    meta: {
      type: "problem",
      docs: {
        description: `solid-checker ${covered.join("/")} ${entry.name}`,
        recommended: entry.defaultEnabled && !entry.uncertifiable,
        url: `${catalog.docsBaseUrl}/${entry.name}.md`
      },
      fixable: "code",
      schema: adapterSchema,
      messages: { finding: "{{message}}" }
    },
    create(context) {
      const filename = contextFilename(context);
      const owned = codes ? codes.map(code => ownedKey(entry.name, code)) : [entry.name];
      for (const key of owned) registerOwnedRule(filename, key);
      return {
        Program(program) {
          // A namespaced compatibility rule analyzes with its manifest's
          // dialect unless the config already chose one. The default,
          // unprefixed surface leaves selection to project detection.
          const forced =
            catalog.namespace && !configuration(context).dialect
              ? contextWithDialect(context, catalog.dialect)
              : context;
          const snapshot = loadSnapshot(forced);
          const findings = (snapshot.findings ?? []).filter(
            finding =>
              finding.rule === entry.name &&
              (!codes || codes.includes(finding.id)) &&
              userFacing(finding)
          );
          if (findings.length === 0) return;
          projectFindings(context, program, findings);
        },
        "Program:exit"() {
          for (const key of owned) releaseOwnedRule(filename, key);
          endPass(context);
        }
      };
    }
  };
}

function contextWithDialect(context, dialect) {
  const solidChecker = { ...(context.settings?.solidChecker ?? {}), dialect };
  return Object.create(context, {
    settings: { value: { ...context.settings, solidChecker }, enumerable: true }
  });
}

const discoveredCatalogs = readdirSync(join(__dirname, "lib"))
  .filter(file => /^rules-solid-v\d+\.json$/.test(file))
  .sort()
  .map(file => {
    const catalog = require(join(__dirname, "lib", file));
    if (
      catalog.schemaVersion !== 1 ||
      typeof catalog.dialect !== "string" ||
      typeof catalog.config !== "string" ||
      typeof catalog.namespace !== "string" ||
      !Array.isArray(catalog.rules) ||
      !catalog.rules.every(entry =>
        typeof entry.defaultEnabled === "boolean" && Array.isArray(entry.presets)
      )
    ) {
      throw new Error(`invalid solid-checker rule manifest ${file}`);
    }
    return catalog;
  });
const manifests = Object.fromEntries(
  discoveredCatalogs.map(catalog => [catalog.dialect, catalog]),
);
if (Object.keys(manifests).length !== discoveredCatalogs.length) {
  throw new Error("duplicate dialect in solid-checker rule manifests");
}
const docsUrlsByRule = new Map(
  discoveredCatalogs.flatMap(catalog =>
    catalog.rules.map(entry => [entry.name, `${catalog.docsBaseUrl}/${entry.name}.md`])
  )
);
const manifestEntriesByRule = new Map(
  discoveredCatalogs.flatMap(catalog => catalog.rules.map(entry => [entry.name, entry]))
);

const plugin = {
  meta: { name: "solid-checker", version: packageVersion },
  rules: { certification, "package-note": packageNote, "contract-note": packageNote },
  configs: {}
};

// Old explicit ESLint keys retained for one minor release. These entries do
// not appear in generated catalogs or presets; they delegate to the current
// identity and carry ESLint's deprecation metadata.
const DEPRECATED_RULE_KEYS = [
  ["component-props-destructure", "no-destructure"],
  ["component-returns-conditionally", "components-return-once"],
  ["expected-function-got-expression", "reactive-handler-frozen"],
  // The `v1/` alias went with the 1.x catalog (ADR 0110): a deprecation alias
  // can only delegate to a rule that still exists, and its target does not.
  ["reactive-write-in-owned-scope", "no-write-in-pure-computation"],
  ["no-direct-mutation", "no-store-mutation-outside-setter"],
  // The third entry limits a former name to the one code of the merged rule it
  // covered (ADR 0384), so each keeps its own severity and findings.
  ["resolve-in-tracked-scope", "no-await-primitive-in-tracked-scope", ["SC2004"]],
  ["resolve-in-reactive-scope", "no-await-primitive-in-tracked-scope", ["SC2004"]],
  ["until-in-tracked-scope", "no-await-primitive-in-tracked-scope", ["SC2005"]],
  ["server-function-module-directive", "valid-use-server"],
  ["sync-node-received-async", "sync-computation-received-async"]
];

// ADR 0398: SC9005 and SC9012 (with SC9011 merged into it) are obligations,
// internal uncertifiable results, and no ESLint key configures one. A key a
// config still names stays a deprecated rule that reports nothing, so an
// existing config loads and its setting changes nothing.
const RETIRED_RULE_KEYS = [
  "package-behavior-unavailable",
  "package-contract-incomplete",
  "reactive-dispatch-unresolved",
  "reactive-source-uncaptured"
];

for (const catalog of Object.values(manifests)) {
  for (const entry of catalog.rules) {
    if (Object.hasOwn(plugin.rules, entry.name)) {
      throw new Error(`catalog rule ${entry.name} collides with an adapter rule`);
    }
    plugin.rules[entry.name] = reportingRule(entry, catalog);
  }
}

for (const [oldName, currentName, codes] of DEPRECATED_RULE_KEYS) {
  const catalog = Object.values(manifests).find(candidate =>
    candidate.rules.some(entry => entry.name === currentName)
  );
  if (!catalog) throw new Error(`deprecated rule target ${currentName} is absent`);
  const entry = catalog.rules.find(candidate => candidate.name === currentName);
  const delegated = reportingRule(entry, catalog, codes ?? null);
  plugin.rules[oldName] = {
    ...delegated,
    meta: {
      ...delegated.meta,
      deprecated: true,
      replacedBy: [currentName]
    }
  };
}

for (const name of RETIRED_RULE_KEYS) {
  if (Object.hasOwn(plugin.rules, name)) {
    throw new Error(`retired rule key ${name} is still a catalog rule`);
  }
  plugin.rules[name] = {
    meta: {
      type: "problem",
      docs: {
        description: `solid-checker ${name} is an obligation, not a rule (ADR 0398); this key does nothing`
      },
      deprecated: true,
      replacedBy: [],
      schema: adapterSchema,
      messages: {}
    },
    create() {
      return {};
    }
  };
}

plugin.configs.recommended = {
  plugins: { "solid-checker": plugin },
  rules: {
    "solid-checker/certification": "error",
    "solid-checker/contract-note": "warn"
  }
};
for (const catalog of Object.values(manifests)) {
  plugin.configs[catalog.config] = {
    plugins: { "solid-checker": plugin },
    rules: {
      // Turning certification off keeps a `[recommended, dialect]` listing
      // from even creating the rule. The reverse listing re-enables it, but
      // the per-file registry above makes certification skip every finding a
      // per-rule rule owns, so both orders report each finding exactly once.
      "solid-checker/certification": "off",
      // Every shipped config enables the note rule at `warn`, so no listing
      // order can make a note fail a lint.
      // Shipped configs keep the historical key: a user config that turned
      // `contract-note` off must keep silencing the note.
      "solid-checker/contract-note": "warn",
      // The manifest lists configurable rules only (ADR 0398): an obligation
      // has no key.
      ...Object.fromEntries(
        catalog.rules
          .filter(entry => entry.defaultEnabled)
          .map(entry => [
          `solid-checker/${entry.name}`,
          entry.severity === "error" ? "error" : "warn"
        ])
      )
    }
  };
  // ADR 0269: the dialect config with the browser runtime target. For client
  // applications, where analyzed code runs in the browser; a later config
  // that sets `settings.solidChecker.runtime` overrides it.
  plugin.configs[`browser-${catalog.config}`] = {
    ...plugin.configs[catalog.config],
    settings: { solidChecker: { runtime: { target: "browser" } } }
  };
  const preferenceRules = catalog.rules.filter(entry =>
    entry.presets.includes("preferences")
  );
  plugin.configs[`preferences-${catalog.config}`] = {
    plugins: { "solid-checker": plugin },
    settings: { solidChecker: { preset: ["preferences"] } },
    rules: {
      "solid-checker/contract-note": "warn",
      ...Object.fromEntries(
        preferenceRules.map(entry => [
          `solid-checker/${entry.name}`,
          entry.severity === "error" ? "error" : "warn"
        ])
      )
    }
  };
}

module.exports = plugin;
module.exports._testing = {
  byteOffsetToIndex,
  configuredProject,
  findProject,
  findingMessage,
  loadSnapshot,
  passSnapshots,
  passOf,
  nativeValueOptions: NATIVE_VALUE_OPTIONS,
  nativeFlags: NATIVE_FLAGS,
  manifests,
  manifestEntriesByRule,
  deprecatedRuleKeys: DEPRECATED_RULE_KEYS,
  retiredRuleKeys: RETIRED_RULE_KEYS,
  ownedRules,
  snapshotCache
};
