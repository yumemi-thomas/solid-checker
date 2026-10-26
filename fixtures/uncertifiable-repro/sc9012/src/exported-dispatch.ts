// app-game ogl core/transform.ts:107: an exported class's method calls a
// member of the object it is handed.
export class Transform {
  parent: Transform | null = null;
  children: Transform[] = [];

  setParent(parent: Transform | null, notifyParent = true): void {
    if (this.parent && parent !== this.parent) this.parent.removeChild(this, false);
    this.parent = parent;
    if (notifyParent && parent) parent.addChild(this, false);
  }

  addChild(child: Transform, notifyChild = true): void {
    if (!~this.children.indexOf(child)) this.children.push(child);
    if (notifyChild) child.setParent(this, false);
  }

  removeChild(child: Transform, notifyChild = true): void {
    this.children = this.children.filter((one) => one !== child);
    if (notifyChild) child.setParent(null, false);
  }
}
