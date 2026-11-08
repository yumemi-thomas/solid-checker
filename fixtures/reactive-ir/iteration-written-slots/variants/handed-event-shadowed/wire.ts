const reach = (Event: new (type: string) => globalThis.Event) => {
  window.dispatchEvent(new Event("x"));
};
reach(globalThis.Event);
export {};
