// The same helper shape with nothing handed on: no array intrinsic reaches it, so its target is no iteration target.
export function putProp(target: object, name: string, value: unknown) {
  Object.assign(target, { [name]: value });
}
