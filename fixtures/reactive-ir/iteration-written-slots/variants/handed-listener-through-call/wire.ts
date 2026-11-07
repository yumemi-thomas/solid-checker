const make = (_x: unknown) =>
  function (this: any) {
    this.Function("0")();
  };
const handler = make(() => {});
window.addEventListener("x", handler);
export {};
