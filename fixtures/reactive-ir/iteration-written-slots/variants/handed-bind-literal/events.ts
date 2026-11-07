export function bindEvent(target: EventTarget, type: string, handler: (e: any) => void) {
  target.addEventListener(type, handler);
  return () => target.removeEventListener(type, handler);
}
