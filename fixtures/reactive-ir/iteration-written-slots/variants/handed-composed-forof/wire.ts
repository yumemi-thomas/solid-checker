document.body.addEventListener("x", (e) => {
  for (const t of e.composedPath() as any[]) t.Function?.("0")();
});
export {};
