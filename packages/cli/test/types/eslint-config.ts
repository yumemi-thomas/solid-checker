import type { Linter, Rule } from "eslint";
import solidChecker, { type SolidCheckerSettings } from "solid-checker/eslint";

const settings: SolidCheckerSettings = {
  project: "./tsconfig.json",
  dialect: "solid-v2",
  acceptedContracts: ".solid-checker/accepted-contracts.json",
  receiptTrustConfiguration: "../trust.json",
  runtime: {
    target: "browser",
    rendering: "csr",
    conditions: ["browser", "import"]
  }
};

const config: Linter.Config[] = [
  solidChecker.configs.recommended,
  solidChecker.configs.v2,
  {
    settings: { solidChecker: settings },
    rules: {
      "solid-checker/strict-read-untracked": "warn",
      "solid-checker/contract-note": "off"
    }
  }
];

export const noteRule: Rule.RuleModule = solidChecker.rules["contract-note"];

export default config;
