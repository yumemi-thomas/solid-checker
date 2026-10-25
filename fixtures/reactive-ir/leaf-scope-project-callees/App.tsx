import { createTrackedEffect, onSettled } from "solid-js";
import { Child, Merged, Tool } from "@lib/classes";
import { arrowWork, clicker, domWork, leaky, outer } from "./lib/dom-helpers";
import { doIt, Widget } from "pkg-nosrc";

// ---- Imports that resolve to project files are followed (a relative path,
// or a path alias the compiler resolves inside the program).

// Clean: the relative-import helper does DOM work only.
export function RelativeClean(props: { el: HTMLElement }) {
  onSettled(() => {
    domWork(props.el);
    arrowWork(props.el);
  });
  return <div />;
}

// Violation: the helper registers a cleanup.
export function RelativeForbidden() {
  onSettled(() => {
    leaky();
  });
  return <div />;
}

// Violation: two project helpers deep, in another file.
export function RelativeTransitive() {
  createTrackedEffect(() => {
    outer();
  });
  return <div />;
}

// Refused dispatcher: the helper's `click()` can run a listener, so the leaf
// stays an obligation (ADR 0329) whichever file the call is in.
export function RelativeDispatcher(props: { el: HTMLElement }) {
  onSettled(() => {
    clicker(props.el);
  });
  return <div />;
}
export function DirectDispatcher(props: { el: HTMLElement }) {
  onSettled(() => {
    props.el.click();
  });
  return <div />;
}

// Obligation: a package that ships no source is not a project helper.
export function PackageFunction() {
  onSettled(() => {
    doIt();
  });
  return <div />;
}
export function PackageInstance() {
  const widget = new Widget();
  onSettled(() => {
    widget.run();
  });
  return <div />;
}

// ---- Methods of an exact project instance (ADR 0209), the class imported
// through a path alias.

// Clean: `go` and the `reset` it calls through `this` do DOM work only.
export function InstanceClean(props: { el: HTMLElement }) {
  const tool = new Tool();
  onSettled(() => {
    tool.go(props.el);
  });
  return <div />;
}

// Violation: the method registers a cleanup in the leaf scope.
export function InstanceForbidden() {
  const tool = new Tool();
  createTrackedEffect(() => {
    tool.leak();
  });
  return <div />;
}

// Clean: `own` is declared in the constructed class.
export function InstanceOwnMethod() {
  const child = new Child();
  onSettled(() => {
    child.own();
  });
  return <div />;
}

// Obligation: `inherited` is declared in the base class, not in the
// constructed class (not followed: ADR 0397 leaves inherited methods open).
export function InstanceInheritedMethod() {
  const child = new Child();
  onSettled(() => {
    child.inherited();
  });
  return <div />;
}

// Obligation: the member has two declarations (class and merged interface).
export function InstanceMergedMember() {
  const merged = new Merged();
  onSettled(() => {
    merged.m();
  });
  return <div />;
}

// Obligation: a caller-supplied instance may be another object.
export function ParameterReceiver(props: { tool: Tool }) {
  onSettled(() => {
    props.tool.go(document.body);
  });
  return <div />;
}

// Obligation: an element access names no exact instance.
export function ElementAccessReceiver() {
  const tools = [new Tool()];
  onSettled(() => {
    tools[0].go(document.body);
  });
  return <div />;
}

// ---- Local functions and `const` arrows called plainly.

// Clean: a declaration used before it is written, and an arrow.
export function LocalFunctions() {
  const arrow = () => {
    document.title = "arrow";
  };
  onSettled(() => {
    declared();
    arrow();
  });
  function declared() {
    document.title = "declared";
  }
  return <div />;
}

// Violation: three local hops to a cleanup.
export function LocalChainForbidden() {
  function a() {
    b();
  }
  function b() {
    c();
  }
  const c = () => leaky();
  onSettled(() => {
    a();
  });
  return <div />;
}

// Obligation: a `let` function value may be another function when the
// callback runs.
export function LocalReassigned() {
  let work = () => {
    document.title = "first";
  };
  onSettled(() => {
    work();
  });
  work = () => {};
  return <div />;
}
