"use strict";

const { existsSync, readFileSync, readdirSync } = require("node:fs");
const { dirname, isAbsolute, join, parse, resolve } = require("node:path");
const { spawnSync } = require("node:child_process");
const { createHash } = require("node:crypto");

const packageVersion = require("./package.json").version;
const snapshotCache = new Map();
/**
 * Subject kinds whose `relatedLocations` are further sites of the finding
 * itself: the checker collapses one package-contract obligation per package,
 * or per package export, over the whole project (`projection.rs`,
 * `collapse_unaccepted_contract_defects`).
 */
const SITE_SUBJECTS = new Set(["package", "package-export"]);

/**
 * The native analysis's `solid-checker: note:` stderr lines, per snapshot.
 *
 * A note is not a finding: it says something about the run itself -- today,
 * that a discovered project catalog was withheld because no receipt trust
 * configuration was supplied, so the contracts it carries were not admitted.
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

const TRUST_REMEDY =
  "In ESLint, set settings.solidChecker.receiptTrustConfiguration to that trust file.";

/** A note as an ESLint message, naming the setting behind the flag it cites. */
function noticeMessage(notice) {
  const remedy = notice.includes("--receipt-trust-configuration")
    ? `\n\n${TRUST_REMEDY}`
    : "";
  return `[solid-checker note] ${notice}${remedy}`;
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
 * The receipt trust configuration, as a cache identity: its resolved path and
 * the digest of its bytes. The native daemon already hashes the file into its
 * cached-answer inputs; this is the same rule for the adapter's in-process
 * snapshot cache, so an edited or replaced trust file re-runs the analysis in
 * a persistent ESLint session instead of serving a verdict it no longer
 * supports. An unreadable file is a configuration error, reported before any
 * analysis starts and not cached, so fixing the file recovers.
 */
function receiptTrust(config) {
  const path = configuredPath(config, "receiptTrustConfiguration");
  if (path == null) return null;
  let bytes;
  try {
    bytes = readFileSync(path);
  } catch (error) {
    throw new Error(
      "solid-checker adapter could not read settings.solidChecker.receiptTrustConfiguration " +
      `${path}: ${error.message}`
    );
  }
  return { path, sha256: createHash("sha256").update(bytes).digest("hex") };
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
  const command = config.command ?? process.env.SOLID_CHECKER_BIN ?? process.execPath;
  const commandArgs = config.command || process.env.SOLID_CHECKER_BIN
    ? [...(config.commandArgs ?? [])]
    : [join(__dirname, "bin", "solid-checker.mjs")];
  const acceptedContracts = configuredPath(config, "acceptedContracts");
  const trust = receiptTrust(config);
  const dialect = config.dialect ?? null;
  const presets = [...new Set(Array.isArray(config.preset) ? config.preset : [])].sort();
  const runtime = runtimeConfiguration(config);
  const configuredRules = Array.isArray(config.enableRule) ? config.enableRule : [];
  const activeDefaultDisabled = [...(ownedRules.get(contextFilename(context)) ?? [])]
    .filter(rule => manifestEntriesByRule.get(rule)?.defaultEnabled === false);
  const enableRules = [...new Set([...configuredRules, ...activeDefaultDisabled])].sort();
  const key = JSON.stringify({
    command,
    commandArgs,
    project,
    acceptedContracts,
    trust,
    dialect,
    presets,
    enableRules,
    runtime
  });
  if (snapshotCache.has(key)) {
    const cached = snapshotCache.get(key);
    if (cached instanceof Error) throw cached;
    return cached;
  }

  // Failures share the snapshot cache and its process lifetime: a persistent
  // editor session with a broken binary reports the cached error to every
  // rule of every lint pass instead of re-spawning the checker each time.
  const failure = message => {
    const error = new Error(message);
    snapshotCache.set(key, error);
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
  if (acceptedContracts) args.push("--accepted-contracts", acceptedContracts);
  if (trust) args.push("--receipt-trust-configuration", trust.path);
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
    env: process.env
  });
  if (result.error) {
    throw failure(`solid-checker adapter could not start analysis: ${result.error.message}`);
  }
  if (result.status !== 0) {
    const stderr = result.stderr.trim();
    // A named policy-2 catalog without trust refuses by citing the CLI flag;
    // name the setting that supplies it here.
    const remedy = !trust && stderr.includes("--receipt-trust-configuration")
      ? `\n\n${TRUST_REMEDY}`
      : "";
    throw failure(
      `solid-checker adapter analysis failed (${result.status}): ${stderr}${remedy}`
    );
  }
  let snapshot;
  try {
    snapshot = JSON.parse(result.stdout);
  } catch (error) {
    throw failure(`solid-checker adapter received invalid JSON: ${error.message}`);
  }
  const notices = stderrNotices(result.stderr);
  if (notices.length > 0 && snapshot && typeof snapshot === "object") {
    snapshotNotices.set(snapshot, notices);
  }
  snapshotCache.set(key, snapshot);
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
  return `[${finding.id}] ${finding.message}${hint}${docs}`;
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
          finding => !owned?.has(finding.rule)
        );
        projectFindings(context, program, findings);
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
const contractNote = {
  meta: {
    type: "suggestion",
    docs: {
      description:
        "Report solid-checker run notes, such as a project contract catalog withheld for want of receipt trust",
      recommended: true
    },
    schema: adapterSchema,
    messages: { notice: "{{message}}" }
  },
  create(context) {
    return {
      Program(program) {
        reportNotices(context, program, loadSnapshot(context));
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
function reportingRule(entry, catalog) {
  return {
    meta: {
      type: "problem",
      docs: {
        description: `solid-checker ${entry.code} ${entry.name}`,
        recommended: entry.defaultEnabled && !entry.uncertifiable,
        url: `${catalog.docsBaseUrl}/${entry.name}.md`
      },
      fixable: "code",
      schema: adapterSchema,
      messages: { finding: "{{message}}" }
    },
    create(context) {
      const filename = contextFilename(context);
      registerOwnedRule(filename, entry.name);
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
            finding => finding.rule === entry.name
          );
          if (findings.length === 0) return;
          projectFindings(context, program, findings);
        },
        "Program:exit"() {
          releaseOwnedRule(filename, entry.name);
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
  rules: { certification, "contract-note": contractNote },
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
  ["resolve-in-reactive-scope", "resolve-in-tracked-scope"],
  ["sync-node-received-async", "sync-computation-received-async"]
];

for (const catalog of Object.values(manifests)) {
  for (const entry of catalog.rules) {
    if (Object.hasOwn(plugin.rules, entry.name)) {
      throw new Error(`catalog rule ${entry.name} collides with an adapter rule`);
    }
    plugin.rules[entry.name] = reportingRule(entry, catalog);
  }
}

for (const [oldName, currentName] of DEPRECATED_RULE_KEYS) {
  const catalog = Object.values(manifests).find(candidate =>
    candidate.rules.some(entry => entry.name === currentName)
  );
  if (!catalog) throw new Error(`deprecated rule target ${currentName} is absent`);
  const entry = catalog.rules.find(candidate => candidate.name === currentName);
  const delegated = reportingRule(entry, catalog);
  plugin.rules[oldName] = {
    ...delegated,
    meta: {
      ...delegated.meta,
      deprecated: true,
      replacedBy: [currentName]
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
      "solid-checker/contract-note": "warn",
      ...Object.fromEntries(
        catalog.rules.filter(entry => entry.defaultEnabled).map(entry => [
          `solid-checker/${entry.name}`,
          entry.severity === "error" ? "error" : "warn"
        ])
      )
    }
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
  manifests,
  manifestEntriesByRule,
  deprecatedRuleKeys: DEPRECATED_RULE_KEYS,
  ownedRules,
  snapshotCache
};
