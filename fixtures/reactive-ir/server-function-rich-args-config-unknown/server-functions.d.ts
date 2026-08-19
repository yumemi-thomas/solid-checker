declare module "@solidjs/web/server-functions" {
  export function configureServerFunctionsClient(options: {
    serializeArgs?: (args: unknown[]) => string;
  }): void;
}
