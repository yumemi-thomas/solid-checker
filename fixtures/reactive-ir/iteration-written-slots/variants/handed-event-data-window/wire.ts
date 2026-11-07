window.addEventListener("m", (e: any) => {
  e.data.Function("0")();
});
window.dispatchEvent(new MessageEvent("m", { data: window }));
export {};
