// app-game spector backend/commands/scissor.ts:10 (baseCommand.ts:8): an exported
// subclass overrides a method of its abstract base; the override calls a member of
// an element of its `IArguments` parameter.
abstract class BaseCommand {
  protected abstract stringifyArgs(args: IArguments): string[];

  describe(...args: number[]): string {
    return this.stringifyArgs(arguments).join(",") + args.length;
  }
}

export class Scissor extends BaseCommand {
  protected stringifyArgs(args: IArguments): string[] {
    const stringified: string[] = [];
    for (let i = 0; i < 4; i++) {
      stringified.push(args[i].toFixed(0));
    }
    return stringified;
  }
}
