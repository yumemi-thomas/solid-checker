export async function save(when: Date) {
  "use server";
  return when.toISOString();
}

// ADR 0830: SC7007's arity arm (TS2554) under a directive: nothing reports
// the call, so whether the extra argument crosses the transport is open.
export function suppressed() {
  // @ts-ignore
  void save(new Date(), new Set([1]));
}

// The directive covers another line: TS2554 stands, silent.
export function elsewhere() {
  // @ts-ignore
  const unused: number = "a";
  void save(new Date(), new Set([1]));
  return unused;
}
