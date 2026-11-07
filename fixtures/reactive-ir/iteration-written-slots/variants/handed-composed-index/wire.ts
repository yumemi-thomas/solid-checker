document.body.addEventListener("x", (e) => {
  const t: any = e.composedPath()[3];
  const AP = t.Array.prototype;
  const keys: PropertyKey[] = [Symbol.iterator];
  AP[keys[0]] = function () {};
});
export {};
