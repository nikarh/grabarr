import { useVirtualizer } from '@tanstack/react-virtual'
import { useEffect, useState, type RefObject } from 'react'

import { useScroller } from './scroller'

/** A virtualizer for a list somewhere inside the scrolling content area. */
export function useListVirtualizer({
  listRef,
  count,
  estimateSize,
  getItemKey,
}: {
  listRef: RefObject<HTMLElement | null>
  count: number
  estimateSize: number
  getItemKey: (index: number) => string | number
}) {
  const scroller = useScroller()
  // Where the list starts within the content area (which is its offset parent).
  const [scrollMargin, setScrollMargin] = useState(0)
  useEffect(() => {
    setScrollMargin(listRef.current?.offsetTop ?? 0)
  }, [listRef, scroller])

  // The React Compiler isn't used; this only says it would skip memoizing this hook.
  // oxlint-disable-next-line react-hooks-js/incompatible-library
  const virtualizer = useVirtualizer({
    count,
    getScrollElement: () => scroller,
    estimateSize: () => estimateSize,
    overscan: 10,
    scrollMargin,
    getItemKey,
  })
  return { virtualizer, scrollMargin }
}
