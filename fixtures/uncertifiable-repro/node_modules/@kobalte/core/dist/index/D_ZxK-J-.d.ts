import "./CFIwL2Gi.js";
import { ValidComponent } from "@solidjs/web";
import { Accessor, Ref } from "solid-js";
import { FocusOutsideEvent, InteractOutsideEvent, PointerDownOutsideEvent } from "@solid-primitives/interaction";
//#region src/dismissable-layer/dismissable-layer.d.ts
interface DismissableLayerCommonProps<T extends HTMLElement = HTMLElement> {
  ref: Ref<T>;
}
interface DismissableLayerRenderProps extends DismissableLayerCommonProps {}
//#endregion
export { DismissableLayerRenderProps as n, DismissableLayerCommonProps as t };