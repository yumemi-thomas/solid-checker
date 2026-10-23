import { createSignal, onCleanup } from "solid-js";

// A submit button's `click()` runs form submission: interactive validation
// (`invalid`, then the focus move), `submit`, and `formdata`; a reset button
// fires `reset`. Those handlers stay obligations; scroll is never reached.
function press(target: HTMLButtonElement) {
  target.click();
}

export function Submit() {
  const [count, setCount] = createSignal(0);
  return (
    <div>
      <form onSubmit={() => { onCleanup(() => undefined); }} />
      <form onFormData={() => { onCleanup(() => undefined); }} />
      <form onReset={() => { onCleanup(() => undefined); }} />
      <input required onInvalid={() => { onCleanup(() => undefined); }} />
      <form onSubmit={() => { setCount(count() + 1); }} />
      <form onScroll={() => { onCleanup(() => undefined); }} />
      <div onScroll={() => { setCount(count() + 1); }}>scroll write</div>
      <button onClick={(event) => press(event.currentTarget as HTMLButtonElement)}>press</button>
    </div>
  );
}
