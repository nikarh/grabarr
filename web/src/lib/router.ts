import { useSyncExternalStore } from 'react'

// A minimal hash router: the backend serves one page, and the hash survives reverse proxies and path prefixes.

export type Screen = 'search' | 'downloads' | 'settings'

export interface Route {
  screen: Screen
  params: URLSearchParams
}

const SCREENS = new Set<string>(['search', 'downloads', 'settings'])

function parse(hash: string): Route {
  const [path = '', query = ''] = hash.replace(/^#\/?/, '').split('?', 2)
  const screen = SCREENS.has(path) ? (path as Screen) : 'search'
  return { screen, params: new URLSearchParams(query) }
}

let current = parse(window.location.hash)
let currentHash = window.location.hash

function subscribe(listener: () => void): () => void {
  window.addEventListener('hashchange', listener)
  return () => {
    window.removeEventListener('hashchange', listener)
  }
}

function snapshot(): Route {
  if (window.location.hash !== currentHash) {
    currentHash = window.location.hash
    current = parse(currentHash)
  }
  return current
}

export function useRoute(): Route {
  return useSyncExternalStore(subscribe, snapshot)
}

export function href(screen: Screen, params?: Record<string, string>): string {
  const query = params ? new URLSearchParams(params).toString() : ''
  return `#/${screen}${query ? `?${query}` : ''}`
}

/** Navigates; `replace` keeps the history entry, e.g. for every keystroke of a search. */
export function navigate(screen: Screen, params?: Record<string, string>, replace = false): void {
  const target = href(screen, params)
  if (replace) {
    window.history.replaceState(null, '', target)
    window.dispatchEvent(new HashChangeEvent('hashchange'))
  } else {
    window.location.hash = target
  }
}
