import type { Release } from '../api/generated/model'

export const SORT_ORDERS = ['relevance', 'newest', 'oldest', 'largest', 'smallest'] as const
export type SortOrder = (typeof SORT_ORDERS)[number]

export const SORT_LABELS: Record<SortOrder, string> = {
  relevance: 'Relevance',
  newest: 'Newest first',
  oldest: 'Oldest first',
  largest: 'Largest first',
  smallest: 'Smallest first',
}

export function parseSort(value: string | null): SortOrder {
  return SORT_ORDERS.find((o) => o === value) ?? 'relevance'
}

function time(release: Release): number | null {
  if (release.publishDate === null) return null
  const t = Date.parse(release.publishDate)
  return Number.isNaN(t) ? null : t
}

/** Sorts a copy; relevance keeps the indexers' order. Releases without a date go last either way. */
export function sortReleases(releases: readonly Release[], order: SortOrder): readonly Release[] {
  switch (order) {
    case 'relevance':
      return releases
    case 'largest':
      return [...releases].sort((a, b) => b.size - a.size)
    case 'smallest':
      return [...releases].sort((a, b) => a.size - b.size)
    case 'newest':
    case 'oldest': {
      const sign = order === 'newest' ? -1 : 1
      return [...releases].sort((a, b) => {
        const ta = time(a)
        const tb = time(b)
        if (ta === null || tb === null) return ta === tb ? 0 : ta === null ? 1 : -1
        return sign * (ta - tb)
      })
    }
  }
}
