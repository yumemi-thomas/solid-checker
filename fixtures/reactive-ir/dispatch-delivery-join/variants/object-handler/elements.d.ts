// `section` is not in the shared stub: declaration merging adds it here. The
// handler type is `EventHandlerWithOptions`-shaped, as in @solidjs/web.
declare namespace JSX {
  interface IntrinsicElements {
    section: {
      children?: unknown;
      onScroll?: ((event: Event) => void) | ({ handleEvent(event: Event): void } & AddEventListenerOptions);
    };
  }
}
