import type { AnyRouter, RegisteredRouter, RouterState } from '@tanstack/router-core';
import type { Accessor } from 'solid-js';
export type UseRouterStateOptions<TRouter extends AnyRouter, TSelected> = {
    router?: TRouter;
    select?: (state: RouterState<TRouter['routeTree']>) => TSelected;
};
export type UseRouterStateResult<TRouter extends AnyRouter, TSelected> = unknown extends TSelected ? RouterState<TRouter['routeTree']> : TSelected;
export declare function useRouterState<TRouter extends AnyRouter = RegisteredRouter, TSelected = unknown>(opts?: UseRouterStateOptions<TRouter, TSelected>): Accessor<UseRouterStateResult<TRouter, TSelected>>;
