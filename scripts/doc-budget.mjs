#!/usr/bin/env bun
// Documentation budget: keeps the agent-facing prose from regrowing.
//
// In 2026-10 the repository carried 6.7 MB of docs, a 24 KB AGENTS.md and
// 600 KB of fixture READMEs, most of it slice reports and review rounds for
// subsystems that no longer existed. Agents read it as current truth. The
// cut kept current invariants, live decisions and open items only; this gate
// holds that line. Slice reports belong in commit messages, a design decision
// in docs/decisions.md, an open item in docs/open-work.md.
//
//   bun scripts/doc-budget.mjs        check, exit 1 on any violation
import { readFileSync, statSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { dirname, join, resolve } from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

export const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");

/** Byte ceilings, about 1.5x the size at the 2026-10-19 cut. */
export const FILE_BUDGETS = {
  "AGENTS.md": 8_000,
  "CLAUDE.md": 1_000,
  // Raised for ADR 0383's latent-key item, and for ADR 0413.
  // Raised for ADR 0414 (the file stood 28,428 bytes before its two entries);
  // for ADR 0415 (the file stood 29,958 bytes).
  // and for ADR 0419 (the file stood 30,082 bytes).
  // Raised for ADR 0436's open cases: the file stood 6 bytes under 26_300.
  // And for ADRs 0437-0438's open cases: the file stood at 26_659 bytes against 26_700.
  // And for ADR 0439's leaf obligations: 27_971 bytes against 27_500.
  // And for ADR 0438's async and test-helper limits: 28_418 bytes against 28_000.
  // Raised again for the ADR 0381 and 0424-0441 docs series (catalog renames, ADRs 0440-0441 to come): 32_000.
  // And for ADR 0443's untracked-read misses: 32_541 bytes against 32_000: 33_000.
  // And for ADRs 0444-0445's trades (workspace boundary, exported capitalized components): 33_453 bytes against 33_000: 34_000.
  // Merging origin/main (the incremental-daemon track, ADRs 0382-0423) into the catalog-review branch (ADRs 0381, 0424-0448), with ADR 0449: the two sides' raises combine; the merged tree stood at 38,235 bytes: 39_000.
  "docs/open-work.md": 39_000,
  // Raised for ADR 0378: the file stood 2 bytes under 113_200; for ADR 0379,
  // 14 bytes under 113_800; for ADR 0380, 3 bytes under 114_700; and for the
  // catalog review's ADRs 0381 and 0424-0434, 4 bytes under 115_600; and for ADR 0435,
  // 213 bytes over 127_000; and for ADR 0436, 87 bytes under 127_300;
  // and for ADRs 0437-0438, 132_568 bytes against 129_100;
  // and for ADR 0439, 133_476 bytes against 132_700;
  // and for ADR 0438's async and test-helper extension, 135_756 bytes against 133_600.
  // Raised again for the ADR 0381 and 0424-0441 docs series (catalog renames, ADRs 0440-0441 to come): 145_000.
  // And for the Opus review's fixes to ADRs 0426, 0436, 0438 and 0442: 150_000.
  // And for ADR 0441: the file stood at 150_325 bytes against 150_000: 151_000.
  // And for ADR 0440: 152_282 bytes against 151_000: 153_000.
  // And for ADR 0443: 154_913 bytes against 153_000: 156_000.
  // And for ADRs 0428-0429's suppression comments: 156_435 bytes against 156_000: 157_000.
  // And for ADRs 0444-0445 (workspace program boundary, census with no entry): 160_120 bytes against 157_000: 161_000.
  // And for ADR 0446 (the reproduction set and the pre-release run): 162_517 bytes against 161_000: 163_500.
  // And for ADR 0447 (veto weight from a veto-off sweep) and ADR 0448 to come (verified coverage): 165_500.
  // origin/main's raises to the same file (instant-edits track): for its ADRs 0382-0400; and for ADRs 0411 and 0412; and
  // for ADR 0413 (the file stood 158,880 bytes); and for ADR 0414
  // (the file stood 160,518 bytes); and for ADR 0415 (the file stood 163,911 bytes);
  // and for ADR 0416 (the file stood 163,998 bytes before its entry);
  // and for ADR 0418 (the file stood 166,991 bytes before its entry); and for
  // ADR 0419 (the file stood 170,124 bytes before its entry and one edit); and for
  // ADR 0420 (the file stood 173,980 bytes before its entry); and for
  // ADR 0421 (the file stood 176,343 bytes before its entry); and for ADR 0422 (the file stood
  // 178,593 bytes before its entry, which removes the CI performance workflow); and for
  // ADR 0423 (the file stood 179,066 bytes before its entry, which adds the real-project gate).
  // Merging origin/main (the incremental-daemon track, ADRs 0382-0423) into the catalog-review branch (ADRs 0381, 0424-0448), with ADR 0449: the two sides' raises combine; the merged tree stood at 231,107 bytes: 232_000.
  // And for ADR 0450 (the file stood 231,443 bytes before its entry; 233_295 with its read extension): 234_000.
  "docs/decisions.md": 234_000,
};

export const GROUP_BUDGETS = [
  // Raised from 600_000 for ADRs 0382-0404 (instant-edits track), and for
  // ADRs 0411 and 0412, and for ADR 0413
  // (the group stood 628,977 bytes); for ADR 0414 (the group stood 631,357); and for
  // ADR 0415 (the group stood 635,538 bytes); and for ADR 0416
  // (the group stood 635,458 bytes before the entry, which added 2,578); and for ADR 0418
  // (the group stood 638,451 bytes before the entry and the open-work lines, which added 3,696); and for
  // ADR 0419 (the group stood 642,147 bytes before its entry, the open-work edit and the
  // protocol notes, which added 4,638); and for ADR 0420 (the group stood 646,785 bytes before
  // its entry, which added 2,363); and for ADR 0421 (the group stood 649,148 bytes before
  // its entry, which added 2,250).
  // and for ADR 0422 (the group stood 650,999 bytes before its entry, which added about 800);
  // and for ADR 0423 (the group stood 651,778 bytes before its entry, which added 1,762).
  // Raised for ADRs 0437-0438 and their rule-page mechanics: docs/ stood at 604_388 bytes.
  // And for ADR 0439 and its two rule pages: 607_028 bytes against 605_000.
  // And for ADR 0438's extension (decisions, open work, two rule pages): 610_122 bytes.
  // Raised again for the ADR 0381 and 0424-0441 docs series (catalog renames, ADRs 0440-0441 to come): 650_000.
  // And for ADRs 0440-0443 and their rule pages: 650_087 bytes against 650_000: 656_000.
  // And for ADRs 0428-0429 (suppression section, SC9016's page): 658_058 bytes against 656_000: 660_000.
  // And for ADRs 0444-0445 (decisions, open work, fixture READMEs): 662_349 bytes against 660_000: 664_000.
  // And for ADR 0446 (the reproduction set and the pre-release run): 665_446 bytes against 664_000: 667_000.
  // And for ADRs 0447-0448 (see above): 669_000.
  // Merging origin/main (the incremental-daemon track, ADRs 0382-0423) into the catalog-review branch (ADRs 0381, 0424-0448), with ADR 0449: the two sides' raises combine; the merged tree stood at 739,773 bytes: 741_000.
  // And for ADR 0450 (its entry and rule-page paragraphs): 743_600 bytes against 741_000: 744_500.
  { name: "docs/", test: (path) => path.startsWith("docs/"), bytes: 744_500 },
  {
    name: "fixture READMEs",
    test: (path) => path.startsWith("fixtures/") && path.endsWith("/README.md"),
    // Raised from 250 KB for the context-read fixtures (ADR 0352): the group was
    // 249.8 KB before they landed, and each case README is a claim the reader needs.
    // Raised again for ADR 0378's fixture: the group stood 1 byte under 256 KB.
    // And for ADR 0435's nine dispatch fixtures: the group stood 455 bytes under 257 KB.
    // And for ADR 0436's four spread fixtures: four READMEs took the group 2_395 bytes past 261 KB.
    // And for ADRs 0437-0438's fixtures (five handler READMEs, three test-runner projects): 271_432 bytes.
    // And for ADR 0439's fixture: 271_811 bytes against 271_500.
    // And for ADR 0438's async and helper variants and the vitest 4 project: 275_151 bytes.
    // Raised again for the ADR 0381 and 0424-0441 docs series (catalog renames, ADRs 0440-0441 to come): 290_000.
    // And for the Opus review's fixes (suite bodies per runner, after-await and sync-dispatch variants): 295_000.
    // And for ADRs 0428-0429's fixtures (group key, suppression comments): 297_071 bytes against 295_000: 298_000.
    // And for ADRs 0444-0445's fixtures (workspace boundary, census with no entry): 298_292 bytes against 298_000: 299_500.
    // And for ADR 0450's two fixtures: 301_497 bytes against 299_500: 302_000.
    bytes: 302_000,
  },
  { name: "skills", test: (path) => path.startsWith(".claude/skills/"), bytes: 25_000 },
];

/** One fixture README is a claim, not a report. */
export const FIXTURE_README_MAX = 8_000;

/**
 * What docs/ may hold. A new top-level document is how the append logs came
 * back last time, so adding one is a deliberate edit here.
 */
export const DOCS_ALLOWED = [
  "docs/README.md",
  "docs/adding-a-dialect.md",
  "docs/compiler-facts.md",
  "docs/decisions.md",
  "docs/development-feedback.md",
  "docs/monorepo.md",
  "docs/open-work.md",
  "docs/typefacts.md",
  "docs/rules/",
  "docs/typefacts/",
  "docs/release-audits/",
];

/** Every violation for a `{ path: bytes }` map, as sentences. */
export function violations(sizes) {
  const found = [];
  for (const [path, limit] of Object.entries(FILE_BUDGETS)) {
    if ((sizes[path] ?? 0) > limit) found.push(`${path} is ${sizes[path]} bytes (budget ${limit})`);
  }
  for (const group of GROUP_BUDGETS) {
    const total = Object.entries(sizes)
      .filter(([path]) => group.test(path))
      .reduce((sum, [, bytes]) => sum + bytes, 0);
    if (total > group.bytes) found.push(`${group.name} total ${total} bytes (budget ${group.bytes})`);
  }
  for (const [path, bytes] of Object.entries(sizes)) {
    if (path.startsWith("fixtures/") && path.endsWith("/README.md") && bytes > FIXTURE_README_MAX) {
      found.push(`${path} is ${bytes} bytes (a fixture README states its claim; budget ${FIXTURE_README_MAX})`);
    }
    if (
      path.startsWith("docs/") &&
      !DOCS_ALLOWED.some((allowed) => (allowed.endsWith("/") ? path.startsWith(allowed) : path === allowed))
    ) {
      found.push(`${path} is not an allowed docs/ location`);
    }
  }
  return found;
}

/** Tracked and untracked-but-not-ignored files under the budgeted roots. */
export function measuredSizes(root = ROOT) {
  const listed = execFileSync(
    "git",
    ["ls-files", "-z", "--cached", "--others", "--exclude-standard", "--", "AGENTS.md", "CLAUDE.md", "docs", "fixtures", ".claude/skills"],
    // Package-source fixtures ship whole published packages.
    { cwd: root, encoding: "utf8", maxBuffer: 256 * 1024 * 1024 },
  );
  const sizes = {};
  for (const path of listed.split("\0").filter(Boolean)) {
    if (path.startsWith("fixtures/") && !path.endsWith("/README.md")) continue;
    try {
      sizes[path] = statSync(resolve(root, path)).size;
    } catch {
      // Deleted in the working tree but still in the index: it costs nothing.
    }
  }
  return sizes;
}

if (process.argv[1] && resolve(process.argv[1]) === resolve(fileURLToPath(import.meta.url))) {
  const found = violations(measuredSizes());
  if (found.length > 0) {
    console.error(
      `doc-budget: ${found.length} violation(s):\n  ${found.join("\n  ")}\n` +
        "Slice reports go in the commit message, a design decision in docs/decisions.md, " +
        "an open item in docs/open-work.md. Raise a budget only as a deliberate edit to scripts/doc-budget.mjs.",
    );
    process.exit(1);
  }
  console.log("doc-budget: within budget");
}
