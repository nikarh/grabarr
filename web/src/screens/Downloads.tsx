import {
  Alert,
  Anchor,
  Button,
  Checkbox,
  Group,
  List,
  Loader,
  Progress,
  SegmentedControl,
  Stack,
  Text,
  TextInput,
} from '@mantine/core'
import { useDebouncedValue } from '@mantine/hooks'
import { notifications } from '@mantine/notifications'
import {
  IconAlertTriangle,
  IconArrowDown,
  IconArrowUp,
  IconDownload,
  IconSearch,
  IconTrash,
} from '@tabler/icons-react'
import { useWindowVirtualizer } from '@tanstack/react-virtual'
import { memo, useEffect, useMemo, useRef, useState, type ReactNode } from 'react'

import type { Torrent } from '../api/generated/model'
import type { TorrentsState } from '../api/hooks'
import { useSettings } from '../api/hooks'
import { rpc } from '../api/rpc'
import { ActionBar, EmptyState, Sheet, StatusLabel } from '../components/ui'
import ui from '../components/ui.module.css'
import { cx } from '../lib/cx'
import { formatBytes, formatDuration, formatPercent, formatSpeed } from '../lib/format'
import { href } from '../lib/router'
import { isActive, isDone, statusColor } from '../lib/status'
import classes from './Downloads.module.css'

type Filter = 'all' | 'active' | 'done'

