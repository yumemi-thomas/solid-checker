import type { ESLint, Linter, Rule } from "eslint";

export interface SolidCheckerRuntimeSettings {
  /** Explicit browser/client or node/server runtime target. */
  target?: "browser" | "node" | (string & {});
  /** Explicit development or production build mode. */
  build?: "development" | "production" | (string & {});
  /** Explicit CSR, string SSR, or streaming SSR rendering mode. */
  rendering?: "csr" | "string-ssr" | "streaming-ssr" | (string & {});
  /**
   * Whether code outside this project may import from it. `"closed"` asserts
   * the analyzed files are the whole program, which lets an exported symbol's
   * caller set be enumerated. It never licenses guessing one: every reference
   * must still resolve to a use the analyzer understands. When unset, the
   * project's nearest `package.json` decides: a private or unpublished
   * package is closed, a published library is open (ADR 0193).
   */
  programBoundary?: "open" | "closed" | (string & {});
  /** Exact package/framework conditions selected for this analysis. */
  conditions?: string[];
  /** Explicit framework/compiler transforms, such as `use-server`. */
  frameworkTransforms?: string[];
}

export interface SolidCheckerSettings {
  /** Path to the tsconfig analyzed by solid-checker. Auto-discovered by default. */
  project?: string;
  /** Working directory used to resolve relative paths. */
  cwd?: string;
  /** Override the solid-checker executable. */
  command?: string;
  /** Arguments placed before solid-checker's generated CLI arguments. */
  commandArgs?: string[];
  /**
   * Receipt-issued stable-v1 contract catalog for exact imports. A relative
   * path resolves against `cwd`.
   */
  acceptedContracts?: string;
  /**
   * Policy-2 issuer trust (`--receipt-trust-configuration`), the file
   * `contract certify --trust-configuration-output` wrote. Without it a
   * discovered policy-2 catalog is not read, and one named by
   * `acceptedContracts` fails the analysis. The withheld catalog is reported
   * by the `contract-note` rule. A relative path resolves against
   * `cwd`; its bytes are part of the analysis cache identity.
   */
  receiptTrustConfiguration?: string;
  /** Force a dialect instead of detecting it from the project. */
  dialect?: "solid-v2" | (string & {});
  /** Exact runtime selection used for artifact cases and rendering proofs. */
  runtime?: SolidCheckerRuntimeSettings;
  /** Opt-in Vite client resolver observation; executes project config. Off by default. */
  runtimeResolution?: "required" | "off";
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
    /**
     * The run's `solid-checker: note:` lines, such as a project catalog
     * withheld for want of receipt trust, at line 1 of every linted file.
     * `warn` in every shipped config; `off` hides the notes.
     */
    "contract-note": Rule.RuleModule;
  };
  configs: Record<string, Linter.Config> & {
    recommended: Linter.Config;
    v1: Linter.Config;
    v2: Linter.Config;
    /**
     * `v2` with `settings.solidChecker.runtime.target` set to `"browser"`
     * (ADR 0269), for client applications.
     */
    "browser-v2": Linter.Config;
  };
}

declare const plugin: SolidCheckerPlugin;

export default plugin;
