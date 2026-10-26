import { s as PolymorphicProps, t as ElementOf } from "./CFIwL2Gi.js";
import { a as ButtonRootOptions, i as ButtonRootCommonProps, s as ButtonRootRenderProps } from "./Bjp2qtUK.js";
import { n as DismissableLayerRenderProps, t as DismissableLayerCommonProps } from "./D_ZxK-J-.js";
import { ComponentProps, JSX as JSX$1, Portal, ValidComponent } from "@solidjs/web";
import { Accessor, ParentProps, Ref, Setter } from "solid-js";
import { FocusOutsideEvent, InteractOutsideEvent, PointerDownOutsideEvent } from "@solid-primitives/interaction";
//#region src/dialog/dialog-close-button.d.ts
interface DialogCloseButtonOptions extends ButtonRootOptions {}
interface DialogCloseButtonCommonProps<T extends HTMLElement = HTMLElement> extends ButtonRootCommonProps<T> {
  onClick: JSX$1.EventHandlerUnion<T, MouseEvent>;
  "aria-label": string;
}
interface DialogCloseButtonRenderProps extends DialogCloseButtonCommonProps, ButtonRootRenderProps {}
type DialogCloseButtonProps<T extends ValidComponent | HTMLElement = HTMLElement> = DialogCloseButtonOptions & Partial<DialogCloseButtonCommonProps<ElementOf<T>>>;
/**
 * The button that closes the dialog.
 */
declare function DialogCloseButton<T extends ValidComponent = "button">(props: PolymorphicProps<T, DialogCloseButtonProps<T>>): JSX$1.Element;
//#endregion
//#region src/dialog/dialog-content.d.ts
interface DialogContentOptions {
  /**
   * Event handler called when focus moves into the component after opening.
   * It can be prevented by calling `event.preventDefault`.
   */
  onOpenAutoFocus?: (event: Event) => void;
  /**
   * Event handler called when focus moves to the trigger after closing.
   * It can be prevented by calling `event.preventDefault`.
   */
  onCloseAutoFocus?: (event: Event) => void;
  /**
   * Event handler called when the escape key is down.
   * It can be prevented by calling `event.preventDefault`.
   */
  onEscapeKeyDown?: (event: KeyboardEvent) => void;
  /**
   * Event handler called when a pointer event occurs outside the bounds of the component.
   * It can be prevented by calling `event.preventDefault`.
   */
  onPointerDownOutside?: (event: PointerDownOutsideEvent) => void;
  /**
   * Event handler called when the focus moves outside the bounds of the component.
   * It can be prevented by calling `event.preventDefault`.
   */
  onFocusOutside?: (event: FocusOutsideEvent) => void;
  /**
   * Event handler called when an interaction (pointer or focus event) happens outside the bounds of the component.
   * It can be prevented by calling `event.preventDefault`.
   */
  onInteractOutside?: (event: InteractOutsideEvent) => void;
}
interface DialogContentCommonProps<T extends HTMLElement = HTMLElement> extends DismissableLayerCommonProps<T> {
  id: string;
}
interface DialogContentRenderProps extends DialogContentCommonProps, DismissableLayerRenderProps {
  role: "dialog" | "alertdialog";
  tabindex: -1;
}
type DialogContentProps<T extends ValidComponent | HTMLElement = HTMLElement> = DialogContentOptions & Partial<DialogContentCommonProps<ElementOf<T>>>;
/**
 * Contains the content to be rendered when the dialog is open.
 */
declare function DialogContent<T extends ValidComponent = "div">(props: PolymorphicProps<T, DialogContentProps<T>>): import("@solidjs/web").JSX.Element;
//#endregion
//#region src/dialog/dialog-description.d.ts
interface DialogDescriptionOptions {}
interface DialogDescriptionCommonProps<_T extends HTMLElement = HTMLElement> {
  id: string;
}
interface DialogDescriptionRenderProps extends DialogDescriptionCommonProps {}
type DialogDescriptionProps<T extends ValidComponent | HTMLElement = HTMLElement> = DialogDescriptionOptions & Partial<DialogDescriptionCommonProps<ElementOf<T>>>;
/**
 * An optional accessible description to be announced when the dialog is open.
 */
