window.addEventListener("m", (e: any) => {
  e.data.Function("0")();
});
window.dispatchEvent(
  new MessageEvent("m", {
    get data() {
      return window;
    },
  }),
);
export {};
