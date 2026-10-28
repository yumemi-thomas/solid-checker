import {
  createEffect,
  createMemo,
  createRenderEffect,
  createRoot,
  createSignal,
  runWithOwner,
  untrack,
} from "solid-js";

function callIt(cb: () => void) {
  cb();
}

// Violations: the primitive runs the function it names as it runs an inline
// callback, and the layers run on its stack (rc.17 throws
// REACTIVE_WRITE_IN_OWNED_SCOPE in each).

export function MemoIifeByName() {
  const [v, setV] = createSignal(0);
  const compute = () => {
    (() => setV(1))();
    return 1;
  };
  const m = createMemo(compute);
  return <div>{m()}{v()}</div>;
}

export function MemoForEachByDeclaration() {
  const [v, setV] = createSignal(0);
  function compute() {
    [1].forEach(() => setV(1));
    return 1;
  }
  const m = createMemo(compute);
  return <div>{m()}{v()}</div>;
}

export function MemoHelperByName() {
  const [v, setV] = createSignal(0);
  const compute = () => {
    callIt(() => setV(1));
    return 1;
  };
  const m = createMemo(compute);
  return <div>{m()}{v()}</div>;
}

export function MemoIifeByNameAlsoHandler() {
  const [v, setV] = createSignal(0);
  const compute = () => {
    (() => setV(1))();
    return 1;
  };
  const m = createMemo(compute);
  return (
    <div>
      {m()}
      {v()}
      <button onClick={compute} />
    </div>
  );
}

export function EffectComputeIifeByName() {
  const [v, setV] = createSignal(0);
  const compute = () => {
    (() => setV(1))();
    return 1;
  };
  createEffect(compute, () => {});
  return <div>{v()}</div>;
}

export function RenderComputeForEachByName() {
  const [v, setV] = createSignal(0);
  const compute = () => {
    [1].forEach(() => setV(1));
    return 1;
  };
  createRenderEffect(compute, () => {});
  return <div>{v()}</div>;
}

export function RootIifeByName() {
  const [v, setV] = createSignal(0);
  const init = () => {
    (() => setV(1))();
  };
  createRoot(init);
  return <div>{v()}</div>;
}

export function RootHelperByNameAlsoHandler() {
  const [v, setV] = createSignal(0);
  const init = () => {
    callIt(() => setV(1));
  };
  createRoot(init);
  return (
    <div>
      {v()}
      <button onClick={init} />
    </div>
  );
}

export function UntrackIifeByName() {
  const [v, setV] = createSignal(0);
  const run = () => {
    (() => setV(1))();
  };
  untrack(run);
  return <div>{v()}</div>;
}

export function UntrackForEachByNameAlsoHandler() {
  const [v, setV] = createSignal(0);
  function run() {
    [1].forEach(() => setV(1));
  }
  untrack(run);
  return (
    <div>
      {v()}
      <button onClick={run} />
    </div>
  );
}

// Legal: no owner, a fresh stack, a handler, a function shadowed by another
// of the same name, an effect apply.

export function NullOwnerIifeByName() {
  const [v, setV] = createSignal(0);
  const run = () => {
    (() => setV(1))();
  };
  runWithOwner(null, run);
  return <div>{v()}</div>;
}

export function NullOwnerForEachByDeclaration() {
  const [v, setV] = createSignal(0);
  function run() {
    [1].forEach(() => setV(1));
  }
  runWithOwner(null, run);
  return <div>{v()}</div>;
}

export function MemoTimerByName() {
  const [v, setV] = createSignal(0);
  const compute = () => {
    setTimeout(() => setV(1));
    return 1;
  };
  const m = createMemo(compute);
  return <div>{m()}{v()}</div>;
}

export function EffectComputeTimerByName() {
  const [v, setV] = createSignal(0);
  const compute = () => {
    setTimeout(() => setV(1));
    return 1;
  };
  createEffect(compute, () => {});
  return <div>{v()}</div>;
}

export function UntrackByNameInHandler() {
  const [v, setV] = createSignal(0);
  const run = () => {
    (() => setV(1))();
  };
  return (
    <div>
      {v()}
      <button onClick={() => untrack(run)} />
    </div>
  );
}

export function MemoShadowedName() {
  const [v, setV] = createSignal(0);
  const compute = () => {
    (() => setV(1))();
    return 1;
  };
  const inner = () => {
    const compute = () => 1;
    return createMemo(compute);
  };
  const m = inner();
  return (
    <div>
      {m()}
      {v()}
      <button onClick={compute} />
    </div>
  );
}

export function EffectComputeShadowedName() {
  const [v, setV] = createSignal(0);
  const compute = () => {
    (() => setV(1))();
    return 1;
  };
  const inner = () => {
    const compute = () => 1;
    createEffect(compute, () => {});
  };
  inner();
  return (
    <div>
      {v()}
      <button onClick={compute} />
    </div>
  );
}

export function EffectApplyIifeByName() {
  const [v, setV] = createSignal(0);
  const apply = () => {
    (() => setV(1))();
  };
  createEffect(() => 1, apply);
  return <div>{v()}</div>;
}

// Uncertifiable: the first run of a render-effect apply is unproven (ADR
// 0460); a function with entries the walk does not place; a name that may
// hold another function; an alias; a call before the declaration's scope.

export function RenderApplyIifeByName() {
  const [v, setV] = createSignal(0);
  const apply = () => {
    (() => setV(1))();
  };
  createRenderEffect(() => 1, apply);
  return <div>{v()}</div>;
}

export function RenderApplyIifeByNameAlsoHandler() {
  const [v, setV] = createSignal(0);
  const apply = () => {
    (() => setV(1))();
  };
  createRenderEffect(() => 1, apply);
  return (
    <div>
      {v()}
      <button onClick={apply} />
    </div>
  );
}

export function NullOwnerByNameAlsoHandlerCall() {
  const [v, setV] = createSignal(0);
  const run = () => {
    (() => setV(1))();
  };
  runWithOwner(null, run);
  return (
    <div>
      {v()}
      <button onClick={() => run()} />
    </div>
  );
}

export function MemoReassignedName() {
  const [v, setV] = createSignal(0);
  let compute = () => {
    (() => setV(1))();
    return 1;
  };
  compute = () => 1;
  const m = createMemo(compute);
  return <div>{m()}{v()}</div>;
}

export function MemoAliasedName() {
  const [v, setV] = createSignal(0);
  const compute = () => {
    (() => setV(1))();
    return 1;
  };
  const alias = compute;
  const m = createMemo(alias);
  return <div>{m()}{v()}</div>;
}

export function RootByNameInHandler() {
  const [v, setV] = createSignal(0);
  const init = () => {
    (() => setV(1))();
  };
  return (
    <div>
      {v()}
      <button onClick={() => createRoot(init)} />
    </div>
  );
}
