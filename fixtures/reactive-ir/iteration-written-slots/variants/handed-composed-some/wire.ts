document.body.addEventListener("x", (e) => {
  e.composedPath().some((t: any) => {
    t.Function("0")();
    return false;
  });
});
export {};
