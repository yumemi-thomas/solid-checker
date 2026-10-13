export declare function taggedForm(tag: (parts: TemplateStringsArray) => string): string;
export declare function getAccessorForm(): number;
export declare function setAccessorForm(): void;
export declare function plainMemberForm(): number;
export declare function unknownMemberForm(bag: Record<string, unknown>): unknown;
export declare function spreadForm(values: Iterable<number>): number[];
export declare function forOfForm(values: Iterable<number>): number;
export declare function instanceofForm(value: unknown): boolean;
export declare function awaitThenableForm(value: PromiseLike<number>): Promise<number>;
export declare function coercionForm(value: { toString(): string }): string;
export declare function objectSpreadForm(
  options: Record<string, unknown>,
): Record<string, unknown>;
export declare function capturedTaggedForm(
  tag: (parts: TemplateStringsArray) => string,
): () => string;
export declare function plainCallForm(callback: () => void): void;
