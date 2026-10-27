import { createEffect, createSignal } from "solid-js";
//#region src/primitives/create-tag-name/create-tag-name.ts
/**
* Returns the tag name by parsing an element ref.
* @example
* function Component(props) {
*   const [ref, setRef] = createSignal<HTMLDivElement>();
*   const tagName = createTagName(ref, () => "button"); // div
*   return <div ref={setRef} {...props} />;
* }
*/
function createTagName(ref, fallback) {
	const [tagName, setTagName] = createSignal(stringOrUndefined(fallback?.()), { ownedWrite: true });
	createEffect(() => ref()?.tagName.toLowerCase() || stringOrUndefined(fallback?.()), (value) => {
		setTagName(value);
	});
	return tagName;
}
function stringOrUndefined(value) {
	return typeof value === "string" ? value : void 0;
}
//#endregion
export { createTagName as t };
