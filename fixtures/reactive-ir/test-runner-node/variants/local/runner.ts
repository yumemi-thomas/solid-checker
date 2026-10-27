// A project module that spells the runner's API: it declares what it exports,
// so its `it` is a project function whatever the name.
export function describe(name: string, body: () => void) {
  body();
}
export function it(name: string, body: () => void) {
  body();
}
