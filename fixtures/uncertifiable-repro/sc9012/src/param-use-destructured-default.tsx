// app-game math-examples/affine-transformations-3d.tsx:36 (ogl core/renderer.ts:452): a component hands an
// object to a project method whose parameter is destructured with a default.
type RenderOptions = { scene: object; camera?: object; update?: boolean };

class Renderer {
  draw(scene: object, camera: object): void {
    void [scene, camera];
  }

  render({ scene, camera, update = true }: RenderOptions): void {
    if (update && camera) this.draw(scene, camera);
  }
}

export function View() {
  const renderer = new Renderer();
  const scene = {};
  const camera = {};
  renderer.render({ scene, camera });
  return <canvas />;
}
