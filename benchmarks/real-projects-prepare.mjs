#!/usr/bin/env bun
// Prepares the real projects benchmarks/real-projects.mjs times (ADR 0423).
//
//   bun benchmarks/real-projects-prepare.mjs --root DIR [--projects a,b]
//
// Each project is fetched at its pinned commit (depth 1, by sha), patched with
// benchmarks/real-projects/<name>.patch (the Solid 2 release-candidate bumps),
// and installed with its recorded command. It is then verified: HEAD is the
// commit, the lockfile has the recorded digest, and the install left the
// working tree's diff byte-identical to the patch. A project whose stamp
// matches its manifest entry and patch is left alone.
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";

import { manifestErrors } from "./lib/real-projects.mjs";

const argv = process.argv.slice(2);
const option = name => {
  const index = argv.indexOf(`--${name}`);
  return index < 0 ? undefined : argv[index + 1];
};
const here = dirname(new URL(import.meta.url).pathname);
const manifest = JSON.parse(readFileSync(join(here, "real-projects", "manifest.json"), "utf8"));
const errors = manifestErrors(manifest);
if (errors.length) throw new Error(`manifest: ${errors.join("; ")}`);
const root = option("root");
if (!root) throw new Error("--root DIR is required");
const only = option("projects")?.split(",");

const sha256 = bytes => `sha256:${createHash("sha256").update(bytes).digest("hex")}`;

function run(command, args, cwd, { capture = false } = {}) {
  const result = spawnSync(command, args, { cwd, encoding: capture ? "buffer" : undefined, stdio: capture ? ["ignore", "pipe", "inherit"] : "inherit", maxBuffer: 1 << 30 });
  if (result.status !== 0) throw new Error(`${command} ${args.join(" ")} failed in ${cwd} (${result.status ?? result.signal})`);
  return result.stdout;
}

function verify(project, dir, patch) {
  const head = run("git", ["rev-parse", "HEAD"], dir, { capture: true }).toString().trim();
  if (head !== project.commit) throw new Error(`${project.name}: HEAD is ${head}, not ${project.commit}`);
  const lockfile = sha256(readFileSync(join(dir, project.lockfile)));
  if (lockfile !== project.lockfileDigest) throw new Error(`${project.name}: ${project.lockfile} is ${lockfile}, not ${project.lockfileDigest}`);
  const diff = run("git", ["diff", "--binary", "--no-color", "--no-ext-diff"], dir, { capture: true });
  if (!diff.equals(patch)) throw new Error(`${project.name}: the tree's diff is not the patch (the install rewrote a tracked file)`);
  for (const path of [project.tsconfig, project.editFile]) {
    if (!existsSync(join(dir, path))) throw new Error(`${project.name}: ${path} is missing`);
  }
}

mkdirSync(root, { recursive: true });
for (const project of manifest.projects.filter(project => !only || only.includes(project.name))) {
  const dir = resolve(root, project.name);
  const patchPath = join(here, "real-projects", project.patch);
  const patch = readFileSync(patchPath);
  const stamp = JSON.stringify({ ...project, patchDigest: sha256(patch) });
  const stampPath = `${dir}.prepared.json`;
  if (existsSync(stampPath) && readFileSync(stampPath, "utf8") === stamp && existsSync(dir)) {
    verify(project, dir, patch);
    console.error(`${project.name}: prepared`);
    continue;
  }
  rmSync(stampPath, { force: true });
  rmSync(dir, { recursive: true, force: true });
  mkdirSync(dir, { recursive: true });
  run("git", ["init", "--quiet"], dir);
  run("git", ["remote", "add", "origin", project.repo], dir);
  run("git", ["fetch", "--quiet", "--depth", "1", "origin", project.commit], dir);
  run("git", ["-c", "advice.detachedHead=false", "checkout", "--quiet", "--detach", "FETCH_HEAD"], dir);
  run("git", ["apply", "--whitespace=nowarn", patchPath], dir);
  const [command, ...args] = project.install.split(" ");
  run(command, args, dir);
  verify(project, dir, patch);
  writeFileSync(stampPath, stamp);
  console.error(`${project.name}: prepared at ${dir}`);
}