declare function DialogDescription<T extends ValidComponent = "p">(props: PolymorphicProps<T, DialogDescriptionProps<T>>): import("@solidjs/web").JSX.Element;
//#endregion
//#region src/dialog/dialog-overlay.d.ts
interface DialogOverlayOptions {}
interface DialogOverlayCommonProps<T extends HTMLElement = HTMLElement> {
  ref: Ref<T>;
  onPointerDown: JSX$1.EventHandlerUnion<T, PointerEvent>;
  style: JSX$1.CSSProperties | string;
}
interface DialogOverlayRenderProps extends DialogOverlayCommonProps {
  "data-expanded": string | undefined;
  "data-closed": string | undefined;
}
type DialogOverlayProps<T extends ValidComponent | HTMLElement = HTMLElement> = DialogOverlayOptions & Partial<DialogOverlayCommonProps<ElementOf<T>>>;
/**
 * A layer that covers the inert portion of the view when the dialog is open.
 */
declare function DialogOverlay<T extends ValidComponent = "div">(props: PolymorphicProps<T, DialogOverlayProps<T>>): JSX$1.Element;
//#endregion
//#region src/dialog/dialog-portal.d.ts
interface DialogPortalProps extends ComponentProps<typeof Portal> {}
/**
 * Portals its children into the `body` when the dialog is open.
 */
declare function DialogPortal(props: DialogPortalProps): import("@solidjs/web").JSX.Element;
//#endregion
//#region src/dialog/dialog.intl.d.ts
declare const DIALOG_INTL_TRANSLATIONS: {
  dismiss: string;
};
type DialogIntlTranslations = typeof DIALOG_INTL_TRANSLATIONS;
//#endregion
//#region src/dialog/dialog-root.d.ts
interface DialogRootOptions {
  /** The localized strings of the component. */
  translations?: DialogIntlTranslations;
  /** The controlled open state of the dialog. */
  open?: boolean;
  /**
   * The default open state when initially rendered.
   * Useful when you do not need to control the open state.
   */
  defaultOpen?: boolean;
  /** Event handler called when the open state of the dialog changes. */
  onOpenChange?: (isOpen: boolean) => void;
  /**
   * A unique identifier for the component.
   * The id is used to generate id attributes for nested components.
   * If no id prop is provided, a generated id will be used.
   */
  id?: string;
  /**
   * Whether the dialog should be the only visible content for screen readers.
   * When set to `true`:
   * - interaction with outside elements will be disabled.
   * - scroll will be locked.
   * - focus will be locked inside the dialog content.
   * - elements outside the dialog content will not be visible for screen readers.
   */
  modal?: boolean;
  /** Whether the scroll should be locked even if the dialog is not modal. */
  preventScroll?: boolean;
  /**
   * Used to force mounting the dialog (portal, overlay and content) when more control is needed.
   * Useful when controlling animation with SolidJS animation libraries.
   */
  forceMount?: boolean;
}
interface DialogRootProps extends ParentProps<DialogRootOptions> {}
/**
 * A dialog is a window overlaid on either the primary window or another dialog window.
 */
declare function DialogRoot(props: DialogRootProps): import("@solidjs/web").JSX.Element;
//#endregion
//#region src/dialog/dialog-title.d.ts
interface DialogTitleOptions {}
interface DialogTitleCommonProps<_T extends HTMLElement = HTMLElement> {
  id: string;
}
interface DialogTitleRenderProps extends DialogTitleCommonProps {}
type DialogTitleProps<T extends ValidComponent | HTMLElement = HTMLElement> = DialogTitleOptions & Partial<DialogTitleCommonProps<ElementOf<T>>>;
/**
 * An accessible title to be announced when the dialog is open.
 */
declare function DialogTitle<T extends ValidComponent = "h2">(props: PolymorphicProps<T, DialogTitleProps<T>>): import("@solidjs/web").JSX.Element;
//#endregion
//#region src/dialog/dialog-trigger.d.ts
interface DialogTriggerOptions {}
interface DialogTriggerCommonProps<T extends HTMLElement = HTMLElement> extends ButtonRootCommonProps<T> {
  onClick: JSX$1.EventHandlerUnion<T, MouseEvent>;
}
interface DialogTriggerRenderProps extends DialogTriggerCommonProps, ButtonRootRenderProps {
  "aria-haspopup": "dialog";
  "aria-expanded": "true" | "false";
  "aria-controls": string | undefined;
  "data-expanded": string | undefined;
  "data-closed": string | undefined;
}
type DialogTriggerProps<T extends ValidComponent | HTMLElement = HTMLElement> = DialogTriggerOptions & Partial<DialogTriggerCommonProps<ElementOf<T>>>;
/**
 * The button that opens the dialog.
 */
