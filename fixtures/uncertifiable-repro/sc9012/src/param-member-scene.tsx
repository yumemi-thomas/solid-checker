// app-game math-examples sphere.component.tsx:17 (ogl core/transform.ts:92-117): a component adds an object it built
// to a scene it was handed; the project method `addChild` calls `.setParent` on its argument, a member of a project class.
import { createContext, useContext } from "solid-js";

class Transform {
  parent: Transform | null = null;
  children: Transform[] = [];

  setParent(parent: Transform | null): void {
    this.parent = parent;
    if (parent) parent.addChild(this);
  }

  addChild(child: Transform): void {
    if (!this.children.includes(child)) this.children.push(child);
    child.setParent(this);
  }
}

class Mesh extends Transform {}

const SceneContext = createContext<Transform>(new Transform());

export function MeshComponent() {
  const scene = useContext(SceneContext);
  scene.addChild(new Mesh());
  return <></>;
}
