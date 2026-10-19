type Runner = { run: () => void };
function helper(o: Runner) {
  o.run();
}
declare const p: Promise<number>;
export async function fwd(o: Runner) {
  let x: number | undefined;
  x ??= await p;
  helper(o);
}
