globalThis.onmessage = (event: MessageEvent<string>) => {
  console.log(event.data);
};
export {};
