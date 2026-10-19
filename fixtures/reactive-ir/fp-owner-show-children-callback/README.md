# fp-owner-show-children-callback

Only the function that *is* a control-flow component's children value (sole `{...}` child or `children={...}`) has accessor parameters; any other function in the element is a callback of its own element. Non-keyed `<Show>` function children and `<For>`'s index are accessors (`uncalled-accessor`); default-keyed `<For>` item, `onChange` event and `ref` element are plain values (none). Stub copies every claim-bearing declaration verbatim from rc.9 typings; `tsc` strict against stub and real `solid-js`/`@solidjs/web` rc.9.
