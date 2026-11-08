export async function zero(n: number) {
  "use server";
  return n;
}
export async function upload(a: number | undefined, b?: Uint8Array) {
  "use server";
  return [a, b?.length];
}
export async function nothing(a: undefined) {
  "use server";
  return a;
}
