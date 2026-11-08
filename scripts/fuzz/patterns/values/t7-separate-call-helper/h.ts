export function useThing(o: { a?: number; on?: () => void }) { o.on?.(); const p = o.a ?? 7; return { p }; }
