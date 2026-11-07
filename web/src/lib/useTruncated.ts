import { useEffect, useState, type RefObject } from 'react'

/**
 * Whether an element with a max height cuts its text off, kept up to date as its size changes. `layout` is anything
 * else that changes how the text flows (the float beside it), to check again when it does.
 */
export function useTruncated(ref: RefObject<HTMLElement | null>, layout?: unknown): boolean {
  const [truncated, setTruncated] = useState(false)
  useEffect(() => {
    const element = ref.current
    if (!element) return
    const check = () => {
      // The height of the text itself: with `overflow: clip`, scrollHeight doesn't count what's cut off.
      const range = document.createRange()
      range.selectNodeContents(element)
      setTruncated(range.getBoundingClientRect().height > element.clientHeight + 1)
    }
    check()
    const observer = new ResizeObserver(check)
    observer.observe(element)
    return () => {
      observer.disconnect()
    }
  }, [ref, layout])
  return truncated
}
