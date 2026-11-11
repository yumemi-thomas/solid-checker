// What was added to a corpus checkout and how it was made (the manifest's `apps.<app>.additions`).
//
// A corpus is a checkout of each app at its commit with the installs run without lifecycle scripts (scripts/corpus/rc21-manifest.json `rebuilt`), so what an app's own build or codegen would
// have produced (a workspace package's `dist`, Paraglide's messages, Panda's `styled-system`) is not there, and the checker meets an unresolved import where a user's checkout has a module.
// An addition is such a generated output, made by the app's own pinned tools and commands, offline, and recorded here: the tools and their versions, the commands, where it ran, what came
// out (a directory of files, counted and hashed) and how the build ended. Nothing else of the corpus changes: `checkAdditions` lists the files of the corpus newer than a marker (outside
// node_modules and .git) and fails on any that is not under a recorded output.
import { createHash } from "node:crypto";
import { existsSync, readFileSync, readdirSync, statSync } from "node:fs";
import { join, relative, sep } from "node:path";

export const ADDITION_KINDS = ["workspace-dist", "paraglide", "panda"];

const sha256 = (bytes) => createHash("sha256").update(bytes).digest("hex");

/** Every file under `dir` (relative paths with `/`), in code-unit order, symlinks followed as files only when they point at one. */
export function filesUnder(dir) {
  const found = [];
  const visit = (path) => {
    for (const entry of readdirSync(path, { withFileTypes: true })) {
      const full = join(path, entry.name);
      if (entry.isDirectory()) visit(full);
      else if (entry.isFile() || (entry.isSymbolicLink() && existsSync(full) && statSync(full).isFile())) found.push(relative(dir, full).split(sep).join("/"));
    }
  };
  visit(dir);
  return found.sort();
}

/** The count and the digest of a directory's files: sha256 over one line `<sha256 of the file> <path>\n` per file, in path order. */
export function hashTree(dir) {
  const lines = filesUnder(dir).map((path) => `${sha256(readFileSync(join(dir, path)))} ${path}\n`);
  return { files: lines.length, sha256: sha256(lines.join("")) };
}

/** Throws, naming the app and the addition, unless every addition of the manifest says what it is, what made it and what came out. */
export function assertManifestAdditions(manifest, manifestPath = "manifest") {
  const problems = [];
  const seen = new Set();
  for (const [app, entry] of Object.entries(manifest.apps ?? {})) {
    for (const addition of entry.additions ?? []) {
      const at = `app ${app} addition ${addition?.id ?? "(no id)"}`;
      const need = (ok, what) => { if (!ok) problems.push(`${at}: ${what}`); };
      need(typeof addition.id === "string" && addition.id !== "", "no id");
      need(!seen.has(`${app}/${addition.id}`), "id repeated");
      seen.add(`${app}/${addition.id}`);
      need(ADDITION_KINDS.includes(addition.kind), `kind is not one of ${ADDITION_KINDS.join(", ")}`);
      need(Array.isArray(addition.commands) && addition.commands.length > 0 && addition.commands.every((command) => typeof command === "string" && command !== ""), "no commands");
      need(typeof addition.cwd === "string", "no cwd");
      need(Array.isArray(addition.tools) && addition.tools.length > 0 && addition.tools.every((tool) => typeof tool?.name === "string" && typeof tool?.version === "string" && tool.version !== ""), "a tool without a name and an exact version");
      need(typeof addition.output === "string" && addition.output !== "" && !addition.output.startsWith("/") && !addition.output.split("/").includes(".."), "no relative output directory");
      need(Number.isInteger(addition.files) && addition.files > 0, "no file count");
      need(/^[0-9a-f]{64}$/.test(addition.sha256 ?? ""), "no sha256");
      need(addition.result && Number.isInteger(addition.result.exit), "no result");
    }
  }
  if (problems.length > 0) throw new Error(`${manifestPath}: a corpus addition is not recorded in full: ${problems.join("; ")}`);
}

/** Every recorded output of the manifest as `{ app, id, dir }`, `dir` relative to the apps root (`<app root>/<output>`; the app's directory is the project rows' `root`). */
export function recordedOutputs(manifest) {
  const roots = new Map();
  for (const row of manifest.projects ?? []) if (!roots.has(row.app)) roots.set(row.app, row.root.split("/")[0]);
  const outputs = [];
  for (const [app, entry] of Object.entries(manifest.apps ?? {})) {
    for (const addition of entry.additions ?? []) outputs.push({ app, id: addition.id, dir: `${roots.get(app) ?? app}/${addition.output}`, addition });
  }
  return outputs;
}

/**
 * The problems of a corpus against its manifest's additions: an output that is missing, whose file count or digest differs from the record, and (when `changed` lists the files of the corpus
 * newer than a marker, relative to the apps root) a changed file that lies under no recorded output.
 * @returns {string[]}
 */
export function checkAdditions({ manifest, appsRoot, changed = null }) {
  const problems = [];
  const outputs = recordedOutputs(manifest);
  for (const { app, id, dir, addition } of outputs) {
    const path = join(appsRoot, dir);
    if (!existsSync(path)) { problems.push(`${app} ${id}: ${dir} is missing`); continue; }
    const now = hashTree(path);
    if (now.files !== addition.files) problems.push(`${app} ${id}: ${dir} holds ${now.files} files, the record says ${addition.files}`);
    else if (now.sha256 !== addition.sha256) problems.push(`${app} ${id}: ${dir} has another digest than the record`);
  }
  if (changed) {
    const under = outputs.map(({ dir }) => `${dir}/`);
    for (const file of changed) if (!under.some((prefix) => file.startsWith(prefix))) problems.push(`changed outside the recorded additions: ${file}`);
  }
  return problems;
}

/** The files under `root` (outside node_modules and .git) newer than `markerPath`'s modification time, relative to `root` with `/`. */
export function changedSince(root, markerPath) {
  const since = statSync(markerPath).mtimeMs;
  const found = [];
  const visit = (path) => {
    for (const entry of readdirSync(path, { withFileTypes: true })) {
      if (entry.name === "node_modules" || entry.name === ".git") continue;
      const full = join(path, entry.name);
      if (entry.isDirectory()) visit(full);
      else if ((entry.isFile() || entry.isSymbolicLink()) && statSync(full, { throwIfNoEntry: false })?.mtimeMs > since) found.push(relative(root, full).split(sep).join("/"));
    }
  };
  visit(root);
  return found.sort();
}
