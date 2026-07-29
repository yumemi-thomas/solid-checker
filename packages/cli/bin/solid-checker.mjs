#!/usr/bin/env node

import { launch } from "./launcher.mjs";

if (process.argv[2] === "contract") {
  try {
    const { generatePackageContract, packageContractHelp } = await import(
      "../scripts/generate-package-contract.mjs"
    );
    if (process.argv[3] === "generate") {
      await generatePackageContract(process.argv.slice(4));
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
