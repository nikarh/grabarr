import type { SearchCategory } from '../api/generated/model'

function categoryName(id: number, tree: readonly SearchCategory[]): string {
  for (const parent of tree) {
    if (parent.id === id) return parent.name
    const sub = parent.subcategories.find((s) => s.id === id)
    if (sub) return sub.name
  }
  return `#${String(id)}`
}

/** "all categories", "Movies, TV/HD" or "4 categories". */
export function describeCategories(
  ids: readonly number[],
  tree: readonly SearchCategory[],
): string {
  if (ids.length === 0) return 'all categories'
  if (ids.length <= 2) return ids.map((id) => categoryName(id, tree)).join(', ')
  return `${String(ids.length)} categories`
}
