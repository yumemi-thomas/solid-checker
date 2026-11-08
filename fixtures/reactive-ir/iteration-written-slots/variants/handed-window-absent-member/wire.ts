const log = (...a: unknown[]) => {
  (window as any).__log?.(...a);
};
log("x");
export {};
