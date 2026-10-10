// Native inference reads an application/dependency/ancestor closure at the
// materialized analysis location. Coverage's key does not yet enumerate that
// closure. Recompute all no-target units, including refused inference; fixture
// names and the presence of an HTML file are not an inference boundary.
export async function runCoverageUnit(cache, parts, compute, runtimeArguments) {
  if (!runtimeArguments.includes("--runtime-target")) {
    return { value: await compute(), hit: false, inferenceCacheBypassed: true };
  }
  return cache.run(parts, compute);
}
