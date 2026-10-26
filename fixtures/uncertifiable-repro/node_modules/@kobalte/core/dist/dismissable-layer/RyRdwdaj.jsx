import { n as layerStack } from "../layer-stack/DUp9FXTP.jsx";
import { createContext, createEffect, createMemo, createSignal, omit, onSettled, useContext } from "solid-js";
import { interactOutside } from "@solid-primitives/interaction";
import { createShortcut } from "@solid-primitives/keyboard";
//#region src/dismissable-layer/dismissable-layer-context.tsx
const DismissableLayerContext = createContext(null);
function useOptionalDismissableLayerContext() {
	return useContext(DismissableLayerContext) ?? void 0;
}
//#endregion
//#region src/dismissable-layer/dismissable-layer.tsx
function DismissableLayer(props) {
	const [ref, setRef] = createSignal(void 0, { ownedWrite: true });
	const parentContext = useOptionalDismissableLayerContext();
	const others = omit(props, "ref", "disableOutsidePointerEvents", "excludedElements", "onEscapeKeyDown", "onPointerDownOutside", "onFocusOutside", "onInteractOutside", "onDismiss", "bypassTopMostLayerCheck");
	const isPointerBlocking = createMemo(() => props.disableOutsidePointerEvents);
	const nestedLayers = /* @__PURE__ */ new Set([]);
	const registerNestedLayer = (element) => {
		nestedLayers.add(element);
		const parentUnregister = parentContext?.registerNestedLayer(element);
		return () => {
			nestedLayers.delete(element);
			parentUnregister?.();
		};
	};
	const shouldExcludeElement = (element) => {
		if (!ref()) return false;
		return props.excludedElements?.some((node) => node()?.contains(element)) || [...nestedLayers].some((layer) => layer.contains(element));
	};
	const onPointerDownOutside = (e) => {
		if (!ref() || layerStack.isBelowPointerBlockingLayer(ref())) return;
		if (!props.bypassTopMostLayerCheck && !layerStack.isTopMostLayer(ref())) return;
		props.onPointerDownOutside?.(e);
		props.onInteractOutside?.(e);
		if (!e.defaultPrevented) props.onDismiss?.();
	};
	const onFocusOutside = (e) => {
		props.onFocusOutside?.(e);
		props.onInteractOutside?.(e);
		if (!e.defaultPrevented) props.onDismiss?.();
	};
	const interactOutsideRef = interactOutside({
		shouldExcludeElement,
		onPointerDownOutside,
		onFocusOutside
	});
	createShortcut(["Escape"], (e) => {
		if (!e || !ref() || !layerStack.isTopMostLayer(ref())) return;
		props.onEscapeKeyDown?.(e);
		if (!e.defaultPrevented && props.onDismiss) {
			e.preventDefault();
			props.onDismiss();
		}
	}, { preventDefault: false });
	onSettled(() => {
		if (!ref()) return;
		layerStack.addLayer({
			node: ref(),
			isPointerBlocking: isPointerBlocking(),
			dismiss: props.onDismiss
		});
		const unregisterFromParentLayer = parentContext?.registerNestedLayer(ref());
		layerStack.assignPointerEventToLayers();
		layerStack.disableBodyPointerEvents(ref());
		return () => {
			if (!ref()) return;
			layerStack.removeLayer(ref());
			unregisterFromParentLayer?.();
			layerStack.assignPointerEventToLayers();
			layerStack.restoreBodyPointerEvents(ref());
		};
	});
	createEffect(() => ({
		ref: ref(),
		disabled: isPointerBlocking()
	}), ({ ref, disabled: disableOutsidePointerEvents }) => {
		if (!ref) return;
		const layer = layerStack.find(ref);
		if (layer && layer.isPointerBlocking !== disableOutsidePointerEvents) {
			layer.isPointerBlocking = disableOutsidePointerEvents;
			layerStack.assignPointerEventToLayers();
		}
		if (disableOutsidePointerEvents) layerStack.disableBodyPointerEvents(ref);
		return () => {
			layerStack.restoreBodyPointerEvents(ref);
		};
	}, { defer: true });
	const context = { registerNestedLayer };
	return <DismissableLayerContext value={context}>
			<div ref={[
		setRef,
		interactOutsideRef,
		props.ref
	]} {...others}>
				{others.children}
			</div>
		</DismissableLayerContext>;
}
//#endregion
export { DismissableLayer as t };
