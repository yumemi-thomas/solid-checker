const key = ["con", "firm"].join("");
(window as any)[key] = function (this: any) {
  this.Function("0")();
};
window.confirm("y");
export {};
