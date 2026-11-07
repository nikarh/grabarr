import type { Torrent, TorrentStatus } from '../api/generated/model'

export const STATUS: Record<TorrentStatus, { label: string; color: string; live?: boolean }> = {
  downloading: { label: 'Downloading', color: 'var(--status-active)', live: true },
  stalled: { label: 'Stalled', color: 'var(--status-warn)' },
  metadata: { label: 'Fetching metadata', color: 'var(--status-active)', live: true },
  queued: { label: 'Queued', color: 'var(--status-queued)' },
  checking: { label: 'Checking', color: 'var(--status-queued)', live: true },
  seeding: { label: 'Seeding', color: 'var(--status-good)' },
  completed: { label: 'Completed', color: 'var(--status-good)' },
  paused: { label: 'Paused', color: 'var(--status-muted)' },
  moving: { label: 'Moving', color: 'var(--status-queued)', live: true },
  error: { label: 'Error', color: 'var(--status-bad)' },
  unknown: { label: 'Unknown', color: 'var(--status-muted)' },
}

export function statusColor(status: TorrentStatus): string {
  return STATUS[status].color
}

const ACTIVE = new Set<TorrentStatus>([
  'downloading',
  'stalled',
  'metadata',
  'queued',
  'checking',
  'moving',
])
const DONE = new Set<TorrentStatus>(['seeding', 'completed'])

/** Still working towards completion. */
export function isActive(t: Torrent): boolean {
  return ACTIVE.has(t.status)
}

export function isDone(t: Torrent): boolean {
  return DONE.has(t.status)
}
