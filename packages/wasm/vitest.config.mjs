import { defineConfig } from "vitest/config";

const requestedWorkers = Number(process.env.SOLID_CHECKER_TEST_WORKERS);
const maxWorkers = Number.isInteger(requestedWorkers) && requestedWorkers > 0 ? requestedWorkers : 2;

export default defineConfig({
  test: {
    environment: "node",
    include: ["**/*.test.mjs"],
    fileParallelism: maxWorkers > 1,
    maxWorkers,
    testTimeout: 120_000,
    hookTimeout: 120_000,
    teardownTimeout: 120_000
  }
});
