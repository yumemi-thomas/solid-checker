import { configDefaults, defineConfig } from "vitest/config";

const requestedWorkers = Number(process.env.SOLID_CHECKER_TEST_WORKERS);
// Four is the measured knee on the 14-core development host: two leaves the
// two long contract suites on the critical path, while eight adds process
// pressure without reducing wall time. The environment override remains for
// smaller CI runners and constrained local machines.
const maxWorkers = Number.isInteger(requestedWorkers) && requestedWorkers > 0 ? requestedWorkers : 4;

export default defineConfig({
  test: {
    environment: "node",
    include: ["**/*.test.mjs"],
    // `.claude/worktrees/` holds gitignored agent worktrees, each a full copy of
    // the repository. A CLI filter such as `scripts/*.test.mjs` matches their
    // copies too, so a stale test there failed the handoff gate for a tree
    // whose own tests all passed. Vitest's default excludes are kept.
    exclude: [...configDefaults.exclude, "**/.claude/**"],
    fileParallelism: maxWorkers > 1,
    maxWorkers,
    // The contract suites launch real native processes and package installs, so
    // retain a generous outer bound while their own probes keep tighter
    // operation-specific timeouts.
    testTimeout: 120_000,
    hookTimeout: 120_000,
    teardownTimeout: 120_000
  }
});
