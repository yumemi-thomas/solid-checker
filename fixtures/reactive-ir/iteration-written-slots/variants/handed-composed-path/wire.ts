document.body.addEventListener("x", (e) => {
  const t: any = e.composedPath().at(-1);
  t.Function("0")();
});
export {};
