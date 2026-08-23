// Executing one `tsc`-oracle case: the expensive half of the oracle gate.
//
// Split out of `scripts/tsc-oracle-gate.mjs` so it can run in a worker thread.
// The gate's *judgement* -- every failure sentence, every cross-case invariant
// -- deliberately stayed behind in the gate, in case order. What lives here is
// only the part that costs: two `ts.createProgram` passes and two checker
// processes per case, none of which depends on any other case.
//
// A case is still its own project, for the reason the gate's own comment gives:
// a case is a claim about exactly those bytes, and project-level analysis can
// differ once unrelated files join. Concurrency does not change that -- each
// case keeps its own `case-<index>` directory, and the index is unique across
// both dialects, so two concurrent cases never share state.
import { execFileSync } from "node:child_process";
import {
  existsSync,
  mkdirSync,
  readFileSync,
  readlinkSync,
  symlinkSync,
  unlinkSync,
  writeFileSync,
} from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import {
  oracleCompilerOptions,
  oracleProject,
  oracleSubjectSpan,
  runOracle,
} from "../tsc-oracle.mjs";

export const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
// Case projects live under the one ignored build root, beside the audited
// installs they resolve against.
export const CASE_ROOT = join(ROOT, "rust/target/tsc-oracle-cases");

export const DIALECTS = ["v1", "v2"];

export const catalogEntries = [
  ...JSON.parse(readFileSync(join(ROOT, "packages/cli/lib/rules-solid-v1.json"), "utf8")).rules,
  ...JSON.parse(readFileSync(join(ROOT, "packages/cli/lib/rules-solid-v2.json"), "utf8")).rules,
];
export const catalogByName = new Map(catalogEntries.map((rule) => [rule.name, rule]));

export const canonicalRule = (testCase) =>
  testCase.dialect === "v1" && !testCase.rule.startsWith("v1/")
    ? `v1/${testCase.rule}`
    : testCase.rule;

const slug = (name) => name.replace(/[^a-z0-9]+/gi, "-");

export const sourceName = (testCase, index) =>
  `${slug(testCase.rule)}-${index}.${testCase.sourceExtension ?? "tsx"}`;

// One directory per dialect, holding a symlink to that dialect's audited
// install. A symlink rather than a copy because the checker picks its dialect
// from the nearest `node_modules/solid-js` above the project -- so the tree the
// oracle compiles against is also the tree that decides which catalog runs --
// and because the audited install must stay read-only.
/**
 * A symlink at `link` pointing at exactly `target`, or a loud failure.
 *
 * The subtlety this exists for: `existsSync` *follows* a symlink, so a
 * **dangling** link makes an `existsSync(link)` guard false, `symlinkSync` then
 * throws `EEXIST`, and swallowing `EEXIST` as "someone else already made the
 * same link" leaves a base whose `node_modules` points nowhere. Reachable by
 * moving the checkout: the recorded target is absolute, `rust/target` travels
 * with the move, and every version check still passes against the new path.
 *
 * So `EEXIST` is tolerated -- two workers really can race on a cold build root
 * -- but only after reading the link and confirming it is the link this wanted.
 * A link pointing somewhere else is replaced; anything that is not a symlink,
 * or a target that does not exist, is a failure, because the dialect the
 * checker selects and the typings the oracle compiles against both come from
 * whatever is behind this link.
 */
export const ensureDirectoryLink = (link, target) => {
  const currentTarget = () => {
    try {
      return readlinkSync(link);
    } catch (error) {
      if (error.code === "ENOENT") return null;
      if (error.code === "EINVAL") {
        throw new Error(
          `${link} exists but is not a symlink; it must be a link to ${target}. ` +
            `Remove it (or \`make clean\`) and re-run.`,
        );
      }
      throw error;
    }
  };
  let current = currentTarget();
  if (current === null) {
    try {
      symlinkSync(target, link, "dir");
      current = target;
    } catch (error) {
      // Lost the race, or the guard above was fooled by a dangling link.
      if (error.code !== "EEXIST") throw error;
      current = currentTarget();
    }
  }
  if (current !== target) {
    // `unlinkSync`, not `rmSync`: `rmSync` stats *through* the link and refuses
    // a link-to-directory with `ERR_FS_EISDIR`, and `recursive: true` would
    // delete the audited install behind it. Unlinking removes only the link.
    unlinkSync(link);
    symlinkSync(target, link, "dir");
    current = currentTarget();
    if (current !== target) {
      throw new Error(`${link} points at ${JSON.stringify(current)}, not ${target}`);
    }
  }
  if (!existsSync(link)) {
    throw new Error(
      `${link} is a dangling link to ${target}: the audited install is not there. ` +
        `Run: node scripts/tsc-oracle.mjs provision --dialect all`,
    );
  }
  return link;
};

