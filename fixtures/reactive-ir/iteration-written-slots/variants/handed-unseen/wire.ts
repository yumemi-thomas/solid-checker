declare function bindEvent(target: unknown, type: string, handler: () => void): void;
bindEvent(window, "resize", () => {});
export {};
