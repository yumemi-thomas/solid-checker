export type ExternalSourceFactory = (fn: (prev: any) => any, trigger: () => void) => ExternalSource;
export interface ExternalSource {
    track: (prev: any) => any;
    dispose: () => void;
}
export interface ExternalSourceConfig {
    factory: ExternalSourceFactory;
    untrack?: <T>(fn: () => T) => T;
}
export declare let externalSourceConfig: {
    factory: ExternalSourceFactory;
    untrack: <T>(fn: () => T) => T;
} | null;
export declare function enableExternalSource(config: ExternalSourceConfig): void;
export declare function _resetExternalSourceConfig(): void;
