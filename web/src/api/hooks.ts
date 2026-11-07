import {
  useInfiniteQuery,
  useMutation,
  useQuery,
  useQueryClient,
  type InfiniteData,
} from '@tanstack/react-query'
import { useEffect, useState, useSyncExternalStore } from 'react'

import type { SearchPage, Settings, Torrent, TorrentsEvent } from './generated/model'
import { rpc, RpcError, type ConnectionState } from './rpc'

export const PAGE_SIZE = 100

export function useConnectionState(): ConnectionState {
  return useSyncExternalStore(rpc.subscribeState, rpc.getState)
}

export function useSettings() {
  return useQuery({ queryKey: ['settings'], queryFn: () => rpc.call('settings.get') })
}

export function useSaveSettings() {
  const client = useQueryClient()
  return useMutation({
    mutationFn: (settings: Settings) => rpc.call('settings.update', settings),
    onSuccess: (saved) => {
      client.setQueryData(['settings'], saved)
      // Results depend on the default indexers.
      void client.invalidateQueries({ queryKey: ['search'] })
    },
  })
}

export function useIndexers() {
  return useQuery({ queryKey: ['indexers'], queryFn: () => rpc.call('indexers.list') })
}

export function useDownloadClients() {
  return useQuery({
    queryKey: ['downloadClients'],
    queryFn: () => rpc.call('downloadClients.list'),
  })
}

export function useCategories(downloadClientId: number | null) {
  return useQuery({
    queryKey: ['categories', downloadClientId],
    queryFn: () => rpc.call('categories.list', { downloadClientId: downloadClientId ?? 0 }),
    enabled: downloadClientId !== null,
  })
}

/** `indexerIds` undefined searches the default indexers. */
export function useSearch(query: string, indexerIds: number[] | undefined) {
  return useInfiniteQuery<
    SearchPage,
    RpcError,
    InfiniteData<SearchPage, number>,
    readonly unknown[],
    number
  >({
    queryKey: ['search', query, indexerIds ?? 'default'],
    queryFn: ({ pageParam }) =>
      rpc.call('search', {
        query,
        offset: pageParam,
        limit: PAGE_SIZE,
        ...(indexerIds === undefined ? {} : { indexerIds }),
      }),
    initialPageParam: 0,
    getNextPageParam: (last, pages) => {
      if (last.nextOffset === null) return undefined
      // Some indexers ignore the offset and return the first page again: stop when nothing new came.
      const seen = new Set(pages.slice(0, -1).flatMap((p) => p.releases.map((r) => r.guid)))
      if (pages.length > 1 && last.releases.every((r) => seen.has(r.guid))) return undefined
      return last.nextOffset
    },
    enabled: query.trim() !== '',
    staleTime: 10 * 60_000,
    gcTime: 30 * 60_000,
    retry: false,
  })
}

export type TorrentsState =
  | { status: 'loading' }
  | { status: 'ready'; torrents: Map<string, Torrent> }
  | { status: 'error'; error: RpcError; torrents: Map<string, Torrent> | null }

function apply(state: TorrentsState, event: TorrentsEvent): TorrentsState {
  switch (event.kind) {
    case 'snapshot':
      return { status: 'ready', torrents: new Map(event.torrents.map((t) => [t.hash, t])) }
    case 'update': {
      const previous = state.status === 'loading' ? null : state.torrents
      const torrents = new Map(previous)
      for (const hash of event.removed) torrents.delete(hash)
      for (const t of event.upserted) torrents.set(t.hash, t)
      return { status: 'ready', torrents }
    }
    case 'error':
      return {
        status: 'error',
        error: new RpcError(event.code, event.message),
        torrents: state.status === 'loading' ? null : state.torrents,
      }
  }
}

/** Live torrents of the default category. */
export function useTorrents(): TorrentsState {
  const [state, setState] = useState<TorrentsState>({ status: 'loading' })
  useEffect(
    () =>
      rpc.subscribe(
        'torrents.subscribe',
        (event) => {
          setState((s) => apply(s, event))
        },
        (error) => {
          setState({ status: 'error', error, torrents: null })
        },
      ),
    [],
  )
  return state
}
