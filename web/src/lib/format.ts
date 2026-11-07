const UNITS = ['B', 'KB', 'MB', 'GB', 'TB', 'PB']

export function formatBytes(bytes: number): string {
  if (bytes < 1) return '0 B'
  const exp = Math.min(UNITS.length - 1, Math.floor(Math.log(bytes) / Math.log(1024)))
  const value = bytes / 1024 ** exp
  const digits = value >= 100 || exp === 0 ? 0 : 1
  return `${value.toFixed(digits)} ${UNITS[exp] ?? 'B'}`
}

export function formatSpeed(bytesPerSecond: number): string {
  return `${formatBytes(bytesPerSecond)}/s`
}

export function formatDuration(seconds: number): string {
  if (seconds < 60) return `${String(Math.max(1, Math.round(seconds)))}s`
  const minutes = Math.round(seconds / 60)
  if (minutes < 60) return `${String(minutes)}m`
  const hours = Math.floor(minutes / 60)
  if (hours < 48) return `${String(hours)}h ${String(minutes % 60)}m`
  return `${String(Math.round(hours / 24))}d`
}

/** "3d", "5h", "2mo": how long ago, compactly. */
export function formatAge(iso: string | null, now: number = Date.now()): string | null {
  if (iso === null) return null
  const then = Date.parse(iso)
  if (Number.isNaN(then)) return null
  const minutes = Math.max(0, (now - then) / 60_000)
  if (minutes < 60) return `${String(Math.round(minutes))}m`
  const hours = minutes / 60
  if (hours < 24) return `${String(Math.round(hours))}h`
  const days = hours / 24
  if (days < 60) return `${String(Math.round(days))}d`
  if (days < 730) return `${String(Math.round(days / 30))}mo`
  return `${String(Math.round(days / 365))}y`
}

export function formatPercent(progress: number): string {
  const percent = progress * 100
  if (percent >= 100) return '100%'
  return `${percent.toFixed(percent < 10 ? 1 : 0)}%`
}
