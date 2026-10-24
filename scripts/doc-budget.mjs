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
  // Raised for ADR 0383's latent-key item.
  "docs/open-work.md": 26_700,
  // Raised for ADR 0378: the file stood 2 bytes under 113_200; for ADR 0379,
  // 14 bytes under 113_800; for ADR 0380, 3 bytes under 114_700; and for
  // ADRs 0382-0392.
  "docs/decisions.md": 128_600,
};

export const GROUP_BUDGETS = [
  { name: "docs/", test: (path) => path.startsWith("docs/"), bytes: 600_000 },
  {
    name: "fixture READMEs",
    test: (path) => path.startsWith("fixtures/") && path.endsWith("/README.md"),
    // Raised from 250 KB for the context-read fixtures (ADR 0352): the group was
    // 249.8 KB before they landed, and each case README is a claim the reader needs.
    // Raised again for ADR 0378's fixture: the group stood 1 byte under 256 KB.
    bytes: 257_000,
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
