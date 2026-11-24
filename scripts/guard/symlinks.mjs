// node scripts/guard/symlinks.mjs: a tracked symbolic link must be a fixture's own, relative link (ADR 1210).
//
// Why. A committed link to an absolute path (ADR 1165's `node_modules` incident: a worktree's install linked into another worktree) works on
// the machine that made it and nowhere else: a clone gets a dangling link, a tool follows it out of the repository, and the gates read another
// tree's bytes. The only links the repository means to hold are the fixtures' workspace links (`node_modules/@ws/kernel -> ../../packages/kernel`),
// which model a package manager's linked install inside one fixture project.
//
// What it checks, from git's index (mode 120000; the link's target is the blob's text), not the disk, so a staged link fails before it is
// committed and a checkout without symlink support reads the same targets. A link fails unless all hold:
//   - its target is relative (no leading `/`, `~` or drive letter);
//   - it is under fixtures/, inside a fixture project (`fixtures/<group>/<project>/…`);
//   - its target, resolved from the link's directory, stays inside that same fixture project;
//   - its target is tracked: a file, a link, or a directory holding tracked files.
// There is no allowlist: a link that needs to be an exception is a decision, made by changing this rule.
import { execFileSync } from "node:child_process";
import { dirname, join, posix } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "../..");

/** The problems of the tracked links. `links`: `[{ path, target }]`; `tracked`: every tracked path. Returns `[{ path, target, problem }]`. */
export function checkSymlinks(links, tracked) {
  const trackedSet = new Set(tracked);
  const holds = (path) => trackedSet.has(path) || tracked.some((other) => other.startsWith(`${path}/`));
  const problems = [];
  for (const { path, target } of links) {
    const fail = (problem) => problems.push({ path, target, problem });
    if (/^(?:\/|~|[A-Za-z]:[\\/]|\\\\)/.test(target)) {
      fail("an absolute target: it exists on the machine that made it and nowhere else");
      continue;
    }
    const parts = path.split("/");
    if (parts[0] !== "fixtures" || parts.length < 4) {
      fail("a link outside a fixture project: only a fixture's workspace links are tracked");
      continue;
    }
    const project = parts.slice(0, 3).join("/");
    const resolved = posix.normalize(posix.join(posix.dirname(path), target.replaceAll("\\", "/")));
    if (resolved !== project && !resolved.startsWith(`${project}/`)) {
      fail(`a target outside its fixture project (${project}): resolves to ${resolved}`);
      continue;
    }
    if (!holds(resolved)) fail(`a target git does not track: ${resolved}`);
  }
  return problems;
}

/** The index's links and paths. */
export function indexLinks(root = ROOT) {
  const entries = execFileSync("git", ["ls-files", "-s", "-z"], { cwd: root, encoding: "utf8", maxBuffer: 1 << 28 })
    .split("\0")
    .filter(Boolean)
    .map((line) => {
      const [meta, path] = line.split("\t");
      const [mode, blob] = meta.split(" ");
      return { mode, blob, path };
    });
  const links = entries
    .filter((entry) => entry.mode === "120000")
    .map((entry) => ({ path: entry.path, target: execFileSync("git", ["cat-file", "-p", entry.blob], { cwd: root, encoding: "utf8" }) }));
  return { links, tracked: entries.map((entry) => entry.path) };
}

if (process.argv[1]?.endsWith("symlinks.mjs")) {
  const { links, tracked } = indexLinks();
  const problems = checkSymlinks(links, tracked);
  for (const { path, target, problem } of problems) console.error(`symlinks: ${path} -> ${target}: ${problem}`);
  console.log(`symlinks: ${links.length} tracked link(s), ${problems.length} problem(s)`);
  process.exit(problems.length ? 1 : 0);
}
