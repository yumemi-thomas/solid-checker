// A project helper mutates its first parameter with a source the caller built (`Object.assign(p, s)`).
export function itMerge(p: object, s: object) {
  Object.assign(p, s);
}
