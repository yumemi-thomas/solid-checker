import {
  createMemo,
  createProjection,
  createSignal,
  createStore,
} from "solid-js";

const [count] = createSignal(0);

export const assignedAwait = createMemo(async () => {
  let result;
  result = await Promise.resolve(1);
  return result + count();
});

export const argumentAwait = createMemo(async () => {
  console.log(await Promise.resolve(1));
  return count();
});

export const templateAwait = createMemo(async () => {
  const url = `/api/${await Promise.resolve("id")}`;
  return url + count();
});

export const nestedBlock = createMemo(async () => {
  {
    await Promise.resolve();
  }
  return count();
});

export const tryBranch = createMemo(async () => {
  await Promise.resolve();
  try {
    return count();
  } catch {
    return 0;
  }
});

export const ifBranch = createMemo(async () => {
  await Promise.resolve();
  if (Math.random() > 0.5) return count();
  return 0;
});

export const ternaryBranch = createMemo(async () => {
  await Promise.resolve();
  return Math.random() > 0.5 ? count() : 0;
});

export const logicalBranch = createMemo(async () => {
  await Promise.resolve();
  return Math.random() > 0.5 && count();
});

export const functionSignal = createSignal(async () => {
  await Promise.resolve();
  return count();
});

export const functionStore = createStore(async () => {
  await Promise.resolve();
  return { count: count() };
}, { count: 0 });

export const projection = createProjection(async draft => {
  await Promise.resolve();
  draft.count = count();
}, { count: 0 });
