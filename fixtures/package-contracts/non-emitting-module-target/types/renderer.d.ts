// An ambient declaration file that names a *value* export. This is
// `@solidjs/universal@2.0.0-rc.3`'s `types/universal.d.ts`, and it is the case
// the export census gets wrong: `createRenderer` is a runtime name to the
// census, so anything said about it for this artifact case would be a claim
// about a module that never executes. Only the emission premise answers it.
export interface RendererOptions {
	createElement(tag: string): unknown;
}

export declare function createRenderer(options: RendererOptions): unknown;
