import type { Linter } from "eslint";
import solidChecker, { type SolidCheckerSettings } from "solid-checker/eslint";

const settings: SolidCheckerSettings = {
  project: "./tsconfig.json",
  dialect: "solid-v2"
};

const config: Linter.Config[] = [
  solidChecker.configs.recommended,
  solidChecker.configs.v2,
  {
    settings: { solidChecker: settings },
    rules: { "solid-checker/strict-read-untracked": "warn" }
  }
];

export default config;
