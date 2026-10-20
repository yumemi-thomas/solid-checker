export interface NavigationOptions {
  onActivate?: (id: string) => void;
  scrollTo?: (id: string) => void;
}

// The options callbacks run only from the keydown listener of the props the
// returned `rowProps` builds.
export function makeNavigation(props: NavigationOptions) {
  function handleKeyDown(event: { key: string }, id: string) {
    if (event.key === "Enter") props.onActivate?.(id);
    props.scrollTo?.(id);
  }
  return {
    rowProps(id: string) {
      return { onKeyDown: (event: { key: string }) => handleKeyDown(event, id) };
    },
  };
}

// The callback runs whenever the returned closure runs.
export function retain(options: { onRun: () => void }) {
  return () => options.onRun();
}

// Calls an absent `log` with the options object as `this` first.
export function retainWithLog(options: { onRun: () => void; log?: () => void }) {
  return () => {
    options.log?.();
    options.onRun();
  };
}
