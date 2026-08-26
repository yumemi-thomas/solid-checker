globalThis.__SOLID_CHECKER_REVIEW_TEST_SHARD = 2;
await import("./contract-review.test.mjs?shard=2");
delete globalThis.__SOLID_CHECKER_REVIEW_TEST_SHARD;
