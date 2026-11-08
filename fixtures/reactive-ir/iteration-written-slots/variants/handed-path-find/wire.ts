document.body.addEventListener("x", (e) => {
  const a: any = e.composedPath().find((el) => el instanceof Node && (el as Node).nodeName === "A");
  console.log(a);
});
export {};
