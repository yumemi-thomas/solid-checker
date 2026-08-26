#!/usr/bin/env bun

import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
  statSync,
  writeFileSync
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, relative, resolve } from "node:path";

const VERSION = "2.0.0-rc.3";
const PACKAGES = [
  {
    name: "solid-js",
    slug: "solid-js",
    metadataUrl: `https://registry.npmjs.org/solid-js/${VERSION}`,
    sourceManifest: "packages/solid/package.json"
  },
  {
    name: "@solidjs/signals",
    slug: "solidjs-signals",
    metadataUrl: `https://registry.npmjs.org/%40solidjs%2Fsignals/${VERSION}`,
    sourceManifest: "packages/signals/package.json"
  },
  {
    name: "@solidjs/web",
    slug: "solidjs-web",
    metadataUrl: `https://registry.npmjs.org/%40solidjs%2Fweb/${VERSION}`,
    sourceManifest: "packages/web/package.json"
  }
];

function sha(algorithm, bytes, encoding = "hex") {
  return createHash(algorithm).update(bytes).digest(encoding);
}

export function canonicalize(value) {
  if (Array.isArray(value)) return value.map(canonicalize);
  if (!value || typeof value !== "object") return value;
  return Object.fromEntries(
    Object.keys(value)
      .sort()
      .map(key => [key, canonicalize(value[key])])
  );
}

function targetKind(path, trace) {
  const key = trace.at(-1);
  if (key === "types" || /\.d\.(?:c|m)?ts$/.test(path)) return "declaration";
  if (/\.(?:c|m)?js$/.test(path)) return "runtime";
  return "other";
}

export function collectExportTargets(exportsMap) {
  const targets = [];
  const visit = (value, trace) => {
    if (typeof value === "string") {
      targets.push({ trace, target: value, kind: targetKind(value, trace), pattern: value.includes("*") });
      return;
    }
    if (!value || typeof value !== "object" || Array.isArray(value)) return;
    for (const [key, child] of Object.entries(value)) visit(child, [...trace, key]);
  };
  for (const [subpath, value] of Object.entries(exportsMap ?? {})) visit(value, [subpath]);
  return targets;
}

export function assertSafeArchiveEntries(entries) {
  for (const entry of entries) {
    if (!entry.startsWith("package/") || entry.startsWith("/") || entry.split("/").includes("..")) {
      throw new Error(`unsafe tar entry ${JSON.stringify(entry)}`);
    }
  }
}

function walkFiles(root) {
  const files = [];
  const visit = directory => {
    for (const entry of readdirSync(directory, { withFileTypes: true })) {
      const path = join(directory, entry.name);
      if (entry.isSymbolicLink()) throw new Error(`published package contains symlink ${path}`);
      if (entry.isDirectory()) visit(path);
      else if (entry.isFile()) files.push(path);
      else throw new Error(`published package contains unsupported entry ${path}`);
    }
  };
  visit(root);
  return files.sort((left, right) => relative(root, left).localeCompare(relative(root, right)));
}

function git(repo, args, encoding = "utf8") {
  return execFileSync("git", ["-C", repo, ...args], { encoding });
}

function sourceEvidence(repo, gitHead, sourceManifest, publishedManifest) {
  if (!repo) return null;
  git(repo, ["cat-file", "-e", `${gitHead}^{commit}`]);
  const sourceBytes = git(repo, ["show", `${gitHead}:${sourceManifest}`], null);
  if (!sourceBytes.equals(publishedManifest)) {
    throw new Error(`${sourceManifest} at ${gitHead} differs from the published package manifest`);
  }
  const upstreamNext = git(repo, ["rev-parse", "upstream/next"]).trim();
  try {
    git(repo, ["merge-base", "--is-ancestor", gitHead, upstreamNext]);
  } catch {
    throw new Error(`${gitHead} is not contained by the local upstream/next authority ${upstreamNext}`);
  }
  return {
    repository: "https://github.com/solidjs/solid.git",
    gitHead,
    sourceManifest,
    manifestByteIdentical: true,
    containedByUpstreamNext: true,
    upstreamNextAtAudit: upstreamNext,
    commit: {
      authoredAt: git(repo, ["show", "-s", "--format=%aI", gitHead]).trim(),
      subject: git(repo, ["show", "-s", "--format=%s", gitHead]).trim()
    }
  };
}

