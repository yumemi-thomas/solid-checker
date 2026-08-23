export function drop(list, count = 1) {
  return list.slice(count);
}

const moduleLocal = [1, 2, 3];

export function readModuleLocal() {
  return moduleLocal.slice(1);
}

export function readBodyLocal() {
  const bodyLocal = [1, 2, 3];
  return bodyLocal.slice(1);
}
