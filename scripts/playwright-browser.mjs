// The browser the runtime oracle and the fuzzer's Chrome host run when no `--browser` is given: the Chromium headless shell that the
// pinned Playwright (scripts/runtime-oracle-install) names in its `browsers.json`, at that exact revision (invariant 7). The newest
// build in the cache is not it: a Playwright of another project leaves its own builds there (on 2026-10-30 a cache held only
// revision 1248, Chrome 156, where Playwright 1.60.0 pins 1223, Chrome 148), and a run on it ran an unpinned browser.
// A missing pinned build stops the run; it never falls back to another one.
import { existsSync, readFileSync } from "node:fs";
import { homedir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repo = resolve(dirname(fileURLToPath(import.meta.url)), "..");
export const ORACLE_INSTALL = join(repo, "scripts/runtime-oracle-install");

/** Playwright's browser cache: `PLAYWRIGHT_BROWSERS_PATH`, else its per-platform default. */
export function browserCache({ home = homedir(), platform = process.platform, env = process.env } = {}) {
  if (env.PLAYWRIGHT_BROWSERS_PATH && env.PLAYWRIGHT_BROWSERS_PATH !== "0") return resolve(env.PLAYWRIGHT_BROWSERS_PATH);
  return platform === "darwin" ? join(home, "Library/Caches/ms-playwright") : platform === "win32" ? join(home, "AppData/Local/ms-playwright") : join(home, ".cache/ms-playwright");
}

/** The executable inside a headless-shell build directory, by the layouts Playwright's builds use per platform. */
const LAYOUTS = {
  darwin: ["chrome-headless-shell-mac-arm64/chrome-headless-shell", "chrome-headless-shell-mac-x64/chrome-headless-shell"],
  linux: ["chrome-headless-shell-linux64/chrome-headless-shell", "chrome-headless-shell-linux-arm64/chrome-headless-shell", "chrome-linux/headless_shell"],
};

/** What the pinned Playwright names: its version and the headless shell's revision and Chrome version. */
export function pinnedHeadlessShell(install = ORACLE_INSTALL) {
  const core = join(install, "node_modules/playwright-core");
  if (!existsSync(join(core, "browsers.json"))) throw new Error(`no Playwright in ${install}: run \`make runtime-oracle-install\``);
  const shell = JSON.parse(readFileSync(join(core, "browsers.json"), "utf8")).browsers.find((browser) => browser.name === "chromium-headless-shell");
  if (!shell) throw new Error(`${core}/browsers.json names no chromium-headless-shell`);
  return { playwright: JSON.parse(readFileSync(join(core, "package.json"), "utf8")).version, revision: shell.revision, browserVersion: shell.browserVersion, directory: `chromium_headless_shell-${shell.revision}` };
}

/** The pinned headless shell's build directory and executable; throws when the pinned build is not installed. */
export function pinnedBrowser({ install = ORACLE_INSTALL, home, platform = process.platform, env } = {}) {
  const pin = pinnedHeadlessShell(install);
  const build = join(browserCache({ home, platform, env }), pin.directory);
  const executable = (LAYOUTS[platform] ?? []).map((layout) => join(build, layout)).find((path) => existsSync(path));
  if (!executable) {
    throw new Error(`Playwright ${pin.playwright} pins chromium-headless-shell ${pin.revision} (Chrome ${pin.browserVersion}), not installed in ${build}: `
      + `run \`cd scripts/runtime-oracle-install && npx playwright install chromium-headless-shell\`, or pass --browser`);
  }
  return { ...pin, build, executable };
}
