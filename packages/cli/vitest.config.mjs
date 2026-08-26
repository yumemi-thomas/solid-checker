import { defineConfig } from "vitest/config";

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