declare function DialogTrigger<T extends ValidComponent = "button">(props: PolymorphicProps<T, DialogTriggerProps<T>>): JSX$1.Element;
//#endregion
//#region src/dialog/dialog-context.d.ts
interface DialogContextValue {
  translations: Accessor<DialogIntlTranslations>;
  isOpen: Accessor<boolean>;
  modal: Accessor<boolean>;
  preventScroll: Accessor<boolean>;
  contentId: Accessor<string | undefined>;
  titleId: Accessor<string | undefined>;
  descriptionId: Accessor<string | undefined>;
  triggerRef: Accessor<HTMLElement | undefined>;
  overlayRef: Accessor<HTMLElement | undefined>;
  setOverlayRef: Setter<HTMLElement | undefined>;
  contentRef: Accessor<HTMLElement | undefined>;
  setContentRef: Setter<HTMLElement | undefined>;
  overlayPresent: Accessor<boolean>;
  contentPresent: Accessor<boolean>;
  close: () => void;
  toggle: () => void;
  setTriggerRef: Setter<HTMLElement | undefined>;
  generateId: (part: string) => string;
  registerContentId: (id: string) => () => void;
  registerTitleId: (id: string) => () => void;
  registerDescriptionId: (id: string) => () => void;
}
declare function useDialogContext(): DialogContextValue;
declare namespace index_d_exports {
  export { DialogCloseButton as CloseButton, DialogContent as Content, DialogDescription as Description, Dialog, DialogCloseButtonCommonProps, DialogCloseButtonOptions, DialogCloseButtonProps, DialogCloseButtonRenderProps, DialogContentCommonProps, DialogContentOptions, DialogContentProps, DialogContentRenderProps, DialogContextValue, DialogDescriptionCommonProps, DialogDescriptionOptions, DialogDescriptionProps, DialogDescriptionRenderProps, DialogOverlayCommonProps, DialogOverlayOptions, DialogOverlayProps, DialogOverlayRenderProps, DialogPortalProps, DialogRootOptions, DialogRootProps, DialogTitleCommonProps, DialogTitleOptions, DialogTitleProps, DialogTitleRenderProps, DialogTriggerCommonProps, DialogTriggerOptions, DialogTriggerProps, DialogTriggerRenderProps, DialogOverlay as Overlay, DialogPortal as Portal, DialogRoot as Root, DialogTitle as Title, DialogTrigger as Trigger, useDialogContext };
}
declare const Dialog: typeof DialogRoot & {
  CloseButton: typeof DialogCloseButton;
  Content: typeof DialogContent;
  Description: typeof DialogDescription;
  Overlay: typeof DialogOverlay;
  Portal: typeof DialogPortal;
  Title: typeof DialogTitle;
  Trigger: typeof DialogTrigger;
};
//#endregion
export { DialogContent as A, DialogOverlayProps as C, DialogDescriptionOptions as D, DialogDescriptionCommonProps as E, DialogCloseButton as F, DialogCloseButtonCommonProps as I, DialogCloseButtonOptions as L, DialogContentOptions as M, DialogContentProps as N, DialogDescriptionProps as O, DialogContentRenderProps as P, DialogCloseButtonProps as R, DialogOverlayOptions as S, DialogDescription as T, DialogRootProps as _, DialogTrigger as a, DialogOverlay as b, DialogTriggerProps as c, DialogTitleCommonProps as d, DialogTitleOptions as f, DialogRootOptions as g, DialogRoot as h, useDialogContext as i, DialogContentCommonProps as j, DialogDescriptionRenderProps as k, DialogTriggerRenderProps as l, DialogTitleRenderProps as m, index_d_exports as n, DialogTriggerCommonProps as o, DialogTitleProps as p, DialogContextValue as r, DialogTriggerOptions as s, Dialog as t, DialogTitle as u, DialogPortal as v, DialogOverlayRenderProps as w, DialogOverlayCommonProps as x, DialogPortalProps as y, DialogCloseButtonRenderProps as z };