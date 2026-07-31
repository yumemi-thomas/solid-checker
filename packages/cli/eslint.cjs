"use strict";

const { existsSync, readFileSync } = require("node:fs");
const { dirname, isAbsolute, join, parse, resolve } = require("node:path");
const { spawnSync } = require("node:child_process");

const packageVersion = require("./package.json").version;
const snapshotCache = new Map();

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
  const contracts = Array.isArray(config.contracts) ? config.contracts : [];
  const dialect = config.dialect ?? null;
  const key = JSON.stringify({ command, commandArgs, project, contracts, dialect });
  if (snapshotCache.has(key)) return snapshotCache.get(key);

  const args = [
    ...commandArgs,
    "--project",
    project,
    "--format",
    "json"
  ];
  if (dialect) args.push("--dialect", dialect);
  for (const contract of contracts) args.push("--contract", contract);
  const result = spawnSync(command, args, {
    cwd: dirname(project),
    encoding: "utf8",
    env: process.env
  });
  if (result.error) {
    throw new Error(`solid-checker adapter could not start analysis: ${result.error.message}`);
  }
  if (result.status !== 0) {
    throw new Error(
      `solid-checker adapter analysis failed (${result.status}): ${result.stderr.trim()}`
    );
  }
  let snapshot;
  try {
    snapshot = JSON.parse(result.stdout);
  } catch (error) {
    throw new Error(`solid-checker adapter received invalid JSON: ${error.message}`);
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
  return `[${finding.id}] ${finding.message}${hint}`;
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
    contracts: { type: "array", items: { type: "string" } },
    dialect: { type: "string" },
    snapshotPath: { type: "string" }
  }
}];

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
        const sourceCode = context.sourceCode ?? context.getSourceCode();
        const filename = contextFilename(context);
        for (const finding of snapshot.findings ?? []) {
          const location = finding.primaryLocation;
          if (location?.path && !samePath(location.path, filename)) continue;
          const range = location ? findingRange(sourceCode, location) : [0, 0];
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
    };
  }
};

/**
 * Hand the certification rule a snapshot narrowed to one diagnostic identity.
 *
 * `loadSnapshot` short-circuits on `settings.solidChecker.snapshot`, so a
 * context carrying a pre-filtered snapshot re-runs the projection (byte-range
 * conversion, same-file filtering, safe fixes) without re-running the
 * analysis. Options are cleared except the dialect: every other option the
 * adapter accepts concerns snapshot acquisition, which has already happened.
 */
function narrowedContext(context, snapshot) {
  const solidChecker = { ...(context.settings?.solidChecker ?? {}), snapshot };
  return Object.create(context, {
    settings: { value: { ...context.settings, solidChecker }, enumerable: true },
    options: { value: [], enumerable: true }
  });
}

/**
 * One ESLint rule per diagnostic identity, so a project can disable
 * `solid-checker/strict-read-untracked` without losing every other finding.
 *
 * The rule owns no analysis: it narrows the shared snapshot to its own rule
 * name and re-enters `certification`, whose projection is reused rather than
 * copied. Every rule of one dialect shares one analysis run — the module-level
 * snapshot cache keys on the dialect, so 38 v1 rules over a project still
 * spawn the binary once.
 */
function reportingRule(entry, dialect) {
  return {
    meta: {
      type: "problem",
      docs: {
        description: `solid-checker ${entry.code} ${entry.name}`,
        recommended: !entry.uncertifiable,
        url: `${manifest(dialect).docsBaseUrl}/${entry.name}.md`
      },
      fixable: "code",
      schema: adapterSchema,
      messages: { finding: "{{message}}" }
    },
    create(context) {
      return {
        Program(program) {
          // A v1/-namespaced rule analyzes with the v1 dialect unless the
          // config already chose one; unprefixed rules leave selection to the
          // config or the binary's own project detection.
          const forced =
            dialect === "v1" && !configuration(context).dialect
              ? contextWithDialect(context, "solid-v1")
              : context;
          const snapshot = loadSnapshot(forced);
          const findings = (snapshot.findings ?? []).filter(
            finding => finding.rule === entry.name
          );
          if (findings.length === 0) return;
          certification
            .create(narrowedContext(context, { ...snapshot, findings }))
            .Program(program);
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

const manifests = {
  v1: require("./lib/rules-v1.json"),
  v2: require("./lib/rules-v2.json")
};

function manifest(dialect) {
  return manifests[dialect];
}

const plugin = {
  meta: { name: "solid-checker", version: packageVersion },
  rules: { certification },
  configs: {}
};

for (const [dialect, catalog] of Object.entries(manifests)) {
  for (const entry of catalog.rules) {
    // v1 identities already carry their namespace ("v1/no-destructure");
    // v2 stays unprefixed — the checker's own names are the default surface.
    plugin.rules[entry.name] = reportingRule(entry, dialect);
  }
}

plugin.configs.recommended = {
  plugins: { "solid-checker": plugin },
  rules: { "solid-checker/certification": "error" }
};
for (const [dialect, catalog] of Object.entries(manifests)) {
  plugin.configs[dialect] = {
    plugins: { "solid-checker": plugin },
    rules: Object.fromEntries(
      catalog.rules.map(entry => [
        `solid-checker/${entry.name}`,
        entry.severity === "error" ? "error" : "warn"
      ])
    )
  };
}

module.exports = plugin;
module.exports._testing = {
  byteOffsetToIndex,
  configuredProject,
  findProject,
  loadSnapshot,
  snapshotCache
};
