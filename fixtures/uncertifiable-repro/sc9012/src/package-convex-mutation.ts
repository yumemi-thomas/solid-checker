// expenses-app convex/user.ts:29 (and 13 more handlers): a handler option
// handed to a package's `mutationGeneric` calls a method of its `ctx` parameter.
// The package registers it and runs it with a context it builds, so no project
// call fixes which `ctx.db.insert` is selected.
import { mutationGeneric } from "convex/server";
import { v } from "convex/values";

export const persist = mutationGeneric({
  args: { name: v.string() },
  handler: async (ctx, args) => {
    await ctx.db.insert("users", { name: args.name });
  },
});