const prepared = new Map();
export const dialectBase = (dialect) => {
  if (prepared.has(dialect)) return prepared.get(dialect);
  const { root } = oracleProject(dialect);
  const base = join(CASE_ROOT, dialect);
  mkdirSync(base, { recursive: true });
  ensureDirectoryLink(join(base, "node_modules"), join(root, "node_modules"));
  const entry = { base };
  prepared.set(dialect, entry);
  return entry;
};

/**
 * Create both dialect bases up front, in the parent.
 *
 * The symlink creation above is race-tolerant, but doing it once before any
 * worker starts also means a *provisioning* failure surfaces as itself, in the
 * parent, rather than as N identical worker errors.
 */
export const prepareDialectBases = (dialects = DIALECTS) => {
  for (const dialect of dialects) dialectBase(dialect);
};

/**
 * Run the checker over one case, in its own project.
 *
 * The compiler options mirror the oracle's `strict` pass so both halves of a
 * case describe the same program.
 */
const runChecker = (testCase, index, strict, { checker, typefacts }) => {
  const { base } = dialectBase(testCase.dialect);
  const dir = join(base, `case-${index}`);
  mkdirSync(dir, { recursive: true });
  const pass = strict ? "strict" : "loose";
  const caseSourceName = sourceName(testCase, index);
  const code = testCase.code.endsWith("\n") ? testCase.code : `${testCase.code}\n`;
  writeFileSync(
    join(dir, `tsconfig.${pass}.json`),
    `${JSON.stringify(
      {
        compilerOptions: oracleCompilerOptions(
          testCase.dialect,
          strict,
          testCase.compilerOptions,
        ),
        files: [caseSourceName],
      },
      null,
      2,
    )}\n`,
  );
  writeFileSync(
    join(dir, caseSourceName),
    code,
  );
  const enablementArgs = [
    ...(testCase.presets ?? []).flatMap((preset) => ["--preset", preset]),
    ...(testCase.enableRules ?? []).flatMap((rule) => ["--enable-rule", rule]),
  ];
  const testedRule = canonicalRule(testCase);
  if (
    catalogByName.get(testedRule)?.defaultEnabled === false &&
    !(testCase.enableRules ?? []).includes(testedRule)
  ) {
    enablementArgs.push("--enable-rule", testedRule);
  }
  const output = execFileSync(
    checker,
    ["--format", "json", "--project", join(dir, `tsconfig.${pass}.json`), ...enablementArgs],
    {
      encoding: "utf8",
      maxBuffer: 256 * 1024 * 1024,
      env: { ...process.env, SOLID_TYPEFACTS_BIN: typefacts },
    },
  );
  const snapshot = JSON.parse(output);
  const findings = (snapshot.findings ?? []).map((finding) => {
    const location = finding.primaryLocation;
    const subject = oracleSubjectSpan(code, location.startByte, location.endByte, caseSourceName);
    return {
      id: finding.id,
      rule: finding.rule,
      kind: finding.kind,
      startByte: location.startByte,
      endByte: location.endByte,
      subjectStartByte: subject.startByte,
      subjectEndByte: subject.endByte,
    };
  });
  return { findings };
};

const errorsOnly = (diagnostics) => diagnostics.filter((d) => d.category === "error");

/**
 * Both sides of one case: what TypeScript reports about these bytes, and what
 * the checker reports about them.
 *
 * Both passes matter. "Only under `strict`" is not an exception the absolute
 * rule recognises, so a case is redundant if *either* pass reports it; a case
 * claiming silence has to be silent in both.
 *
 * @returns {{perPass: [string, object[]][], checkerPasses: [string, {findings: object[]}][]}}
 */
export const runCase = (testCase, index, { checker, typefacts }) => {
  const result = runOracle(
    testCase.dialect,
    [{ name: sourceName(testCase, index), code: testCase.code }],
    testCase.compilerOptions,
  );
  const perPass = [
    ["strict", errorsOnly(result.passes.strict)],
    ["loose", errorsOnly(result.passes.loose)],
  ];
  // The second half of the case: what the *checker* says about the same bytes.
  // Without it a `silent` case proves only that TypeScript is quiet, which an
  // over-narrowed rule that reports nothing at all satisfies just as well.
  const checkerPasses = [
    ["strict", runChecker(testCase, index, true, { checker, typefacts })],
    ["loose", runChecker(testCase, index, false, { checker, typefacts })],
  ];
  return { perPass, checkerPasses };
};
