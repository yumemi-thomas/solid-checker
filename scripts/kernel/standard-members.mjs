#!/usr/bin/env node
// ADR 1120: writes rust/crates/solid-facts/src/ast/standard_members.txt, every function-valued standard name: the methods and
// function-typed properties of every interface in TypeScript's lib files, the declared functions, and the declared global
// variables. A write under one of these keys on any receiver may put project code where standard code runs (the receiver may be a
// prototype, the window or an element by a route no scan names), so the plain-program premise refuses it by key.
//
//   node scripts/kernel/standard-members.mjs [--check]
import { createRequire } from "node:module";
import { readdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repo = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const require = createRequire(join(repo, "packages/cli/package.json"));
const ts = require("typescript");
const lib = join(dirname(require.resolve("typescript")), "");
const names = new Set();
for (const file of readdirSync(lib).filter((name) => /^lib\..*\.d\.ts$/.test(name)).sort()) {
  const source = ts.createSourceFile(file, readFileSync(join(lib, file), "utf8"), ts.ScriptTarget.Latest, true);
  const visit = (node) => {
    const name = node.name && ts.isIdentifier(node.name) ? node.name.text : null;
    if (name && (ts.isMethodSignature(node) || ts.isFunctionDeclaration(node) || ts.isMethodDeclaration(node))) names.add(name);
    if (name && ts.isPropertySignature(node) && node.type && (ts.isFunctionTypeNode(node.type) || (ts.isTypeReferenceNode(node.type) && /Handler|Listener|Callback/.test(node.type.getText(source))))) names.add(name);
    if (name && ts.isVariableDeclaration(node)) names.add(name);
    ts.forEachChild(node, visit);
  };
  visit(source);
}
const text = `${[...names].sort().join("\n")}\n`;
const target = join(repo, "rust/crates/solid-facts/src/ast/standard_members.txt");
if (process.argv.includes("--check")) {
  if (readFileSync(target, "utf8") !== text) {
    console.error(`standard-members: ${target} is stale (TypeScript ${ts.version}); run node scripts/kernel/standard-members.mjs`);
    process.exit(1);
  }
  console.log(`standard-members: ok (${names.size} names, TypeScript ${ts.version})`);
} else {
  writeFileSync(target, text);
  console.log(`standard-members: wrote ${names.size} names (TypeScript ${ts.version})`);
}
