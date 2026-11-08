const onKey = (event: KeyboardEvent) => {
  if (event.key === "a" && !event.ctrlKey) event.preventDefault();
};
window.addEventListener("keydown", onKey);
export {};
