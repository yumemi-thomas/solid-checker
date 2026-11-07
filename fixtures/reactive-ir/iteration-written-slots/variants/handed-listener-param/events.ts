export function bindEvent(target: EventTarget, type: string, handler: () => void) {
  target.addEventListener(type, handler);
}
