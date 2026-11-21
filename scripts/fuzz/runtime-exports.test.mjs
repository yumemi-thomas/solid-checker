import { existsSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

// ADR 1148 (round 1, F2): the dialect audits the pinned runtime packages subpath by subpath (`Dialect::runtime_module_element_writes`: the frames tier morphs the
// children it finds with the server's attributes, `type` included, and a file-level exclusion of the runtime hid it). The audit's input is the frozen list of every
// subpath the pinned packages export, `rust/crates/solid-dialect/src/exports/solid_v2_runtime_subpaths.txt`; solid-dialect's test
// `every_exported_runtime_subpath_is_audited` holds the dialect's answers against that list, and this test holds the list against the installed packages. A release that
// adds a subpath changes the packages, fails here until the list is rewritten, and then fails there until the subpath is audited; at run time an unaudited subpath is
// unknown and withdraws the proof.
const root = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const FROZEN = join(root, "rust", "crates", "solid-dialect", "src", "exports", "solid_v2_runtime_subpaths.txt");
const INSTALLED = join(root, "scripts", "runtime-oracle-install", "node_modules");
const PACKAGES = ["solid-js", "@solidjs/signals", "@solidjs/web"];

/** The frozen file: the pinned versions, and each `<package> <subpath>` line. */
function frozen() {
  const pins = {};
  const subpaths = [];
  for (const raw of readFileSync(FROZEN, "utf8").split("\n")) {
    const line = raw.trim();
    if (line === "" || line.startsWith("#")) continue;
    const words = line.split(" ");
    if (words[0] === "pin" && words.length === 3) pins[words[1]] = words[2];
    else if (words.length === 2) subpaths.push(line);
    else throw new Error(`${FROZEN}: a line is \`<package> <subpath>\` or \`pin <package> <version>\`: ${line}`);
  }
  return { pins, subpaths };
}

const manifest = (name) => JSON.parse(readFileSync(join(INSTALLED, name, "package.json"), "utf8"));

describe("the audited runtime subpaths", () => {
  it("are read from the installed packages the oracle runs, which are installed", () => {
    for (const name of PACKAGES) expect(existsSync(join(INSTALLED, name, "package.json")), `${name} is not installed under ${INSTALLED}`).toBe(true);
  });

  it("name the versions the pinned install holds", () => {
    const { pins } = frozen();
    for (const name of PACKAGES) expect(pins[name], `${FROZEN} pins ${name}`).toBe(manifest(name).version);
  });

  it("are every subpath the pinned packages export, patterns as written", () => {
    const expected = PACKAGES.flatMap((name) => Object.keys(manifest(name).exports ?? {}).map((subpath) => `${name} ${subpath}`));
    expect(frozen().subpaths.toSorted()).toEqual(expected.toSorted());
  });
});
