export function plain(obj: object): boolean {
  let proto;
  return !(proto = Object.getPrototypeOf(obj)) || proto === Object.prototype;
}