export function DownloadsScreen({ state }: { state: TorrentsState }) {
  const settings = useSettings()
  const [filter, setFilter] = useState<Filter>('all')
  const [text, setText] = useState('')
  const [needle] = useDebouncedValue(text.trim().toLowerCase(), 150)
  const [picked, setSelected] = useState<ReadonlySet<string>>(new Set())
  const [confirming, setConfirming] = useState(false)
  const [removing, setRemoving] = useState(false)

  const torrents = state.status === 'loading' ? null : state.torrents
  const all = useMemo(
    () =>
      [...(torrents?.values() ?? [])].sort(
        (a, b) => b.addedOn - a.addedOn || a.name.localeCompare(b.name),
      ),
    [torrents],
  )
  const shown = useMemo(
    () =>
      all.filter(
        (t) =>
          (filter === 'all' ||
            (filter === 'active' && !isDone(t)) ||
            (filter === 'done' && isDone(t))) &&
          (needle === '' || t.name.toLowerCase().includes(needle)),
      ),
    [all, filter, needle],
  )

  // Torrents that are gone drop out of the selection.
  const selected = useMemo(
    () => new Set([...picked].filter((h) => torrents?.has(h) === true)),
    [picked, torrents],
  )

  const stats = useMemo(() => {
    let down = 0
    let up = 0
    let active = 0
    for (const t of all) {
      down += t.downloadSpeed
      up += t.uploadSpeed
      if (isActive(t)) active += 1
    }
    return { down, up, active }
  }, [all])

  const toggle = (hash: string) => {
    setSelected((s) => {
      const next = new Set(s)
      if (!next.delete(hash)) next.add(hash)
      return next
    })
  }

  const remove = async () => {
    const hashes = [...selected]
    setRemoving(true)
    try {
      await rpc.call('torrents.remove', { hashes })
      setSelected(new Set())
      setConfirming(false)
      notifications.show({
        color: 'signal',
        title: `Deleted ${String(hashes.length)} ${hashes.length === 1 ? 'torrent' : 'torrents'}`,
        message: 'Their files were deleted too.',
      })
    } catch (e) {
      notifications.show({
        color: 'red',
        title: 'Delete failed',
        message: e instanceof Error ? e.message : String(e),
      })
    } finally {
      setRemoving(false)
    }
  }

  const selectedTorrents = [...selected].flatMap((h) => {
    const t = torrents?.get(h)
    return t ? [t] : []
  })

  return (
    <Stack gap="md">
      <div className={classes.summary}>
        <div>
          <Text size="xs" c="dimmed" tt="uppercase" fw={600} lts="0.06em">
            Category
          </Text>
          <Text fw={650} size="lg" lh={1.2}>
            {settings.data?.category ?? '—'}
          </Text>
        </div>
        <div className={classes.stats}>
          <Stat label="Torrents" value={String(all.length)} />
          <Stat label="Active" value={String(stats.active)} />
          <Stat
            label="Down"
            value={formatSpeed(stats.down)}
            icon={<IconArrowDown size={12} stroke={2.4} />}
            accent={stats.down > 0}
          />
          <Stat
            label="Up"
            value={formatSpeed(stats.up)}
            icon={<IconArrowUp size={12} stroke={2.4} />}
          />
        </div>
      </div>

      <Group gap="sm" wrap="wrap">
        <SegmentedControl
          value={filter}
          onChange={(v) => {
            setFilter(v)
          }}
          data={[
            { value: 'all', label: 'All' },
            { value: 'active', label: 'In progress' },
            { value: 'done', label: 'Done' },
          ]}
          radius="md"
          className={classes.segments}
        />
        <TextInput
          className={classes.filter}
          placeholder="Filter by name"
          aria-label="Filter by name"
          leftSection={<IconSearch size={16} stroke={1.8} />}
          value={text}
          onChange={(e) => {
            setText(e.currentTarget.value)
          }}
        />
      </Group>

      {state.status === 'error' && (
        <Alert
          color="red"
          variant="light"
          icon={<IconAlertTriangle />}
          title="Can't reach qBittorrent"
        >
          {state.error.message}
          {state.error.code === 'notConfigured' && (
            <>
              {' '}
              <Anchor href={href('settings')} inherit>
                Open Settings
              </Anchor>
            </>
          )}
        </Alert>
      )}

      {state.status === 'loading' ? (
        <EmptyState icon={<Loader size={22} color="signal" />} title="Loading downloads" />
      ) : all.length === 0 && state.status === 'ready' ? (
        <EmptyState icon={<IconDownload size={24} stroke={1.6} />} title="No downloads yet">
          <Text size="sm">
            What you grab from <Anchor href={href('search')}>Search</Anchor> shows up here.
          </Text>
        </EmptyState>
      ) : shown.length === 0 && all.length > 0 ? (
        <EmptyState icon={<IconSearch size={24} stroke={1.6} />} title="Nothing matches" />
      ) : (
        shown.length > 0 && <TorrentList torrents={shown} selected={selected} onToggle={toggle} />
      )}

      {selected.size > 0 && (
        <ActionBar label={`${String(selected.size)} selected`}>
          <Button
            variant="subtle"
            color="gray"
            onClick={() => {
              setSelected(new Set())
            }}
          >
            Clear
          </Button>
          <Button
            color="red"
            leftSection={<IconTrash size={16} />}
            onClick={() => {
              setConfirming(true)
            }}
          >
            Delete
          </Button>
        </ActionBar>
      )}

      <Sheet
        opened={confirming}
        onClose={() => {
          if (!removing) setConfirming(false)
        }}
        title={`Delete ${String(selectedTorrents.length)} ${selectedTorrents.length === 1 ? 'torrent' : 'torrents'}?`}
      >
        <Stack gap="md">
          <Alert color="red" variant="light" icon={<IconAlertTriangle />}>
            They are removed from qBittorrent and <b>their files are deleted from disk</b>. This
            can&apos;t be undone.
          </Alert>
          <List size="sm" spacing={4} className={classes.confirmList}>
            {selectedTorrents.slice(0, 6).map((t) => (
              <List.Item key={t.hash}>
                <Text size="sm" lineClamp={1}>
                  {t.name}
                </Text>
              </List.Item>
            ))}
          </List>
          {selectedTorrents.length > 6 && (
            <Text size="sm" c="dimmed">
              and {selectedTorrents.length - 6} more
            </Text>
          )}
          <Group justify="flex-end">
            <Button
              variant="subtle"
              color="gray"
              disabled={removing}
              onClick={() => {
                setConfirming(false)
              }}
            >
              Cancel
            </Button>
            <Button
              color="red"
              loading={removing}
              onClick={() => {
                void remove()
              }}
            >
              Delete torrents and files
            </Button>
          </Group>
        </Stack>
      </Sheet>
    </Stack>
  )
}

