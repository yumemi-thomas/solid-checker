import { existsSync, readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";

export function splitPackageSpecifier(specifier) {
  if (typeof specifier !== "string" || specifier === "" || specifier.startsWith(".")) {
    return null;
  }
  const parts = specifier.split("/");
  const packageName = specifier.startsWith("@")
    ? parts.slice(0, 2).join("/")
    : parts[0];
  if (!packageName || (specifier.startsWith("@") && parts.length < 2)) return null;
  const suffix = parts.slice(specifier.startsWith("@") ? 2 : 1).join("/");
  return {
    specifier,
    package: packageName,
    entrypoint: suffix ? `./${suffix}` : "."
  };
}

export function extractExternalModuleSpecifiers(text) {
  if (typeof text !== "string") return [];
  const found = new Set();
  const patterns = [
    /cannot statically expand external export-all "([^"]+)"/g,
    /accepted dependency (\S+) has no exact (?:runtime|declaration) binding/g,
    /dependency contract for (\S+) has no entrypoint/g,
    /PackageContract(?:EnvironmentDependent|ExportMissing)\s*\{[^}]*module:\s*"([^"]+)"/g,
    /"module"\s*:\s*"([^"]+)"/g
  ];
  for (const pattern of patterns) {
    for (const match of text.matchAll(pattern)) {
      if (splitPackageSpecifier(match[1])) found.add(match[1]);
    }
  }
  return [...found].sort();
}

export function isDependencyCompositionRefusalText(text) {
  return /cannot statically expand external export-all|unaccepted external dependency|accepted dependency contract|accepted dependency .* exact .* binding/i.test(
    text ?? ""
  );
}

export function readInstalledDependencyVersion({ projectDir, packageRoot, packageName }) {
  const project = resolve(projectDir);
  let directory = resolve(packageRoot);
  while (directory === project || directory.startsWith(`${project}/`)) {
    const manifestPath = join(directory, "node_modules", ...packageName.split("/"), "package.json");
    if (existsSync(manifestPath)) {
      try {
        const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
        if (manifest.name === packageName && typeof manifest.version === "string") {
          return manifest.version;
        }
      } catch {
        return null;
      }
    }
    if (directory === project) break;
    directory = dirname(directory);
  }
  return null;
}

export function collectExternalEdges({ texts, projectDir = null, packageRoot = null }) {
  const specifiers = new Set();
  for (const value of texts ?? []) {
    for (const specifier of extractExternalModuleSpecifiers(value)) specifiers.add(specifier);
  }
  return [...specifiers].sort().map(specifier => {
    const split = splitPackageSpecifier(specifier);
    return {
      ...split,
      resolvedVersion:
        projectDir && packageRoot
          ? readInstalledDependencyVersion({ projectDir, packageRoot, packageName: split.package })
          : null
    };
  });
}
