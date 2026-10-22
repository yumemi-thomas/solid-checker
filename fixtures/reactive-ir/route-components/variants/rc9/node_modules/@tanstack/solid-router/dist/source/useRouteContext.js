import { useMatch } from './useMatch';
export function useRouteContext(opts) {
    return useMatch({
        ...opts,
        select: (match) => opts.select ? opts.select(match.context) : match.context,
    });
}
//# sourceMappingURL=useRouteContext.js.map