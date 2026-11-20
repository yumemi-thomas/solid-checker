import { defineConfig } from "vitest/config";
import solid from "@solidjs/vite-plugin";

export default defineConfig({
  plugins: [solid({ diagnostics: true })],
  test: { environment: "jsdom", setupFiles: ["./setup.ts"] },
});
