import { Accessor } from 'solid-js';
import { AnyRouter, RegisteredRouter, ResolveUseSearch, StrictOrFrom, ThrowConstraint, ThrowOrOptional, UseSearchResult } from '@tanstack/router-core';
export interface UseSearchBaseOptions<TRouter extends AnyRouter, TFrom, TStrict extends boolean, TThrow extends boolean, TSelected> {
    select?: (state: ResolveUseSearch<TRouter, TFrom, TStrict>) => TSelected;
    shouldThrow?: TThrow;
}
export type UseSearchOptions<TRouter extends AnyRouter, TFrom, TStrict extends boolean, TThrow extends boolean, TSelected> = StrictOrFrom<TRouter, TFrom, TStrict> & UseSearchBaseOptions<TRouter, TFrom, TStrict, TThrow, TSelected>;
export type UseSearchRoute<out TFrom> = <TRouter extends AnyRouter = RegisteredRouter, TSelected = unknown>(opts?: UseSearchBaseOptions<TRouter, TFrom, true, true, TSelected>) => Accessor<UseSearchResult<TRouter, TFrom, true, TSelected>>;
export declare function useSearch<TRouter extends AnyRouter = RegisteredRouter, const TFrom extends string | undefined = undefined, TStrict extends boolean = true, TThrow extends boolean = true, TSelected = unknown>(opts: UseSearchOptions<TRouter, TFrom, TStrict, ThrowConstraint<TStrict, TThrow>, TSelected>): Accessor<ThrowOrOptional<UseSearchResult<TRouter, TFrom, TStrict, TSelected>, TThrow>>;
