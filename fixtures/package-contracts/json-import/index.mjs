// Mirrors the real defect: @solidjs/start@2.0.3's
// dist/shared/dev-toolbar/index.jsx imports its own package.json for the
// version string. The import target is legitimate ESM data, not JavaScript,
// so it must not make contract generation fail, and its fields read here
// are plain data -- no reactive read, no callback, no owner requirement.
import pkg from "./package.json";

export function packageName() {
  return pkg.name;
}

export const packageVersion = pkg.version;
