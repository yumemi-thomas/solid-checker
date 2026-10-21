// Query wrappers: each returns a server function that carries a member next to
// the call. One member is untyped (`any`), so TypeScript resolves no declaration
// for it; the other is an ordinary method declared in this file.
declare function query<A extends unknown[], R>(
  fn: (...args: A) => R,
): ((...args: A) => R) & { keyFor: any };
declare function queryTyped<A extends unknown[], R>(
  fn: (...args: A) => R,
): ((...args: A) => R) & { keyFor(when: Date): string };

export const loadUser = query(async (id: number) => {
  "use server";
  return id;
});

export const loadTyped = queryTyped(async (id: number) => {
  "use server";
  return id;
});
