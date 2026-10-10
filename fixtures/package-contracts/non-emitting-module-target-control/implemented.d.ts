// A `.d.ts` whose bytes carry an initializer: TS1039. The suffix claims a
// declaration file and the bytes refute it, so the suffix cannot vouch for
// them and the case refuses. `declare const value: number;` would not.
declare const value = 1;
export type Value = typeof value;