function renderMarkdown(audit) {
  const lines = [
    "# Solid 2 RC.3 published-artifact audit",
    "",
    `Captured at ${audit.capturedAt}. The registry tarballs are the runtime and declaration authority.`,
    "",
    "| Package | SRI verified | Tarball SHA-256 | Files | Unpacked bytes | Export targets |",
    "| --- | --- | --- | ---: | ---: | ---: |"
  ];
  for (const pkg of audit.packages) {
    lines.push(
      `| \`${pkg.name}@${pkg.version}\` | ${pkg.integrity.verified ? "yes" : "no"} | ` +
        `\`${pkg.tarball.sha256}\` | ${pkg.files.count} | ${pkg.files.unpackedBytes} | ` +
        `${pkg.exportTargets.length} |`
    );
  }
  lines.push("", `Shared registry gitHead: \`${audit.gitHead}\`.`);
  lines.push(
    "",
    "Every concrete export target exists. Wildcard type targets remain patterns and are covered by each package's complete file manifest."
  );
  lines.push(
    "",
    "The file-manifest digest binds every extracted regular file, but does not establish the separately resolved dependency/peer closure."
  );
  return `${lines.join("\n")}\n`;
}

function parseArgs(argv) {
  const options = { outputDir: null, solidRepo: null };
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--output-dir") options.outputDir = argv[++index];
    else if (arg === "--solid-repo") options.solidRepo = argv[++index];
    else if (arg === "-h" || arg === "--help") options.help = true;
    else throw new Error(`unknown argument ${arg}`);
  }
  if (!options.outputDir && !options.help) throw new Error("--output-dir is required");
  return options;
}

