#!/usr/bin/env node
// What was added to the corpus checkouts (scripts/lib/corpus-additions.mjs), against the manifest's record.
//
//   node scripts/corpus-additions.mjs check [--manifest scripts/corpus/rc21-manifest.json] [--apps-root ../solid-checker-rc21-frozen/apps] [--since <marker file>]
//   node scripts/corpus-additions.mjs hash <dir>        the file count and digest of a directory, as the manifest records an output
//
// `check` verifies that every recorded output exists with its recorded file count and digest; with `--since` it also lists the files of the corpus newer than the marker (outside
// node_modules and .git) and fails on one that lies under no recorded output: the corpus has not changed in any other way. Exit 1 on a problem.
import { readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

import { assertManifestAdditions, changedSince, checkAdditions, hashTree } from "./lib/corpus-additions.mjs";

const repo = resolve(dirname(fileURLToPath(import.meta.url)), "..");

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const [command, ...rest] = process.argv.slice(2);
  if (command === "hash" && rest[0]) {
    process.stdout.write(`${JSON.stringify(hashTree(resolve(rest[0])))}\n`);
  } else if (command === "check") {
    const options = { manifest: join(repo, "scripts/corpus/rc21-manifest.json"), "apps-root": resolve(repo, "../solid-checker-rc21-frozen/apps"), since: null };
    for (let at = 0; at < rest.length; at += 2) {
      const key = rest[at].replace(/^--/, "");
      if (!(key in options) || rest[at + 1] === undefined) throw new Error(`unknown or incomplete option ${rest[at]}`);
      options[key] = rest[at + 1];
    }
    const manifest = JSON.parse(readFileSync(resolve(options.manifest), "utf8"));
    assertManifestAdditions(manifest, options.manifest);
    const appsRoot = resolve(options["apps-root"]);
    const changed = options.since ? changedSince(appsRoot, resolve(options.since)) : null;
    const problems = checkAdditions({ manifest, appsRoot, changed });
    for (const problem of problems) process.stderr.write(`${problem}\n`);
    const additions = Object.values(manifest.apps).reduce((total, entry) => total + (entry.additions?.length ?? 0), 0);
    process.stdout.write(`${additions} addition(s) recorded${changed ? `, ${changed.length} changed file(s) since the marker` : ""}: ${problems.length === 0 ? "ok" : `${problems.length} problem(s)`}\n`);
    process.exit(problems.length === 0 ? 0 : 1);
  } else {
    process.stderr.write("usage: node scripts/corpus-additions.mjs check [--manifest m] [--apps-root dir] [--since marker] | hash <dir>\n");
    process.exit(64);
  }
}
