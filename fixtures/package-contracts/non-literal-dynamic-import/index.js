export const thing = 1;

export function load(name) {
  return import(`./mod-${name}.js`);
}
