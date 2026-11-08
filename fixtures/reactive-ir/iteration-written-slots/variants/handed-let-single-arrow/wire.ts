let wake: (() => void) | undefined;
wake = () => {};
setTimeout(wake, 0);
addEventListener("online", wake);
export {};
