#!/usr/bin/env node

import { launch, runNative } from "./launcher.mjs";

if (process.argv[2] === "contract") {
  try {
    const { generatePackageContract, packageContractHelp } = await import(
      "../scripts/generate-package-contract-v2.mjs"
    );
    if (process.argv[3] === "generate") {
      const args = process.argv.slice(4);
      // Any `=` spelling routes here too. `--missing=1` used to miss this
      // branch entirely and reach the single-package parser, which reported it
      // as an unknown argument -- so the sweep's own "--missing takes no value"
      // refusal, the message that says what the flag actually means, could
      // never fire.
      if (args.some(argument => argument === "--missing" || argument.startsWith("--missing="))) {
        // The sweep enumerates packages from the contract report and owns its
        // own exit code, so it is a separate entry point rather than a mode of
        // the single-package generator.
        const { generateMissingContracts } = await import(
          "../scripts/generate-missing-contracts.mjs"
        );
        await generateMissingContracts(args);
      } else {
        await generatePackageContract(args);
      }
    } else if (process.argv[3] === "probe") {
      // A fourth branch, and Node-only. It executes the package's own code, so
      // it is deliberately its own opt-in command rather than a flag on
      // generate, which imports nothing.
      const { probeContract } = await import("../scripts/probe-contract.mjs");
      await probeContract(process.argv.slice(4));
    } else if (process.argv[3] === "verify") {
      // The machine promotion path. It takes no decision, so it is not a mode
      // of `contract review`, whose whole shape is recorded human decisions --
      // and keeping it separate is what keeps that command's invariant ("no row
      // reaches reviewed evidence without a human decision naming its export")
      // intact rather than weakened by a flag.
      const { verifyContract } = await import("../scripts/verify-contract.mjs");
      await verifyContract(process.argv.slice(4));
    } else if (process.argv[3] === "review") {
      const { reviewContract } = await import("../scripts/review-contract.mjs");
      await reviewContract(process.argv.slice(4));
    } else if (process.argv[3] === "check") {
      // The native checker owns contract discovery, so `contract check` is the
      // discoverable spelling of `--check-contracts` rather than a second
      // implementation of it. Remaining arguments pass straight through.
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
