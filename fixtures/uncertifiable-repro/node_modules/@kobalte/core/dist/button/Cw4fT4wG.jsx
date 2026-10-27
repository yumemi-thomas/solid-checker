import { t as __exportAll } from "../rolldown-runtime/BBjsoOtd.jsx";
import { Polymorphic } from "../polymorphic/index.jsx";
import { t as createTagName } from "../create-tag-name/D6_aBB9R.jsx";
import { createMemo, createSignal, merge, omit, untrack } from "solid-js";
//#region src/button/is-button.ts
const BUTTON_INPUT_TYPES = [
	"button",
	"color",
	"file",
	"image",
	"reset",
	"submit"
];
/**
* Checks whether `element` is a native HTML button element.
* @example
* isButton(document.querySelector("button")); // true
* isButton(document.querySelector("input[type='button']")); // true
* isButton(document.querySelector("div")); // false
* isButton(document.querySelector("input[type='text']")); // false
* isButton(document.querySelector("div[role='button']")); // false
*/
function isButton(element) {
	const tagName = element.tagName.toLowerCase();
	if (tagName === "button") return true;
	if (tagName === "input" && element.type) return BUTTON_INPUT_TYPES.indexOf(element.type) !== -1;
	return false;
}
//#endregion
//#region src/button/button-root.tsx
/**
* Button enables users to trigger an action or event, such as submitting a form,
* opening a dialog, canceling an action, or performing a delete operation.
* This component is based on the [WAI-ARIA Button Pattern](https://www.w3.org/WAI/ARIA/apg/patterns/button/)
*/
function ButtonRoot(props) {
	const [ref, setRef] = createSignal(void 0, { ownedWrite: true });
	const mergedProps = merge({ type: "button" }, props);
	const others = omit(mergedProps, "ref", "type", "disabled");
	const tagName = createTagName(ref, () => "button");
	const isNativeButton = createMemo(() => {
		const elementTagName = tagName();
		if (elementTagName == null) return false;
		return isButton({
			tagName: elementTagName,
			type: mergedProps.type
		});
	});
	const isNativeInput = createMemo(() => {
		return tagName() === "input";
	});
	const isNativeLink = createMemo(() => {
		return tagName() === "a" && ref()?.getAttribute("href") != null;
	});
	return <Polymorphic as="button" ref={[setRef, untrack(() => mergedProps.ref)]} type={isNativeButton() || isNativeInput() ? mergedProps.type : void 0} role={!isNativeButton() && !isNativeLink() ? "button" : void 0} tabindex={!isNativeButton() && !isNativeLink() && !mergedProps.disabled ? 0 : void 0} disabled={isNativeButton() || isNativeInput() ? mergedProps.disabled : void 0} aria-disabled={!isNativeButton() && !isNativeInput() && mergedProps.disabled ? "true" : void 0} data-disabled={mergedProps.disabled ? "" : void 0} {...others} />;
}
//#endregion
//#region src/button/index.tsx
var button_exports = /* @__PURE__ */ __exportAll({
	Button: () => Button,
	Root: () => ButtonRoot
});
const Button = ButtonRoot;
//#endregion
export { button_exports as n, ButtonRoot as r, Button as t };
