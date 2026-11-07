document.body.addEventListener("x", (e) => {
  const a: any = e.composedPath().find((el) => el !== document);
  a.Function("0")();
});
export {};
