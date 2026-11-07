window.addEventListener("m", (e: any) => e.data());
window.dispatchEvent(
  new MessageEvent("m", {
    data() {
      (this as any).currentTarget.Function("0")();
    },
  } as any),
);
export {};
