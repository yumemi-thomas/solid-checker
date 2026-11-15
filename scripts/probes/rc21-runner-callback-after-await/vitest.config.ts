import { fileURLToPath } from "node:url";
import { defineConfig } from "vitest/config";

// The dev build of @solidjs/signals 2.0.0-rc.21 (its `development` export), named by file so no resolution condition decides it. The probe is no test of the repository:
// it is included by name only here.
export default defineConfig({
  resolve: { alias: { "@solidjs/signals": fileURLToPath(new URL("./node_modules/@solidjs/signals/dist/dev.js", import.meta.url)) } },
  test: { include: ["runner.probe.ts"] },
});
