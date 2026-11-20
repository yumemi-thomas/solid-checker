import { defineConfig } from "vitest/config";

export default defineConfig({
  plugins: [],
  resolve: { tsconfigPaths: true },
  test: { environment: "jsdom", setupFiles: ["./setup.ts"] },
});
