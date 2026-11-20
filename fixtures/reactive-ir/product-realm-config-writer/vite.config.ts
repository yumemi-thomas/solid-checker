import "fz-prealm-named";
import { defineConfig } from "vitest/config";

export default defineConfig({
  plugins: [],
  test: { environment: "jsdom", setupFiles: ["./setup.ts"] },
});
