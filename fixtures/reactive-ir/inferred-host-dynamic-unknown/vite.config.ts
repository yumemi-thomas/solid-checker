import { defineConfig } from "vite";
export default defineConfig(({mode}) => mode === "production" ? {plugins: []} : {plugins: [...unknownPlugins]});
