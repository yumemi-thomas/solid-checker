// `@solidjs/h@2.0.0-rc.3`'s `types/hyperscript.d.ts`: the default export names
// a binding these same bytes declare ambiently, so the whole file is ambient
// and there is no expression to evaluate. `export default 1;` is not this
// shape, and the control fixture pins its refusal.
export type HyperScript = {
	(...args: unknown[]): unknown;
};
declare const _default: HyperScript;
export default _default;
