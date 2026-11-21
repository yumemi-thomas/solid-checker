// A type-only import of `vite` loads nothing.
import type { UserConfig } from "vite";

export const config: UserConfig = { plugins: [] };
