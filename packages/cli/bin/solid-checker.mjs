#!/usr/bin/env node

import { launch } from "./launcher.mjs";

if (process.argv[2] === "feedback") {
  try {
    const { developmentFeedback } = await import("../scripts/development-feedback.mjs");
    await developmentFeedback(process.argv.slice(3));
  } catch (error) {
    console.error(`solid-checker: ${error instanceof Error ? error.message : error}`);
    process.exitCode = 2;
  } finally {
    if (process.argv[3] === "run") {
      // Configs can leave timers even when startup or collection fails.
      // Drain both published JSON and diagnostics before ending this CLI.
      try {
        await Promise.all([process.stdout, process.stderr].map(stream =>
          new Promise((resolve, reject) => stream.write("", error => error ? reject(error) : resolve()))));
      } catch { process.exitCode = 2; }
      process.exit(process.exitCode ?? 0);
    }
  }
} else if (process.argv[2] === "contract") {
  console.error(
    "solid-checker: the contract commands were removed with package contracts; " +
      "packages are analyzed from source or their behaviour stays uncertifiable"
  );
  process.exitCode = 2;
} else {
  launch("solid-checker");
}