function Stat({
  label,
  value,
  icon,
  accent = false,
}: {
  label: string
  value: string
  icon?: ReactNode
  accent?: boolean
}) {
  return (
    <div className={classes.stat} data-accent={accent}>
      <span className={classes.statLabel}>{label}</span>
      <span className={cx('mono', classes.statValue)}>
        {icon}
        {value}
      </span>
    </div>
  )
}

function TorrentList({
  torrents,
  selected,
  onToggle,
}: {
  torrents: Torrent[]
  selected: ReadonlySet<string>
  onToggle: (hash: string) => void
}) {
  const listRef = useRef<HTMLDivElement>(null)
  const [scrollMargin, setScrollMargin] = useState(0)
  useEffect(() => {
    setScrollMargin(listRef.current?.offsetTop ?? 0)
  }, [])

  const virtualizer = useWindowVirtualizer({
    count: torrents.length,
    estimateSize: () => 92,
    overscan: 10,
    scrollMargin,
    getItemKey: (i) => torrents[i]?.hash ?? i,
  })

  return (
    <div ref={listRef} className={ui.list}>
      <div style={{ height: virtualizer.getTotalSize(), position: 'relative' }}>
        {virtualizer.getVirtualItems().map((item) => {
          const torrent = torrents[item.index]
          if (!torrent) return null
          return (
            <div
              key={item.key}
              data-index={item.index}
              ref={virtualizer.measureElement}
              style={{
                position: 'absolute',
                top: 0,
                left: 0,
                width: '100%',
                transform: `translateY(${String(item.start - scrollMargin)}px)`,
              }}
            >
              <TorrentRow
                torrent={torrent}
                selected={selected.has(torrent.hash)}
                onToggle={onToggle}
              />
            </div>
          )
        })}
      </div>
    </div>
  )
}

const TorrentRow = memo(function TorrentRow({
  torrent,
  selected,
  onToggle,
}: {
  torrent: Torrent
  selected: boolean
  onToggle: (hash: string) => void
}) {
  const id = `torrent-${torrent.hash}`
  const downloading = torrent.status === 'downloading' || torrent.status === 'metadata'
  return (
    <div className={ui.row} data-selected={selected}>
      <label className={ui.rowMain} htmlFor={id}>
        <Checkbox
          id={id}
          className={ui.rowCheck}
          checked={selected}
          onChange={() => {
            onToggle(torrent.hash)
          }}
          aria-label={torrent.name}
        />
        <div className={ui.rowBody}>
          <div className={ui.rowTitle}>{torrent.name}</div>
          <div className={classes.progressLine}>
            <Progress
              value={torrent.progress * 100}
              color={statusColor(torrent.status)}
              className={classes.progress}
              animated={downloading}
              striped={downloading}
              aria-label={`${formatPercent(torrent.progress)} done`}
            />
            <span className={cx('mono', classes.percent)}>{formatPercent(torrent.progress)}</span>
          </div>
          <div className={ui.rowMeta}>
            <StatusLabel status={torrent.status} />
            <span className="mono">{formatBytes(torrent.size)}</span>
            {torrent.downloadSpeed > 0 && (
              <span className={cx('mono', classes.speed)}>
                <IconArrowDown size={12} stroke={2.4} />
                {formatSpeed(torrent.downloadSpeed)}
              </span>
            )}
            {torrent.uploadSpeed > 0 && (
              <span className={cx('mono', classes.speed)}>
                <IconArrowUp size={12} stroke={2.4} />
                {formatSpeed(torrent.uploadSpeed)}
              </span>
            )}
            {torrent.eta !== null && torrent.progress < 1 && (
              <span className="mono">{formatDuration(torrent.eta)} left</span>
            )}
          </div>
        </div>
      </label>
    </div>
  )
})
