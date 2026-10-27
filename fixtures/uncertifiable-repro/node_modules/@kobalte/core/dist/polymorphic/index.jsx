import { omit, untrack } from "solid-js";
import { Dynamic } from "@solidjs/web";
//#region src/polymorphic/polymorphic.tsx
/**
* A utility component that render its `as` prop.
*/
function Polymorphic(props) {
	const others = omit(props, "as");
	if (!untrack(() => props.as)) throw new Error("[kobalte]: Polymorphic is missing the required `as` prop.");
	return <Dynamic {...others} component={props.as} />;
}
//#endregion
export { Polymorphic };
