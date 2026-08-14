import { existsSync, readFileSync } from "node:fs";
import { join, resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const packageJson = JSON.parse(readFileSync(join(root, "package.json"), "utf8"));

if (packageJson.name !== "solid-checker") {
  throw new Error(`unexpected package name: ${packageJson.name}`);
}
if (!/^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/.test(packageJson.version)) {
  throw new Error(`invalid publish version: ${packageJson.version}`);
}
for (const command of ["solid-checker"]) {
  if (!existsSync(join(root, packageJson.bin[command]))) {
    throw new Error(`launcher for ${command} is missing`);
  }
}
if (
  packageJson.exports?.["./eslint"]?.types !== "./eslint.d.ts" ||
  packageJson.exports?.["./eslint"]?.default !== "./eslint.cjs"
) {
  throw new Error("solid-checker/eslint export is missing");
}
for (const file of ["eslint.cjs", "eslint.d.ts"]) {
  if (!existsSync(join(root, file))) {
    throw new Error(`ESLint and Oxlint adapter file is missing: ${file}`);
  }
}
if (packageJson.optionalDependencies) {
  const expected = `^${packageJson.version}`;
  for (const [name, version] of Object.entries(packageJson.optionalDependencies)) {
    if (!name.startsWith("@solid-checker/binding-") || version !== expected) {
      throw new Error(`invalid native optional dependency: ${name}@${version}`);
    }
  }
}
if (packageJson.dependencies) {
  const wasmVersion = packageJson.dependencies["solid-checker-wasm"];
  if (wasmVersion !== `^${packageJson.version}`) {
    throw new Error(`invalid solid-checker-wasm dependency: ${wasmVersion}`);
  }
}
