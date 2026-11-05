export function useThing(o: { a?: number }) { const p = o.a ?? 7; return { p }; }
