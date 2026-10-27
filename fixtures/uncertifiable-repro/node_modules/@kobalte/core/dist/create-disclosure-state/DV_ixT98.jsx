import { createControllableBooleanSignal } from "@solid-primitives/controlled-signal";
import { access } from "@solid-primitives/utils";
//#region src/primitives/create-disclosure-state/create-disclosure-state.ts
/**
* Provides state management for open, close and toggle scenarios.
* Used to control the "open state" of components like Modal, Drawer, etc.
*/
function createDisclosureState(props = {}) {
	const [isOpen, setIsOpen] = createControllableBooleanSignal({
		value: () => access(props.open),
		defaultValue: () => !!access(props.defaultOpen),
		onChange: (value) => props.onOpenChange?.(value)
	});
	const open = () => {
		setIsOpen(true);
	};
	const close = () => {
		setIsOpen(false);
	};
	const toggle = () => {
		isOpen() ? close() : open();
	};
	return {
		isOpen,
		setIsOpen,
		open,
		close,
		toggle
	};
}
//#endregion
export { createDisclosureState as t };
