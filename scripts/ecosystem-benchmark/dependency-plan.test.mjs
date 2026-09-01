import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import {
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  symlinkSync,
  writeFileSync
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "vitest";

import { planRecursiveDependencies } from "./lib/dependency-plan.mjs";
import { ArtifactResolutionError } from "../../packages/cli/scripts/artifact-resolution.mjs";

const digest = bytes =>
  `sha256:${createHash("sha256").update(bytes).digest("hex")}`;

function packageRoot(project, name, version) {
  const root = join(project, "packages", `${name.replaceAll("/", "__")}-${version}`);
  mkdirSync(root, { recursive: true });
  writeFileSync(join(root, "package.json"), JSON.stringify({ name, version }));
  return root;
}

function installedPackageRoot(project, installedPath, name, version) {
  const root = join(project, ...installedPath.split("/"));
  mkdirSync(root, { recursive: true });
  writeFileSync(join(root, "package.json"), JSON.stringify({ name, version }));
  return root;
}

function fakeResolution({ graph }) {
  return ({ specifier, packageRoot: root, integrity }) => {
    const manifestBytes = readFileSync(join(root, "package.json"));
    const manifest = JSON.parse(manifestBytes);
    const dependencies = graph[`${manifest.name}@${manifest.version}:${specifier}`] ?? [];
    const fixedDigest = character => `sha256:${character.repeat(64)}`;
    return {
      specifier,
      requestedEntrypoint: specifier === manifest.name
        ? "."
        : `./${specifier.slice(manifest.name.length + 1)}`,
      packageName: manifest.name,
      packageVersion: manifest.version,
      packageIntegrity: integrity,
      packageRoot: root,
      runtime: { path: "./index.js", digest: fixedDigest("1") },
      declarations: { path: "./index.d.ts", digest: fixedDigest("2") },
      closure: {
        digest: fixedDigest("3"),
        entries: [{ role: "manifest", path: "./package.json", digest: digest(manifestBytes) }],
        hazards: dependencies.map(dependency => ({
          kind: "unaccepted-external-dependency",
          source: `./index.js:${dependency}`,
          importerPath: "./index.js",
          specifier: dependency,
          affectedExports: [],
          affectedDomains: ["return-semantics"]
        }))
      }
    };
  };
}

test("recursive dependency planning retains exact subpaths, versions, integrities, and terminal leaves", () => {
  const project = mkdtempSync(join(tmpdir(), "solid-checker-dependency-plan-"));
  try {
    const roots = {
      root: packageRoot(project, "root", "1.0.0"),
      dep: packageRoot(project, "@scope/dep", "2.0.0"),
      leaf: packageRoot(project, "leaf", "3.0.0")
    };
    const byName = new Map([["@scope/dep", roots.dep], ["leaf", roots.leaf]]);
    const graph = {
      "root@1.0.0:root": ["@scope/dep/subpath"],
      "@scope/dep@2.0.0:@scope/dep/subpath": ["leaf"]
    };
    const plan = planRecursiveDependencies({
      projectDir: project,
      rootPackageRoot: roots.root,
      rootPackage: "root",
      rootVersion: "1.0.0",
      rootIntegrity: "sha512-root",
      artifactCases: [{ entrypoint: ".", conditions: ["browser"] }],
      resolveClosure: fakeResolution({ graph }),
      locatePackage: (_importer, name) => byName.get(name),
      integrityForVersion: (_project, name, version) => `sha512-${name}@${version}`
    });

    assert.equal(plan.complete, true);
    assert.equal(plan.status, "exact-leaf-refusal");
    assert.equal(plan.nodes.length, 3);
    assert.deepEqual(plan.nodes.map(node => [node.package, node.version, node.entrypoint]).sort(), [
      ["@scope/dep", "2.0.0", "./subpath"],
      ["leaf", "3.0.0", "."],
      ["root", "1.0.0", "."]
    ]);
    assert.equal(plan.edges.length, 2);
    assert.deepEqual(
      plan.leaves.filter(leaf => leaf.kind === "authenticated-receipt-unavailable")
        .map(leaf => leaf.package),
      ["leaf"]
    );
    assert.equal(plan.graphDigest, digest(JSON.stringify({
      roots: plan.roots,
      nodes: plan.nodes,
      edges: plan.edges,
      cycles: plan.cycles,
      leaves: plan.leaves
    })), "an empty additive conditional census does not perturb existing graph identities");
  } finally {
    rmSync(project, { recursive: true, force: true });
  }
});

test("an absent dynamic optional peer is conditional planning evidence, not a refusal leaf", () => {
  const project = mkdtempSync(join(tmpdir(), "solid-checker-optional-peer-plan-"));
  try {
    const root = packageRoot(project, "root", "1.0.0");
    const resolver = fakeResolution({ graph: { "root@1.0.0:root": [] } });
    const plan = planRecursiveDependencies({
      projectDir: project,
      rootPackageRoot: root,
      rootPackage: "root",
      rootVersion: "1.0.0",
      rootIntegrity: "sha512-root",
      artifactCases: [{ entrypoint: ".", conditions: [] }],
      resolveClosure: input => {
        const resolved = resolver(input);
        resolved.closure.hazards = [{
          kind: "unaccepted-external-dependency",
          source: "./index.js:optional-peer",
          importerPath: "./index.js",
          specifier: "optional-peer",
          affectedExports: [],
          affectedDomains: ["return-semantics"],
          optionalPeer: true,
          dynamicImport: true
        }];
        return resolved;
      },
      locatePackage: () => {
        throw new ArtifactResolutionError("package-not-found", "absent");
      },
      pathExists: () => false,
      integrityForVersion: () => null
    });

    assert.equal(plan.status, "conditional-only");
    assert.deepEqual(plan.leaves, []);
    assert.deepEqual(plan.conditionalDependencies.map(({ id: _id, ...entry }) => entry), [{
      kind: "absent-optional-peer",
      node: plan.roots[0].node,
      specifier: "optional-peer"
    }]);
    assert.deepEqual(plan.edges, [{
      from: plan.roots[0].node,
      specifier: "optional-peer",
      to: null,
      conditional: "absent-optional-peer"
    }]);
    assert.equal(plan.graphDigest, digest(JSON.stringify({
      roots: plan.roots,
      nodes: plan.nodes,
      edges: plan.edges,
      cycles: plan.cycles,
      leaves: plan.leaves,
      conditionalDependencies: plan.conditionalDependencies
    })), "a nonempty conditional census is identity-bearing");
  } finally {
    rmSync(project, { recursive: true, force: true });
  }
});

test("optional-peer planning preserves present, inaccessible, and static failures", () => {
  const project = mkdtempSync(join(tmpdir(), "solid-checker-optional-peer-failures-"));
  try {
    const root = packageRoot(project, "root", "1.0.0");
    const resolver = fakeResolution({ graph: { "root@1.0.0:root": [] } });
    const planFor = ({ hazards, pathExists }) => planRecursiveDependencies({
      projectDir: project,
      rootPackageRoot: root,
      rootPackage: "root",
      rootVersion: "1.0.0",
      rootIntegrity: "sha512-root",
      artifactCases: [{ entrypoint: ".", conditions: [] }],
      resolveClosure: input => {
        const resolved = resolver(input);
        resolved.closure.hazards = hazards;
        return resolved;
      },
      locatePackage: () => {
        throw new ArtifactResolutionError("package-not-found", "unresolved");
      },
      pathExists,
      integrityForVersion: () => null
    });
    const hazard = (dynamicImport, optionalPeer = true) => ({
      kind: "unaccepted-external-dependency",
      source: "./index.js:optional-peer",
      importerPath: "./index.js",
      specifier: "optional-peer",
      affectedExports: [],
      affectedDomains: ["return-semantics"],
      optionalPeer,
      dynamicImport
    });

    const present = planFor({ hazards: [hazard(true)], pathExists: () => true });
    assert.equal(present.status, "exact-leaf-refusal");
    assert.deepEqual(present.conditionalDependencies, []);
    assert.equal(present.leaves[0].kind, "dependency-identity");

    const inaccessibleError = Object.assign(new Error("permission denied"), { code: "EACCES" });
    const inaccessible = planFor({
      hazards: [hazard(true)],
      pathExists: () => { throw inaccessibleError; }
    });
    assert.equal(inaccessible.status, "exact-leaf-refusal");
    assert.deepEqual(inaccessible.conditionalDependencies, []);
    assert.equal(inaccessible.leaves[0].code, "planner-error");

    const mixed = planFor({
      hazards: [hazard(false, false), hazard(true)],
      pathExists: () => false
    });
    assert.equal(mixed.status, "exact-leaf-refusal");
    assert.equal(mixed.leaves.length, 1, "the static edge remains a refusal");
    assert.equal(mixed.conditionalDependencies.length, 1, "only the dynamic optional edge is conditional");
    assert.equal(mixed.edges.filter(edge => edge.conditional).length, 1);
    assert.equal(mixed.edges.filter(edge => !edge.conditional).length, 1);
  } finally {
    rmSync(project, { recursive: true, force: true });
  }
});

test("colon-bearing importer paths cannot redirect optional-peer absence proof", () => {
  const project = mkdtempSync(join(tmpdir(), "solid-checker-colon-importer-plan-"));
  try {
    const root = packageRoot(project, "root", "1.0.0");
    const resolver = fakeResolution({ graph: { "root@1.0.0:root": [] } });
    const plan = planRecursiveDependencies({
      projectDir: project,
      rootPackageRoot: root,
      rootPackage: "root",
      rootVersion: "1.0.0",
      rootIntegrity: "sha512-root",
      artifactCases: [{ entrypoint: ".", conditions: [] }],
      resolveClosure: input => {
        const resolved = resolver(input);
        resolved.closure.hazards = [{
          kind: "unaccepted-external-dependency",
          source: "./dist/chunk:browser.js:optional-peer",
          importerPath: "./dist/chunk:browser.js",
          specifier: "optional-peer",
          affectedExports: [],
          affectedDomains: ["return-semantics"],
          optionalPeer: true,
          dynamicImport: true
        }];
        return resolved;
      },
      locatePackage: (_importer, name) => {
        assert.equal(name, "optional-peer");
        throw new ArtifactResolutionError("package-not-found", "present but broken");
      },
      pathExists: candidate => candidate.endsWith("/node_modules/optional-peer"),
      integrityForVersion: () => null
    });

    assert.equal(plan.status, "exact-leaf-refusal");
    assert.deepEqual(plan.conditionalDependencies, []);
    assert.equal(plan.leaves[0].kind, "dependency-identity");
    assert.equal(plan.leaves[0].specifier, "optional-peer");
  } finally {
    rmSync(project, { recursive: true, force: true });
  }
});

test("recursive dependency planning binds integrity to the exact installed Bun locator", () => {
  const project = mkdtempSync(join(tmpdir(), "solid-checker-dependency-lock-locator-"));
  try {
    const root = installedPackageRoot(project, "node_modules/root", "root", "1.0.0");
    const parent = installedPackageRoot(project, "node_modules/parent", "parent", "1.0.0");
    installedPackageRoot(project, "node_modules/leaf", "leaf", "2.0.0");
    const nestedLeaf = installedPackageRoot(
      project,
      "node_modules/parent/node_modules/leaf",
      "leaf",
      "2.0.0"
    );
    writeFileSync(join(project, "bun.lock"), JSON.stringify({
      packages: {
        root: ["root@1.0.0", "", {}, "sha512-root"],
        parent: ["parent@1.0.0", "", {}, "sha512-parent"],
        leaf: ["leaf@2.0.0", "", {}, "sha512-top-leaf"],
        "parent/leaf": ["leaf@2.0.0", "", {}, "sha512-nested-leaf"]
      }
    }));
    const graph = {
      "root@1.0.0:root": ["parent"],
      "parent@1.0.0:parent": ["leaf"]
    };
    const plan = planRecursiveDependencies({
      projectDir: project,
      rootPackageRoot: root,
      rootPackage: "root",
      rootVersion: "1.0.0",
      rootIntegrity: "sha512-root",
      artifactCases: [{ entrypoint: ".", conditions: [] }],
      resolveClosure: fakeResolution({ graph }),
      locatePackage: (_importer, name) => name === "parent" ? parent : nestedLeaf
    });

    assert.equal(
      plan.nodes.find(node => node.package === "leaf")?.integrity,
      "sha512-nested-leaf"
    );
  } finally {
    rmSync(project, { recursive: true, force: true });
  }
});

test("recursive dependency planning records cycles and fails closed at resource budgets", () => {
  const project = mkdtempSync(join(tmpdir(), "solid-checker-dependency-cycle-"));
  try {
    const root = packageRoot(project, "root", "1.0.0");
    const dep = packageRoot(project, "dep", "1.0.0");
    const byName = new Map([["root", root], ["dep", dep]]);
    const graph = {
      "root@1.0.0:root": ["dep"],
      "dep@1.0.0:dep": ["root"]
    };
    const common = {
      projectDir: project,
      rootPackageRoot: root,
      rootPackage: "root",
      rootVersion: "1.0.0",
      rootIntegrity: "sha512-root",
      artifactCases: [{ entrypoint: ".", conditions: [] }],
      resolveClosure: fakeResolution({ graph }),
      locatePackage: (_importer, name) => byName.get(name),
      integrityForVersion: (_project, name, version) => `sha512-${name}@${version}`
    };
    const cyclic = planRecursiveDependencies(common);
    assert.equal(cyclic.complete, true);
    assert.equal(cyclic.cycles.length, 1);
    assert.equal(cyclic.status, "cycle-refusal");
    assert.deepEqual(cyclic.conditionalDependencies, []);

    const bounded = planRecursiveDependencies({ ...common, maxNodes: 1 });
    assert.equal(bounded.complete, false);
    assert.equal(bounded.status, "resource-refusal");
    assert.ok(bounded.leaves.some(leaf => leaf.kind === "node-budget"));
  } finally {
    rmSync(project, { recursive: true, force: true });
  }
});

test("recursive dependency planning resolves each exact installed closure once without merging unequal conditions", () => {
  const project = mkdtempSync(join(tmpdir(), "solid-checker-dependency-memo-"));
  try {
    const root = packageRoot(project, "root", "1.0.0");
    const calls = [];
    const resolveClosure = input => {
      calls.push({
        importer: input.importer,
        specifier: input.specifier,
        packageRoot: input.packageRoot,
        integrity: input.integrity,
        conditions: [...input.conditions]
      });
      return fakeResolution({ graph: {} })(input);
    };

    const plan = planRecursiveDependencies({
      projectDir: project,
      rootPackageRoot: root,
      rootPackage: "root",
      rootVersion: "1.0.0",
      rootIntegrity: "sha512-root",
      artifactCases: [
        { entrypoint: ".", conditions: ["browser"] },
        { entrypoint: ".", conditions: ["browser"] },
        { entrypoint: ".", conditions: ["worker"] }
      ],
      resolveClosure
    });

    assert.equal(plan.roots.length, 3, "every artifact case remains represented");
    assert.equal(plan.nodes.length, 2, "unequal condition programs remain distinct");
    assert.deepEqual(
      calls.map(call => call.conditions),
      [["browser"], ["worker"]],
      "only byte-identical resolution inputs may share one untrusted planning result"
    );
  } finally {
    rmSync(project, { recursive: true, force: true });
  }
});

test("recursive dependency planning invalidates a memoized closure when source bytes change", () => {
  const project = mkdtempSync(join(tmpdir(), "solid-checker-dependency-source-mutation-"));
  try {
    const root = packageRoot(project, "root", "1.0.0");
    const sourcePath = join(root, "source.js");
    writeFileSync(sourcePath, "export const value = 1;\n");
    let calls = 0;
    const plan = planRecursiveDependencies({
      projectDir: project,
      rootPackageRoot: root,
      rootPackage: "root",
      rootVersion: "1.0.0",
      rootIntegrity: "sha512-root",
      artifactCases: [
        { entrypoint: ".", conditions: [] },
        { entrypoint: ".", conditions: [] }
      ],
      resolveClosure: input => {
        calls += 1;
        const sourceBytes = readFileSync(sourcePath);
        const sourceDigest = digest(sourceBytes);
        const resolved = fakeResolution({ graph: {} })(input);
        resolved.closure.entries.push({
          role: "runtime",
          path: "./source.js",
          digest: sourceDigest
        });
        resolved.closure.digest = sourceDigest;
        if (calls === 1) writeFileSync(sourcePath, "export const value = 2;\n");
        return resolved;
      }
    });

    assert.equal(calls, 2, "a changed source program must be reacquired");
    assert.equal(plan.nodes.length, 2);
    assert.notEqual(plan.roots[0].node, plan.roots[1].node);
  } finally {
    rmSync(project, { recursive: true, force: true });
  }
});

test("recursive dependency planning revalidates the current symlink target before memo reuse", () => {
  const project = mkdtempSync(join(tmpdir(), "solid-checker-dependency-symlink-mutation-"));
  try {
    const root = packageRoot(project, "root", "1.0.0");
    const firstTarget = join(root, "source-first.js");
    const secondTarget = join(root, "source-second.js");
    const sourcePath = join(root, "source.js");
    writeFileSync(firstTarget, "export const value = 1;\n");
    writeFileSync(secondTarget, "export const value = 2;\n");
    symlinkSync("./source-first.js", sourcePath);
    let calls = 0;
    planRecursiveDependencies({
      projectDir: project,
      rootPackageRoot: root,
      rootPackage: "root",
      rootVersion: "1.0.0",
      rootIntegrity: "sha512-root",
      artifactCases: [
        { entrypoint: ".", conditions: [] },
        { entrypoint: ".", conditions: [] }
      ],
      resolveClosure: input => {
        calls += 1;
        const sourceBytes = readFileSync(sourcePath);
        const resolved = fakeResolution({ graph: {} })(input);
        resolved.closure.entries.push({
          role: "runtime",
          path: "./source.js",
          digest: digest(sourceBytes)
        });
        resolved.closure.digest = digest(sourceBytes);
        if (calls === 1) {
          rmSync(sourcePath);
          symlinkSync("./source-second.js", sourcePath);
        }
        return resolved;
      }
    });

    assert.equal(calls, 2, "a changed symlink target must be reacquired");
  } finally {
    rmSync(project, { recursive: true, force: true });
  }
});

test("recursive dependency planning reuses a supplied exact package root across parent importers", () => {
  const project = mkdtempSync(join(tmpdir(), "solid-checker-dependency-diamond-"));
  try {
    const roots = Object.fromEntries(
      ["root", "left", "right", "leaf"].map(name => [
        name,
        packageRoot(project, name, "1.0.0")
      ])
    );
    const byName = new Map(Object.entries(roots));
    const graph = {
      "root@1.0.0:root": ["left", "right"],
      "left@1.0.0:left": ["leaf"],
      "right@1.0.0:right": ["leaf"]
    };
    const calls = [];
    const resolver = fakeResolution({ graph });
    const plan = planRecursiveDependencies({
      projectDir: project,
      rootPackageRoot: roots.root,
      rootPackage: "root",
      rootVersion: "1.0.0",
      rootIntegrity: "sha512-root",
      artifactCases: [{ entrypoint: ".", conditions: [] }],
      resolveClosure: input => {
        calls.push(input.specifier);
        return resolver(input);
      },
      locatePackage: (_importer, name) => byName.get(name),
      integrityForVersion: (_project, name, version) => `sha512-${name}@${version}`
    });

    assert.equal(plan.nodes.length, 4);
    assert.equal(plan.edges.filter(edge => edge.specifier === "leaf").length, 2);
    assert.equal(
      calls.filter(specifier => specifier === "leaf").length,
      1,
      "an exact supplied root makes the parent importer non-semantic"
    );
  } finally {
    rmSync(project, { recursive: true, force: true });
  }
});

test("recursive dependency planning relocates the nearest installed dependency on every edge", () => {
  const project = mkdtempSync(join(tmpdir(), "solid-checker-dependency-layout-mutation-"));
  try {
    const root = packageRoot(project, "root", "1.0.0");
    const firstLeaf = packageRoot(project, "leaf", "1.0.0");
    const secondLeaf = packageRoot(project, "leaf", "2.0.0");
    const resolver = fakeResolution({ graph: { "root@1.0.0:root": ["leaf"] } });
    let locations = 0;
    const plan = planRecursiveDependencies({
      projectDir: project,
      rootPackageRoot: root,
      rootPackage: "root",
      rootVersion: "1.0.0",
      rootIntegrity: "sha512-root",
      artifactCases: [
        { entrypoint: ".", conditions: ["browser"] },
        { entrypoint: ".", conditions: ["worker"] }
      ],
      resolveClosure: resolver,
      locatePackage: () => (locations++ === 0 ? firstLeaf : secondLeaf),
      integrityForVersion: (_project, name, version) => `sha512-${name}@${version}`
    });

    assert.equal(locations, 2, "nearest-package layout is a live input for every edge");
    assert.deepEqual(
      plan.nodes
        .filter(node => node.package === "leaf")
        .map(node => node.version)
        .sort(),
      ["1.0.0", "2.0.0"]
    );
  } finally {
    rmSync(project, { recursive: true, force: true });
  }
});

test("recursive dependency planning rereads a dependency manifest before selecting integrity", () => {
  const project = mkdtempSync(join(tmpdir(), "solid-checker-dependency-identity-mutation-"));
  try {
    const root = packageRoot(project, "root", "1.0.0");
    const leaf = packageRoot(project, "leaf", "1.0.0");
    const resolver = fakeResolution({ graph: { "root@1.0.0:root": ["leaf"] } });
    let leafResolutions = 0;
    const plan = planRecursiveDependencies({
      projectDir: project,
      rootPackageRoot: root,
      rootPackage: "root",
      rootVersion: "1.0.0",
      rootIntegrity: "sha512-root",
      artifactCases: [
        { entrypoint: ".", conditions: ["browser"] },
        { entrypoint: ".", conditions: ["worker"] }
      ],
      resolveClosure: input => {
        const resolved = resolver(input);
        if (resolved.packageName === "leaf" && leafResolutions++ === 0) {
          writeFileSync(
            join(leaf, "package.json"),
            JSON.stringify({ name: "leaf", version: "2.0.0" })
          );
        }
        return resolved;
      },
      locatePackage: () => leaf,
      integrityForVersion: (_project, name, version) => `sha512-${name}@${version}`
    });

    assert.deepEqual(
      plan.nodes
        .filter(node => node.package === "leaf")
        .map(node => [node.version, node.integrity])
        .sort(),
      [
        ["1.0.0", "sha512-leaf@1.0.0"],
        ["2.0.0", "sha512-leaf@2.0.0"]
      ]
    );
  } finally {
    rmSync(project, { recursive: true, force: true });
  }
});

test("recursive dependency planning retries an acquisition failure without revalidatable source inputs", () => {
  const project = mkdtempSync(join(tmpdir(), "solid-checker-dependency-refusal-"));
  try {
    const root = packageRoot(project, "root", "1.0.0");
    let calls = 0;
    const plan = planRecursiveDependencies({
      projectDir: project,
      rootPackageRoot: root,
      rootPackage: "root",
      rootVersion: "1.0.0",
      rootIntegrity: "sha512-root",
      artifactCases: [
        { entrypoint: ".", conditions: [] },
        { entrypoint: ".", conditions: [] }
      ],
      resolveClosure: () => {
        calls += 1;
        throw new Error("exact missing bytes");
      }
    });

    assert.equal(calls, 2);
    assert.deepEqual(plan.roots.map(rootCase => rootCase.node), [null, null]);
    assert.equal(plan.leaves.length, 1, "the stable refusal remains canonically deduplicated");
    assert.match(plan.leaves[0].reason, /exact missing bytes/);
  } finally {
    rmSync(project, { recursive: true, force: true });
  }
});
