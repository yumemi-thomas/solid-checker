import { defineConfig } from "vite";

export default defineConfig({
  plugins: [],
  define: { "Array.prototype.fzprobe": "1" },
});
