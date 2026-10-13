# A later types condition is reachable when import has no declaration sibling

The runtime is published JavaScript. The import branch has no index.d.mts;
TypeScript continues to the later types branch and selects index.d.ts. The
checker must select the same declaration with its exact /exports/./types trace.
Companion adapter/native controls preserve an earlier existing .d.mts candidate
and refuse a missing runtime file even when the types branch exists. This
fixture's signature is the complete published signature, not a widened stub.
