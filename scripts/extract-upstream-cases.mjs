// Extracts every valid/invalid case from upstream eslint-plugin-solid's rule
// tests into a JSON corpus.
//
// The test files are uniform — `export const cases = run(name, rule, {valid,
// invalid})` — so stubbing the three imports and evaluating the module yields
// the cases as data. Node 24 strips the type annotations natively.
import { mkdirSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { basename, join } from "node:path";
import { pathToFileURL } from "node:url";

const [source, destination] = process.argv.slice(2);
const staging = join(destination, ".staging");
mkdirSync(staging, { recursive: true });

const STUBS = `
const run = (name, rule, cases) => ({ name, ...cases });
const tsOnly = Symbol.for("ts only");
const rule = {};
const T = new Proxy({}, { get: (_, key) => String(key) });
`;

const corpus = [];
for (const entry of readdirSync(source).filter((f) => f.endsWith(".test.ts"))) {
  const original = readFileSync(join(source, entry), "utf8");
  // Drop the imports the stubs replace; leave any other line untouched so a
  // case's own text is never rewritten.
  const body = original
    .split("\n")
    .filter((line) => !/^import\s.*from\s+"(\.\.\/ruleTester|\.\.\/\.\.\/src\/rules\/|@typescript-eslint\/utils)/.test(line))
    .join("\n");
  const staged = join(staging, entry.replace(".test.ts", ".mts"));
  writeFileSync(staged, STUBS + body);
  let module;
  try {
    module = await import(pathToFileURL(staged).href);
  } catch (error) {
    console.error(`SKIP ${entry}: ${error.message.split("\n")[0]}`);
    continue;
  }
  const cases = module.cases;
  if (!cases) {
    console.error(`SKIP ${entry}: no exported cases`);
    continue;
  }
  const normalise = (test, expected) => {
    const object = typeof test === "string" ? { code: test } : test;
    return {
      code: object.code,
      errors: expected ? (object.errors?.length ?? 1) : 0,
      messageIds: expected ? (object.errors ?? []).map((e) => e.messageId).filter(Boolean) : [],
      options: object.options ?? null,
      output: object.output ?? null,
      tsOnly: Boolean(object[Symbol.for("ts only")]),
    };
  };
  corpus.push({
    rule: cases.name ?? basename(entry, ".test.ts"),
    valid: (cases.valid ?? []).map((t) => normalise(t, false)),
    invalid: (cases.invalid ?? []).map((t) => normalise(t, true)),
  });
}

corpus.sort((a, b) => a.rule.localeCompare(b.rule));
let valid = 0;
let invalid = 0;
for (const rule of corpus) {
  valid += rule.valid.length;
  invalid += rule.invalid.length;
  console.log(`${rule.rule.padEnd(26)} ${rule.valid.length} valid, ${rule.invalid.length} invalid`);
}
writeFileSync(join(destination, "upstream-cases.json"), `${JSON.stringify(corpus, null, 2)}\n`);
console.log(`\n${corpus.length} rules, ${valid} valid, ${invalid} invalid, ${valid + invalid} total`);
