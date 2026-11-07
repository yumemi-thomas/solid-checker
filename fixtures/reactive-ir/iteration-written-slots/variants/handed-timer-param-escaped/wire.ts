function later(run: () => void) {
  setTimeout(run, 0);
}
later(() => {});
(globalThis as any).later = later;
export {};
