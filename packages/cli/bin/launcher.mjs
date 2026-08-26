import { spawn, spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, join, parse, resolve } from "node:path";
import process from "node:process";

const packageRoot = resolve(import.meta.dirname, "..");
const suffix = process.platform === "win32" ? ".exe" : "";
const require = createRequire(import.meta.url);
const nativeInvocations = new Map();

function nativePackageRoot() {
  const target = {
    "darwin-arm64": "darwin-arm64",
    "linux-arm64": "linux-arm64-gnu",
    "linux-x64": "linux-x64-gnu",
    "win32-x64": "win32-x64-msvc"
  }[`${process.platform}-${process.arch}`];
  if (!target) return undefined;
  const packageName = `@solid-checker/binding-${target}`;
  try {
    return dirname(require.resolve(`${packageName}/package.json`));
  } catch {
    return undefined;
  }
}

function findRepository(start) {
  let directory = resolve(start);
  for (;;) {
    // `go.mod` used to mark the root; the producer moved to its own
    // repository, so the Rust workspace plus the Makefile identify it now.
    if (
      existsSync(join(directory, "Makefile")) &&
      existsSync(join(directory, "rust", "Cargo.toml"))
    ) {
      return directory;
    }
    const parent = dirname(directory);
    if (parent === directory || directory === parse(directory).root) return undefined;
    directory = parent;
  }
}

function packagedBinary(name) {
  const relative = join("native", `${process.platform}-${process.arch}`, `${name}${suffix}`);
  const dependencyRoot = nativePackageRoot();
  if (dependencyRoot) return join(dependencyRoot, relative);
  return join(packageRoot, relative);
}

function nativeInvocation(command) {
  const cached = nativeInvocations.get(command);
  if (cached) return cached;
  const repository = findRepository(packageRoot) ?? findRepository(process.cwd());
  const override = process.env.SOLID_CHECKER_NATIVE_BIN;
  let executable = override || packagedBinary(command);
  let developmentTypeFacts;

  if (!existsSync(executable) && repository) {
    executable = join(repository, "bin", `${command}-rust${suffix}`);
    developmentTypeFacts = join(repository, "bin", `solid-typefacts${suffix}`);
    if (!existsSync(executable) || !existsSync(developmentTypeFacts)) {
      const build = spawnSync("make", ["build-rust"], {
        cwd: repository,
        env: process.env,
        stdio: "inherit"
      });
      if (build.error) {
        console.error(`solid-checker: could not build Rust development binaries: ${build.error.message}`);
        process.exit(2);
      }
      if (build.status !== 0) process.exit(build.status ?? 2);
    }
  }

  if (!existsSync(executable)) {
    // darwin-x64 had a published binding and no longer does, so "install a
    // supported package" would send those users looking for something that is
    // not coming back. Building from a checkout still works there.
    const retired =
      process.platform === "darwin" && process.arch === "x64"
        ? "; macOS on Intel is no longer published — build from a checkout with `make build-rust`, " +
          "or stay on a release that still shipped a darwin-x64 binding"
        : "; set SOLID_CHECKER_NATIVE_BIN or install a supported package";
    console.error(
      `solid-checker: no ${command} binary for ${process.platform}-${process.arch}${retired}`
    );
    process.exit(2);
  }

  const env = { ...process.env };
  if (!env.SOLID_TYPEFACTS_BIN) {
    const packagedTypeFacts = packagedBinary("solid-typefacts");
    if (existsSync(packagedTypeFacts)) {
      env.SOLID_TYPEFACTS_BIN = packagedTypeFacts;
    } else if (developmentTypeFacts && existsSync(developmentTypeFacts)) {
      env.SOLID_TYPEFACTS_BIN = developmentTypeFacts;
    }
  }
  const invocation = { executable, env };
  nativeInvocations.set(command, invocation);
  return invocation;
}

export function runNative(command, args, options = {}) {
  const { executable, env } = nativeInvocation(command);
  return spawnSync(executable, args, {
    cwd: options.cwd ?? process.cwd(),
    env: { ...env, ...options.env },
    stdio: options.stdio ?? "inherit",
    encoding: options.encoding
  });
}

export function runNativeAsync(command, args, options = {}) {
  const { executable, env } = nativeInvocation(command);
  return new Promise(resolvePromise => {
    const child = spawn(executable, args, {
      cwd: options.cwd ?? process.cwd(),
      env: { ...env, ...options.env },
      stdio: ["ignore", "pipe", "pipe"]
    });
    let stdout = "";
    let stderr = "";
    let error;
    child.stdout.on("data", chunk => {
      stdout += chunk;
    });
    child.stderr.on("data", chunk => {
      stderr += chunk;
    });
    child.on("error", cause => {
      error = cause;
    });
    child.on("close", (status, signal) => {
      resolvePromise({ status, signal, error, stdout, stderr });
    });
  });
}

export function launch(command) {
  const child = runNative(command, process.argv.slice(2));
  if (child.error) {
    console.error(`solid-checker: could not start native ${command}: ${child.error.message}`);
    process.exit(2);
  }
  if (child.signal) {
    process.kill(process.pid, child.signal);
  }
  process.exit(child.status ?? 2);
}
