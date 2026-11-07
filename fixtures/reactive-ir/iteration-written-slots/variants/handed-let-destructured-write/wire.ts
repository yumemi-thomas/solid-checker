let wake: any;
wake = () => {};
({ wake } = globalThis as any);
setTimeout(wake, 0);
export {};