async function auditPackage(definition, scratch, outputDir, solidRepo) {
  const metadataResponse = await fetch(definition.metadataUrl, {
    headers: { accept: "application/json" }
  });
  if (!metadataResponse.ok) throw new Error(`${definition.metadataUrl} returned ${metadataResponse.status}`);
  const metadataText = await metadataResponse.text();
  const metadata = JSON.parse(metadataText);
  if (metadata.name !== definition.name || metadata.version !== VERSION) {
    throw new Error(`registry identity mismatch for ${definition.name}@${VERSION}`);
  }
  const tarballResponse = await fetch(metadata.dist.tarball);
  if (!tarballResponse.ok) throw new Error(`${metadata.dist.tarball} returned ${tarballResponse.status}`);
  const tarball = Buffer.from(await tarballResponse.arrayBuffer());
  const computedSri = `sha512-${sha("sha512", tarball, "base64")}`;
  const computedSha1 = sha("sha1", tarball);
  if (computedSri !== metadata.dist.integrity) throw new Error(`${definition.name} SRI mismatch`);
  if (computedSha1 !== metadata.dist.shasum) throw new Error(`${definition.name} SHA-1 mismatch`);

  const tarPath = join(scratch, `${definition.slug}.tgz`);
  const extractRoot = join(scratch, definition.slug);
  mkdirSync(extractRoot, { recursive: true });
  writeFileSync(tarPath, tarball);
  const entries = execFileSync("tar", ["-tzf", tarPath], { encoding: "utf8" })
    .split("\n")
    .filter(Boolean);
  assertSafeArchiveEntries(entries);
  execFileSync("tar", ["-xzf", tarPath, "-C", extractRoot]);

  const packageRoot = join(extractRoot, "package");
  const files = walkFiles(packageRoot).map(path => {
    const bytes = readFileSync(path);
    return { path: relative(packageRoot, path), sha256: sha("sha256", bytes), bytes: bytes.length };
  });
  const unpackedBytes = files.reduce((sum, file) => sum + file.bytes, 0);
  if (metadata.dist.fileCount !== undefined && files.length !== metadata.dist.fileCount) {
    throw new Error(`${definition.name} file-count mismatch: ${files.length} != ${metadata.dist.fileCount}`);
  }
  if (metadata.dist.unpackedSize !== undefined && unpackedBytes !== metadata.dist.unpackedSize) {
    throw new Error(`${definition.name} unpacked-size mismatch: ${unpackedBytes} != ${metadata.dist.unpackedSize}`);
  }

  const manifestBytes = readFileSync(join(packageRoot, "package.json"));
  const manifest = JSON.parse(manifestBytes);
  const exportTargets = collectExportTargets(manifest.exports).map(target => {
    if (target.pattern) return { ...target, exists: null, sha256: null };
    const local = join(packageRoot, target.target.replace(/^\.\//, ""));
    const exists = statSync(local, { throwIfNoEntry: false })?.isFile() ?? false;
    if (!exists) throw new Error(`${definition.name} export target is missing: ${target.target}`);
    return { ...target, exists: true, sha256: sha("sha256", readFileSync(local)) };
  });
  const fileManifestText = files.map(file => `${file.sha256}  ${file.path}\n`).join("");
  const source = sourceEvidence(
    solidRepo,
    metadata.gitHead,
    definition.sourceManifest,
    manifestBytes
  );

  const packageOutput = join(outputDir, definition.slug);
  mkdirSync(packageOutput, { recursive: true });
  writeFileSync(join(packageOutput, "registry-metadata.json"), `${JSON.stringify(metadata, null, 2)}\n`);
  writeFileSync(join(packageOutput, "package.json"), manifestBytes);
  writeFileSync(join(packageOutput, "exports.json"), `${JSON.stringify(manifest.exports, null, 2)}\n`);
  writeFileSync(join(packageOutput, "files.json"), `${JSON.stringify(files, null, 2)}\n`);

  return {
    name: definition.name,
    version: VERSION,
    registry: {
      metadataUrl: definition.metadataUrl,
      tarballUrl: metadata.dist.tarball,
      gitHead: metadata.gitHead,
      shasum: metadata.dist.shasum,
      integrity: metadata.dist.integrity,
      signatures: metadata.dist.signatures ?? [],
      attestations: metadata.dist.attestations ?? null
    },
    integrity: { verified: true, computedSha1, computedSri },
    tarball: { sha256: sha("sha256", tarball), bytes: tarball.length },
    manifest: {
      sha256: sha("sha256", manifestBytes),
      exportMapSha256: sha("sha256", JSON.stringify(canonicalize(manifest.exports)))
    },
    files: {
      count: files.length,
      unpackedBytes,
      manifestSha256: sha("sha256", fileManifestText)
    },
    exportTargets,
    source
  };
}

export async function runAudit({ outputDir, solidRepo = null }) {
  const destination = resolve(outputDir);
  mkdirSync(destination, { recursive: true });
  const scratch = mkdtempSync(join(tmpdir(), "solid-checker-rc3-audit-"));
  try {
    const packages = [];
    for (const definition of PACKAGES) {
      packages.push(await auditPackage(definition, scratch, destination, solidRepo));
    }
    const heads = new Set(packages.map(pkg => pkg.registry.gitHead));
    if (heads.size !== 1 || heads.has(undefined)) throw new Error("RC.3 tuple does not share one registry gitHead");
    const audit = {
      schemaVersion: 1,
      documentKind: "solid-rc3-published-artifact-audit",
      capturedAt: new Date().toISOString(),
      version: VERSION,
      gitHead: [...heads][0],
      packages
    };
    writeFileSync(join(destination, "audit.json"), `${JSON.stringify(audit, null, 2)}\n`);
    writeFileSync(join(destination, "audit.md"), renderMarkdown(audit));
    return audit;
  } finally {
    rmSync(scratch, { recursive: true, force: true });
  }
}

if (import.meta.main) {
  try {
    const options = parseArgs(process.argv.slice(2));
    if (options.help) console.log("Usage: bun scripts/audit-solid-rc3.mjs --output-dir <DIR> [--solid-repo <DIR>]");
    else {
      const audit = await runAudit(options);
      console.log(`audited ${audit.packages.length} Solid RC.3 packages at gitHead ${audit.gitHead}`);
    }
  } catch (error) {
    console.error(`audit-solid-rc3: ${error?.stack ?? error}`);
    process.exitCode = 1;
  }
}
