//#region src/dismissable-layer/layer-stack.tsx
const DATA_TOP_LAYER_ATTR = "data-kb-top-layer";
let originalBodyPointerEvents;
let hasDisabledBodyPointerEvents = false;
const layers = [];
function indexOf(node) {
	return layers.findIndex((layer) => layer.node === node);
}
function find(node) {
	return layers[indexOf(node)];
}
function isTopMostLayer(node) {
	return layers.length > 0 && layers[layers.length - 1].node === node;
}
function getPointerBlockingLayers() {
	return layers.filter((layer) => layer.isPointerBlocking);
}
function getTopMostPointerBlockingLayer() {
	return [...getPointerBlockingLayers()].slice(-1)[0];
}
function hasPointerBlockingLayer() {
	return getPointerBlockingLayers().length > 0;
}
function isBelowPointerBlockingLayer(node) {
	const highestBlockingIndex = indexOf(getTopMostPointerBlockingLayer()?.node);
	return indexOf(node) < highestBlockingIndex;
}
function addLayer(layer) {
	layers.push(layer);
}
function removeLayer(node) {
	const index = indexOf(node);
	if (index < 0) return;
	layers.splice(index, 1);
}
function assignPointerEventToLayers() {
	for (const { node } of layers) node.style.pointerEvents = isBelowPointerBlockingLayer(node) ? "none" : "auto";
}
/**
* Disable body `pointer-events` if there are "pointer blocking" layers in the stack,
* and body `pointer-events` has not been disabled yet.
*/
function disableBodyPointerEvents(node) {
	if (hasPointerBlockingLayer() && !hasDisabledBodyPointerEvents) {
		const ownerDocument = node.ownerDocument;
		originalBodyPointerEvents = document.body.style.pointerEvents;
		ownerDocument.body.style.pointerEvents = "none";
		hasDisabledBodyPointerEvents = true;
	}
}
/**
* Restore body `pointer-events` style if there is no "pointer blocking" layer in the stack.
*/
function restoreBodyPointerEvents(node) {
	if (hasPointerBlockingLayer()) return;
	const ownerDocument = node.ownerDocument;
	ownerDocument.body.style.pointerEvents = originalBodyPointerEvents;
	if (ownerDocument.body.style.length === 0) ownerDocument.body.removeAttribute("style");
	hasDisabledBodyPointerEvents = false;
}
const layerStack = {
	layers,
	isTopMostLayer,
	hasPointerBlockingLayer,
	isBelowPointerBlockingLayer,
	addLayer,
	removeLayer,
	indexOf,
	find,
	assignPointerEventToLayers,
	disableBodyPointerEvents,
	restoreBodyPointerEvents
};
//#endregion
export { layerStack as n, DATA_TOP_LAYER_ATTR as t };
