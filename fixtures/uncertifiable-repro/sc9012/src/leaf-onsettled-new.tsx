// app-game paint webgl-whiteboard/index.tsx:17: onSettled builds an object of a class the project declares.
import { onSettled } from "solid-js";

class Context {
  constructor(readonly canvas: HTMLCanvasElement) {
    canvas.width = window.innerWidth;
  }
}

export default function Board() {
  const canvas = <canvas width="800" height="600" /> as HTMLCanvasElement;
  onSettled(() => {
    const context = new Context(canvas);
    context.canvas.width = 1;
  });
  return <div>{canvas}</div>;
}
