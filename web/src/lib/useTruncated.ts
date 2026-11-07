import { useEffect, useState, type RefObject } from 'react'

/** Whether a line-clamped element cuts its text off, kept up to date as its size changes. */
export function useTruncated(ref: RefObject<HTMLElement | null>): boolean {
  const [truncated, setTruncated] = useState(false)
  useEffect(() => {
    const element = ref.current
    if (!element) return
    const check = () => {
      setTruncated(element.scrollHeight > element.clientHeight + 1)
    }
    const observer = new ResizeObserver(check)
    observer.observe(element)
    return () => {
      observer.disconnect()
    }
  }, [ref])
  return truncated
}
