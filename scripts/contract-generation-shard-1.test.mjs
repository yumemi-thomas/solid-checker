globalThis.__SOLID_CHECKER_GENERATION_TEST_SHARD = 1;
await import("./contract-generation.test.mjs?shard=1");
delete globalThis.__SOLID_CHECKER_GENERATION_TEST_SHARD;
