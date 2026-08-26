globalThis.__SOLID_CHECKER_REVIEW_TEST_SHARD = 3;
await import("./contract-review.test.mjs?shard=3");
delete globalThis.__SOLID_CHECKER_REVIEW_TEST_SHARD;
