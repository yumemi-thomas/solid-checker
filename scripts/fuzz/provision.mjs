// Provisions what the fuzzer's own tests need beyond packages/cli: the fast path's pinned install (scripts/fuzz/install), the runtime
// oracle's (scripts/runtime-oracle-install, ADR 0453) and the pinned Playwright's headless shell. Idempotent, like the tsc oracle's
// `provision`: an install that already passes the pin check is left alone, so `make verify` on a provisioned machine spends
// milliseconds here and a fresh CI runner installs (`npm ci`: the lockfile's exact bytes, invariant 7).
//
//   bun scripts/fuzz/provision.mjs           install what is missing or not the pinned Solid, then verify
//   bun scripts/fuzz/provision.mjs --check   verify only; exit 1 naming the command that provisions what is missing
import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { ORACLE_INSTALL, pinnedBrowser } from "../playwright-browser.mjs";
import { DEFAULT_FAST_INSTALL, installedVersions, pinnedProblems } from "./fast.mjs";

const repo = resolve(dirname(fileURLToPath(import.meta.url)), "../..");

/** The pieces, each with how to tell it is provisioned (`problems()`: empty when it is) and the command that provisions it. */
export function requirements({ fastInstall = DEFAULT_FAST_INSTALL, oracleInstall = ORACLE_INSTALL, browser = pinnedBrowser } = {}) {
  // The oracle's install has no @solidjs/diagnostics (the fast path's dev-diagnostics reader); the rest is pinned the same.
  const solid = (install, { diagnostics = true } = {}) => () => pinnedProblems(installedVersions(install)).filter((problem) => diagnostics || !problem.startsWith("@solidjs/diagnostics"));
  const has = (install, ...files) => files.filter((file) => !existsSync(join(install, "node_modules", file))).map((file) => `${file} is not installed`);
  return [
    {
      name: "fuzz fast-path install",
      dir: fastInstall,
      problems: () => [...solid(fastInstall)(), ...has(fastInstall, "vitest/vitest.mjs", "vitest/dist/node.js", "happy-dom/package.json")],
      install: ["npm", "ci", "--no-audit", "--no-fund"],
    },
    {
      name: "runtime oracle install",
      dir: oracleInstall,
      problems: () => [...solid(oracleInstall, { diagnostics: false })(), ...has(oracleInstall, "playwright-core/browsers.json", "vite/package.json")],
      install: ["npm", "ci", "--no-audit", "--no-fund"],
    },
    {
      name: "pinned Playwright headless shell",
      dir: oracleInstall,
      // Needs the oracle install first (it names the revision), so a missing install reports that, not a second error here.
      problems: () => {
        if (!existsSync(join(oracleInstall, "node_modules/playwright-core/browsers.json"))) return [];
        try { browser({ install: oracleInstall }); return []; } catch (error) { return [error.message]; }
      },
      install: ["npx", "playwright", "install", "chromium-headless-shell"],
    },
  ];
}

/** The command a person runs to provision `requirement` by hand. */
export const manualCommand = (requirement) => `cd ${requirement.dir.replace(`${repo}/`, "")} && ${requirement.install.join(" ")}`;

/** Checks every requirement; returns the unmet ones with their problems. */
export function unmet(list = requirements()) {
  return list.map((requirement) => ({ requirement, problems: requirement.problems() })).filter(({ problems }) => problems.length);
}

/** Loud, for a lane that does not install: names each unmet requirement, why, and the command that provisions it. */
export function unmetMessage(found) {
  return `the fuzzer's tests need what is not provisioned:\n${found.map(({ requirement, problems }) => `  ${requirement.name}: ${problems.join("; ")}\n    provision: ${manualCommand(requirement)}`).join("\n")}\n(or: bun scripts/fuzz/provision.mjs)`;
}

function provision(list, run = spawnSync) {
  for (const requirement of list) {
    if (!requirement.problems().length) { console.log(`provisioned already: ${requirement.name}`); continue; }
    console.log(`provisioning ${requirement.name}: ${requirement.problems().join("; ")}`);
    const [command, ...args] = requirement.install;
    const result = run(command, args, { cwd: requirement.dir, stdio: "inherit" });
    if (result.status !== 0) throw new Error(`${requirement.name}: \`${manualCommand(requirement)}\` failed (${result.status ?? result.error})`);
  }
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const check = process.argv.includes("--check");
  try {
    const list = requirements();
    if (!check) provision(list);
    const found = unmet(list);
    if (found.length) throw new Error(unmetMessage(found));
    if (check) console.log("fuzz test requirements: provisioned");
  } catch (error) {
    console.error(`fuzz provision: ${error.message}`);
    process.exit(1);
  }
}
