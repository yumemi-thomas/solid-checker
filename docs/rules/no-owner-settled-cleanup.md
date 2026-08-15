# no-owner-settled-cleanup

`SC4004` · **error** · violation (uncertifiable for exported functions)

An `onSettled` callback returns a cleanup function, but no owner can register
the cleanup.

## What it does

Flags `onSettled` calls whose callback returns a cleanup function while the
call executes without a component, computation, or root owner. When the call
sits in an exported function whose call sites are outside the project, the
finding is reported as **uncertifiable** instead: solid-checker cannot prove
callers provide an owner. Both forms carry **error** severity — unlike the
other owner rules, whose proven form is a warning, this one mirrors a runtime
throw.

## Why is this bad?

`onSettled` may return a cleanup function after the current reactive activity
settles. That function only has a lifetime when Solid can attach it to an
owner that will eventually dispose. In development, returning a cleanup with
no owner **throws `SETTLED_CLEANUP_UNOWNED`**. In production the guard is
compiled out: setup runs but the returned cleanup is silently dropped, so an
interval, listener, socket, or subscription can leak.

## Examples

Examples of **incorrect** code for this rule:

```tsx
// Module scope has no owner, so the returned cleanup is dropped.
onSettled(() => {
  const id = setInterval(poll, 5000);
  return () => clearInterval(id);
});

button.onclick = () => {
  onSettled(() => {
    subscribe();
    return () => unsubscribe();
  });
};
```

Examples of **correct** code for this rule:

```tsx
function Poller() {
  onSettled(() => {
    const id = setInterval(poll, 5000);
    return () => clearInterval(id);
  });
  return <Status />;
}

const stopPolling = createRoot((dispose) => {
  onSettled(() => {
    subscribe();
    return () => unsubscribe();
  });
  return dispose;
});

// Later, when the standalone setup should end:
stopPolling();
```

## How to fix

Call `onSettled` where an owner is active, such as a component body or
computation. For deliberate standalone setup, wrap the call in `createRoot`,
return and retain its `dispose` callback, and invoke that callback when the
setup should end. An event handler may use `onSettled` to defer work until the
graph is idle, but it has no cleanup lifetime of its own; perform teardown
explicitly instead of returning it there.

## Related

- [no-owner-cleanup](no-owner-cleanup.md) — `onCleanup` without an owner
- [invalid-cleanup-return](invalid-cleanup-return.md) — what a returned cleanup may be
