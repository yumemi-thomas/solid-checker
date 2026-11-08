export function listen(target: EventTarget, type: string) {
  target.addEventListener(type, () => {});
}
