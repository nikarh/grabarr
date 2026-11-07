import { createContext, useContext } from 'react'

/**
 * The element that scrolls: the content area between the header and the tab bar. The document itself never scrolls,
 * so that nothing is anchored to a viewport that mobile browsers resize while hiding their toolbars.
 */
export const ScrollerContext = createContext<HTMLElement | null>(null)

export function useScroller(): HTMLElement | null {
  return useContext(ScrollerContext)
}
