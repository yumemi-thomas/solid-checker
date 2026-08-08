// File B: the project's only invocation of the adapter `adapter.ts` returned.
// The staleness test edits this call away and nothing else, so any answer about
// `adapter.ts` that survives the edit unchanged is stale.
import { scaled } from "./adapter";

export function rows() {
  return scaled();
}
