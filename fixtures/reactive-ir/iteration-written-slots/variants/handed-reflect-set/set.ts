// A project helper mutates its first parameter reflectively (`Reflect.set(p, k, f)`).
export function itReflect(p: object, k: PropertyKey, f: unknown) {
  Reflect.set(p, k, f);
}
