import type { CSSProperties } from 'react'

/** The style of a row with this many buttons in its corner, for the float that keeps its text clear of them. */
export function asideStyle(items: number): CSSProperties {
  return { '--aside-items': items } as CSSProperties
}
