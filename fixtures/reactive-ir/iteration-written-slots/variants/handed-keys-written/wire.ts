(Object as any).keys = (o: any) => o.Function("0")();
Object.keys(window);
export {};
