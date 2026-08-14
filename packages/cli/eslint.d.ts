import type { ESLint, Linter, Rule } from "eslint";

export interface SolidCheckerSettings {
  /** Path to the tsconfig analyzed by solid-checker. Auto-discovered by default. */
  project?: string;
  /** Working directory used to resolve relative paths. */
  cwd?: string;
  /** Override the solid-checker executable. */
  command?: string;
  /** Arguments placed before solid-checker's generated CLI arguments. */
  commandArgs?: string[];
  /** Additional reviewed package-contract documents. */
  contracts?: string[];
  /** Force a dialect instead of detecting it from the project. */
  dialect?: "solid-v1" | "solid-v2" | (string & {});
  /** Read a canonical JSON snapshot instead of starting an analysis process. */
  snapshotPath?: string;
}

export interface SolidCheckerPlugin extends ESLint.Plugin {
  meta: {
    name: "solid-checker";
    version: string;
  };
  rules: Record<string, Rule.RuleModule> & {
    certification: Rule.RuleModule;
  };
  configs: Record<string, Linter.Config> & {
    recommended: Linter.Config;
    v1: Linter.Config;
    v2: Linter.Config;
  };
}

declare const plugin: SolidCheckerPlugin;

export default plugin;
