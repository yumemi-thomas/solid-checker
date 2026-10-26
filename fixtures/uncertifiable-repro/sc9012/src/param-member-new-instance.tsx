// app-game ogl-examples/raycasting/raycasting.tsx:61 (ogl/core/transform.ts:92-110, spector-iframe.tsx:93 is the
// same with a function): the argument is an instance built with `new` of a project class, and the callee calls a
// method of that class on it (`setParent` calls `parent.addChild`), so no implementation is selected for it.
import { createSignal } from "solid-js";

class Node {
  parent: Node | null = null;
  children: Node[] = [];
  setParent(parent: Node | null, notify = true): void {
    this.parent = parent;
    if (notify && parent) parent.addChild(this, false);
  }
  addChild(child: Node, notify = true): void {
    if (!this.children.includes(child)) this.children.push(child);
    if (notify) child.setParent(this, false);
  }
}

export function Scene() {
  const [count, setCount] = createSignal(0);
  const scene = new Node();
  const mesh = new Node();
  mesh.setParent(scene);
  return <button onClick={() => setCount(scene.children.length + count())}>{count()}</button>;
}
