// Intrinsic elements the event-type variants write, merged beside the shared
// `solid-js.d.ts` (which declares `div`, `button` and `p`; an element cannot be
// redeclared, so these are the others). Every handler is `(event: Event) => void`.
declare namespace JSX {
  interface IntrinsicElements {
    input: {
      type?: string;
      required?: boolean;
      onFocus?: (event: Event) => void;
      onBlur?: (event: Event) => void;
      onFocusIn?: (event: Event) => void;
      onFocusOut?: (event: Event) => void;
      onInput?: (event: Event) => void;
      onChange?: (event: Event) => void;
      onClick?: (event: Event) => void;
      onInvalid?: (event: Event) => void;
      onBeforeToggle?: (event: Event) => void;
      onBeforeMatch?: (event: Event) => void;
      onScroll?: (event: Event) => void;
    };
    section: {
      children?: unknown;
      onScroll?: (event: Event) => void;
      onSync?: (event: Event) => void;
      onFocus?: (event: Event) => void;
    };
    form: {
      children?: unknown;
      onSubmit?: (event: Event) => void;
      onFormData?: (event: Event) => void;
      onReset?: (event: Event) => void;
      onScroll?: (event: Event) => void;
    };
  }
}
