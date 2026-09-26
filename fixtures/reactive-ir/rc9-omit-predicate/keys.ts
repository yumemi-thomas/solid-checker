// A predicate defined in another module. It is inert, but `omit`'s call site
// in App.tsx sees only the import, not this body.
export function hiddenKey(key: string | symbol): boolean {
  return key === "a";
}
