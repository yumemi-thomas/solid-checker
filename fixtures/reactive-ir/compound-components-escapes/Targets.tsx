import { createSignal } from "solid-js";

const [count] = createSignal(0);

function Plain() {
  return <i />;
}

function ClassList() {
  const value = count();
  return <b>{value}</b>;
}

function SetterList() {
  const value = count();
  return <b>{value}</b>;
}

function ProxyList() {
  const value = count();
  return <b>{value}</b>;
}

// Uncertifiable: `Object.assign` writes with [[Set]], so each of these targets
// can receive the member and call it plainly, or answer another value on
// read. Only a function declaration or a `const` arrow target stores it as a
// data property.
class Root {
  static set List(member: () => unknown) {
    member();
  }
  static get List(): () => unknown {
    return Plain;
  }
}

export const ClassTabs = Object.assign(Root, { List: ClassList });

const accessors = {
  set List(member: () => unknown) {
    member();
  },
  get List(): () => unknown {
    return Plain;
  },
};

export const ObjectTabs = Object.assign(accessors, { List: SetterList });

const proxied = new Proxy(Plain, {
  set(_target, _key, member: () => unknown) {
    member();
    return true;
  },
});

export const ProxyTabs = Object.assign(proxied, { List: ProxyList });

export function Screen() {
  return (
    <div>
      <ClassTabs.List />
      <ObjectTabs.List />
      <ProxyTabs.List />
    </div>
  );
}
