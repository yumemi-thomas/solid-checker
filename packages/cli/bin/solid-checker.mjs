#!/usr/bin/env node

import { launch, runNative } from "./launcher.mjs";

if (process.argv[2] === "contract") {
  try {
    const { generatePackageContract, packageContractHelp } = await import(
      "../scripts/generate-package-contract.mjs"
    );
    if (process.argv[3] === "generate") {
      await generatePackageContract(process.argv.slice(4));
    } else if (process.argv[3] === "probe") {
      // A fourth branch, and Node-only. It executes the package's own code, so
      // it is deliberately its own opt-in command rather than a flag on
      // generate, which imports nothing.
      const { probeContract } = await import("../scripts/probe-contract.mjs");
      await probeContract(process.argv.slice(4));
    } else if (process.argv[3] === "check") {
      // The native checker owns contract discovery, so `contract check` is the
      // discoverable spelling of `--check-contracts` rather than a second
      // implementation of it. Remaining arguments (--project, --format,
      // --contract) pass straight through.
      const child = runNative("solid-checker", [
        "--check-contracts",
        ...process.argv.slice(4)
      ]);
      if (child.error) {
        throw new Error(`could not start the native checker: ${child.error.message}`);
      }
      process.exit(child.status ?? 2);
    } else if (!process.argv[3] || ["--help", "-h"].includes(process.argv[3])) {
      process.stdout.write(packageContractHelp);
    } else {
      throw new Error(`unknown contract command ${process.argv[3]}`);
    }
  } catch (error) {
    console.error(`solid-checker: ${error instanceof Error ? error.message : error}`);
    process.exitCode = 2;
  }
} else {
  launch("solid-checker");
}
