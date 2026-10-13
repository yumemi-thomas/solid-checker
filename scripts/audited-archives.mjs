// The published archives the negative claim authority quotes, installed
// byte-for-byte so the archive-reading arm of the citation test can run.
//
// `rust/crates/solid-dialect/audited-archives.json` lists every archive a
// dialect's `AuditedCitation::Implementation` rows cite, by name, version, SRI
// integrity, and the SHA-256 of its `package.json` -- and a Rust test pins that
// file to the dialect tables, so it is the authoritative list here too. Each
// archive is fetched with `npm pack` at its exact version, its tarball refused
// unless it hashes to the pinned integrity, and extracted to
//
//   rust/target/audited-archives/<dialect>/<version>/node_modules/<name>/
//
// which is exactly the `<root>/<package>/<archive path>` shape the test reads
// under `SOLID_CHECKER_RC<N>_ARCHIVE_ROOT`. Nothing here resolves a
// dependency: an archive is a quoted document, not a runtime, so each one is
// the tarball's own bytes and nothing else.
//
// Deliberately separate from the tsc-oracle install. The oracle installs the
// *audited release* (`fixtures/tsc-oracle/packages.json`) and moves when the
// audited release moves; an archive a row cites must stay exactly those bytes
// for as long as the row does, whichever release is audited.
//
//   bun scripts/audited-archives.mjs provision [--force]
//   bun scripts/audited-archives.mjs roots [--json]
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  renameSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
const MANIFEST = join(ROOT, "rust/crates/solid-dialect/audited-archives.json");
export const ARCHIVES_ROOT = join(ROOT, "rust/target/audited-archives");
const STAMP = ".solid-checker-archive.json";

const sha256 = (bytes) => createHash("sha256").update(bytes).digest("hex");
const sri512 = (bytes) => `sha512-${createHash("sha512").update(bytes).digest("base64")}`;

/** Every archive the manifest lists, with the directory it is extracted to. */
export const auditedArchives = (manifestPath = MANIFEST) => {
  const document = JSON.parse(readFileSync(manifestPath, "utf8"));
  if (document.schemaVersion !== 1) {
    throw new Error(`${manifestPath}: unsupported schemaVersion ${document.schemaVersion}`);
  }
  return document.dialects.flatMap(({ id, archives }) =>
    archives.map((archive) => {
      for (const field of ["name", "version", "integrity", "manifestSha256"]) {
        if (typeof archive[field] !== "string" || !archive[field]) {
          throw new Error(`${manifestPath}: ${id} archive is missing ${field}`);
        }
      }
      const root = join(ARCHIVES_ROOT, id, archive.version, "node_modules");
      return { dialect: id, ...archive, root, directory: join(root, archive.name) };
    }),
  );
};

/**
 * The `<root>` each release's archives share, keyed `<dialect>@<version>`.
 * One root per release, because the citation test reads one variable per
 * release and joins the package name onto it.
 */
export const archiveRoots = (archives = auditedArchives()) =>
  Object.fromEntries(archives.map(({ dialect, version, root }) => [`${dialect}@${version}`, root]));

/**
 * Whether `archive` is already extracted from the pinned tarball.
 *
 * The stamp records the integrity the tarball was verified against at
 * extraction; the `package.json` digest is re-hashed from disk every time, so a
 * tree that was swapped for another release fails here even with a stamp.
 */
const isProvisioned = (archive) => {
  const stampPath = join(archive.directory, STAMP);
  const manifestPath = join(archive.directory, "package.json");
  if (!existsSync(stampPath) || !existsSync(manifestPath)) return false;
  try {
    const stamp = JSON.parse(readFileSync(stampPath, "utf8"));
    return (
      stamp.integrity === archive.integrity &&
      sha256(readFileSync(manifestPath)) === archive.manifestSha256
    );
  } catch {
    return false;
  }
};

/** Fetch, verify, and extract one archive. Any mismatch is a hard failure. */
const install = (archive) => {
  const work = mkdtempSync(join(tmpdir(), "solid-checker-audited-archive-"));
  try {
    const spec = `${archive.name}@${archive.version}`;
    execFileSync("npm", ["pack", spec, "--pack-destination", work, "--silent"], {
      cwd: work,
      stdio: ["ignore", "ignore", "inherit"],
    });
    const tarballs = readdirSync(work).filter((name) => name.endsWith(".tgz"));
    if (tarballs.length !== 1) {
      throw new Error(`npm pack ${spec} produced ${tarballs.length} tarballs in ${work}`);
    }
    const tarball = join(work, tarballs[0]);
    const integrity = sri512(readFileSync(tarball));
    if (integrity !== archive.integrity) {
      throw new Error(
        `${spec}: the registry served ${integrity}, not the pinned ${archive.integrity}. ` +
          `A republished or substituted archive is never extracted.`,
      );
    }
    const staging = join(work, "extract");
    mkdirSync(staging);
    execFileSync("tar", ["-xzf", tarball, "-C", staging], { stdio: "inherit" });
    const extracted = join(staging, "package");
    const manifestDigest = sha256(readFileSync(join(extracted, "package.json")));
    if (manifestDigest !== archive.manifestSha256) {
      throw new Error(
        `${spec}: package.json hashes to ${manifestDigest}, not the pinned ${archive.manifestSha256}`,
      );
    }
    writeFileSync(
      join(extracted, STAMP),
      `${JSON.stringify({ name: archive.name, version: archive.version, integrity }, null, 2)}\n`,
    );
    rmSync(archive.directory, { recursive: true, force: true });
    mkdirSync(dirname(archive.directory), { recursive: true });
    renameSync(extracted, archive.directory);
  } finally {
    rmSync(work, { recursive: true, force: true });
  }
};

export const provisionArchives = ({ force = false, archives = auditedArchives() } = {}) => {
  for (const archive of archives) {
    const label = `${archive.name}@${archive.version}`;
    if (!force && isProvisioned(archive)) {
      console.error(`already provisioned ${label}`);
      continue;
    }
    install(archive);
    if (!isProvisioned(archive)) throw new Error(`${label} did not verify after extraction`);
    console.error(`provisioned ${label} (${archive.integrity})`);
  }
  return archiveRoots(archives);
};

const main = () => {
  const [command, ...rest] = process.argv.slice(2);
  if (command === "provision") {
    provisionArchives({ force: rest.includes("--force") });
    return;
  }
  if (command === "roots") {
    const roots = archiveRoots();
    if (rest.includes("--json")) console.log(JSON.stringify(roots, null, 2));
    else for (const [release, root] of Object.entries(roots)) console.log(`${release}\t${root}`);
    return;
  }
  console.error("usage: bun scripts/audited-archives.mjs provision [--force] | roots [--json]");
  process.exit(2);
};

if (process.argv[1] && resolve(process.argv[1]) === resolve(fileURLToPath(import.meta.url))) {
  try {
    main();
  } catch (error) {
    console.error(String(error.message ?? error));
    process.exit(1);
  }
}
