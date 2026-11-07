function idle(run: () => void) {
  return () => run();
}
const flush = idle(() => {});
addEventListener("load", flush);
export {};
