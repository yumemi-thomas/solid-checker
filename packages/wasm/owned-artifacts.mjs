// The two reactors this package ships, read beside it and checked against its
// manifest (P5 S2). The digest check detects a corrupted or mismatched file
// in a package that is trusted as a unit; it authenticates nothing, and the
// producer handshake that follows checks compatibility only.
import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { OwnedCheckerError } from "./owned-client.mjs";

const ownedDirectory = new URL("./owned/", import.meta.url);

const own = bytes => bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength);

export async function loadArtifacts(directory = ownedDirectory) {
  let manifest;
  try {
    manifest = JSON.parse(await readFile(new URL("manifest.json", directory), "utf8"));
  } catch (error) {
    throw new OwnedCheckerError(`the owned artifacts' manifest cannot be read: ${error.message}`);
  }
  if (manifest?.format !== 1) throw new OwnedCheckerError("the owned artifacts' manifest has an unknown format");
  const artifacts = {};
  for (const role of ["checker", "producer"]) {
    const entry = manifest.artifacts?.[role];
    if (typeof entry?.file !== "string" || entry.file.includes("/") || typeof entry.sha256 !== "string") {
      throw new OwnedCheckerError(`the owned artifacts' manifest names no ${role}`);
    }
    const bytes = await readFile(new URL(entry.file, directory));
    const digest = createHash("sha256").update(bytes).digest("hex");
    if (digest !== entry.sha256) {
      throw new OwnedCheckerError(`owned/${entry.file} is sha256:${digest}, not the manifest's sha256:${entry.sha256}`);
    }
    artifacts[role] = own(bytes);
  }
  return artifacts;
}
