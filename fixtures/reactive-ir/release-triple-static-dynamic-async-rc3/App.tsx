// A promise-valued source with `{ static: true }` on the @solidjs/web@2.0.0-rc.3
// triple. rc.3 declares `dynamic(source)` with one parameter, so each
// two-argument call below is TS2554, and no rc.3 build reads a second
// argument: the source is the lazy memo's compute, which settles a Promise
// like any async source. Nothing throws and something renders, so rc.9's
// static-source rule has nothing to say here.
import { dynamic } from "@solidjs/web";

const Plain = () => <div />;

export const AsyncStatic = dynamic(async () => Plain, { static: true });

export const ResolvedStatic = dynamic(() => Promise.resolve(Plain), { static: true });

// The one-argument twin, legal on every release.
export const AsyncDefault = dynamic(async () => Plain);
