import { createEffect, createMemo, createSignal, getOwner, runWithOwner } from "solid-js";

declare function prep(): string;

function h1(cb: () => unknown) {
  cb();
  return 1;
}

// Violation (SC6002; the effect is SC4001 too): a template tag runs its tag
// function on the apply's stack, so the effect it creates has no owner.
export function EffectThroughTag() {
  const [count] = createSignal(0);
  createEffect(
    () => count(),
    () => {
      const hf = (_s: TemplateStringsArray) => {
        createEffect(() => 1, () => {});
      };
      hf`x`;
    },
  );
  return null;
}

// Violation (SC6002): the tag inside a helper's callback the helper calls first.
export function MemoThroughHelperTag() {
  const [count] = createSignal(0);
  createEffect(
    () => count(),
    (value) => {
      h1(() => {
        const hf = (_s: TemplateStringsArray) => {
          createMemo(() => value * 2);
        };
        hf`x`;
      });
    },
  );
  return null;
}

// Uncertifiable: a call in the template's substitutions runs before the tag.
export function SubstitutionCallFirst() {
  const [count] = createSignal(0);
  createEffect(
    () => count(),
    (value) => {
      const hf = (_s: TemplateStringsArray, _p: string) => {
        createMemo(() => value * 2);
      };
      hf`x${prep()}`;
    },
  );
  return null;
}

// Uncertifiable: a call before the tagged template in the apply.
export function CallBeforeTag() {
  const [count] = createSignal(0);
  createEffect(
    () => count(),
    (value) => {
      const hf = (_s: TemplateStringsArray) => {
        createMemo(() => value * 2);
      };
      prep();
      hf`x`;
    },
  );
  return null;
}

// Clean: the tag runs under the captured owner.
export function TagUnderOwner() {
  const [count] = createSignal(0);
  const owner = getOwner();
  createEffect(
    () => count(),
    (value) => {
      const hf = (_s: TemplateStringsArray) => {
        createMemo(() => value * 2);
      };
      runWithOwner(owner, () => hf`x`);
    },
  );
  return null;
}
