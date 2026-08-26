import { readFileSync } from "node:fs";
import { join } from "node:path";

const parseJsonLike = source => {
  try {
    return JSON.parse(source);
  } catch {}

  // Bun's lockfile is JSON with trailing commas. Remove only commas that
  // occur outside strings and directly precede a closing object/array so
  // package metadata containing punctuation remains untouched.
  let normalized = "";
  let inString = false;
  let escaped = false;
  for (let index = 0; index < source.length; index += 1) {
    const character = source[index];
    if (inString) {
      normalized += character;
      if (escaped) escaped = false;
      else if (character === "\\") escaped = true;
      else if (character === '"') inString = false;
      continue;
    }
    if (character === '"') {
      inString = true;
      normalized += character;
      continue;
    }
    if (character === ",") {
      const remainder = source.slice(index + 1);
      if (/^\s*[}\]]/.test(remainder)) continue;
    }
    normalized += character;
  }
  try {
    return JSON.parse(normalized);
  } catch {
    return null;
  }
};

const readJson = path => {
  try {
    return parseJsonLike(readFileSync(path, "utf8"));
  } catch {
    return null;
  }
};

const installedVersion = (projectDir, name) =>
  readJson(join(projectDir, "node_modules", ...name.split("/"), "package.json"))?.version ?? null;

/**
 * Reads the integrity Bun recorded for an installed package.
 *
 * Bun stores package records as `[identifier, resolution, metadata, integrity]`
 * in `bun.lock`, while npm stores `{ integrity }` under a `node_modules/` path.
 * Supporting the latter during the transition keeps old scratch projects
 * diagnosable, but all new installs are made by Bun and produce `bun.lock`.
 */
export function packageIntegrity(projectDir, name) {
  const bunLock = readJson(join(projectDir, "bun.lock"));
  const version = installedVersion(projectDir, name);
  for (const [key, record] of Object.entries(bunLock?.packages ?? {})) {
    if (!Array.isArray(record) || typeof record[3] !== "string") continue;
    const identifier = record[0];
    if (
      (key === name || key === `${name}@${version}` || identifier === `${name}@${version}`) &&
      (!version || identifier === `${name}@${version}` || key === name)
    ) {
      return record[3];
    }
  }

  const packageLock = readJson(join(projectDir, "package-lock.json"));
  return packageLock?.packages?.[`node_modules/${name}`]?.integrity ?? null;
}
