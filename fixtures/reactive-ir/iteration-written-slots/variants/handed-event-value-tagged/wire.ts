window.addEventListener("m", (e: any) => e.data`x`);
window.dispatchEvent(
  new MessageEvent("m", {
    data() {
      (this as any).currentTarget.Function("0")();
    },
  } as any),
);
export {};
