// A project helper mutates its first parameter through a computed key (`Object.assign(p, { [k]: f })`).
export function itAssign(p: object, k: PropertyKey, f: unknown) {
  Object.assign(p, { [k]: f });
}
