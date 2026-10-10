// A declaration-only module: no calls, no JSX, no member expressions and no
// bindings. Every loop the returned-adapter machinery runs over a file other
// than the one it was asked about iterates one of those four tables, so nothing
// here can be read by any cross-file proof -- and editing it must not
// invalidate the answers cached for `adapter.ts` and `consumer.ts`.
export interface Row {
  value: number;
}

export type Rows = Row[];
