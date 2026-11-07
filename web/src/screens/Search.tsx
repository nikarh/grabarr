import {
  ActionIcon,
  Alert,
  Anchor,
  Button,
  Checkbox,
  Group,
  Loader,
  Menu,
  Stack,
  Text,
  TextInput,
  Tooltip,
  UnstyledButton,
} from '@mantine/core'
import { notifications } from '@mantine/notifications'
import {
  IconAlertTriangle,
  IconArrowDown,
  IconArrowUp,
  IconArrowsSort,
  IconCategory,
  IconCheck,
  IconExternalLink,
  IconServer,
  IconSearch,
  IconX,
} from '@tabler/icons-react'
import { useWindowVirtualizer } from '@tanstack/react-virtual'
import { memo, useCallback, useEffect, useMemo, useRef, useState, type SubmitEvent } from 'react'

import type { Indexer, Release, SearchCategory } from '../api/generated/model'
import { useIndexers, useSearch, useSearchCategories, useSettings } from '../api/hooks'
import { rpc, RpcError } from '../api/rpc'
import { CategoryPicker } from '../components/CategoryPicker'
import { TitleToggle } from '../components/TitleToggle'
import { ActionBar, EmptyState, Sheet } from '../components/ui'
import ui from '../components/ui.module.css'
import { describeCategories } from '../lib/categories'
import { cx } from '../lib/cx'
import { formatAge, formatBytes } from '../lib/format'
import { href, navigate } from '../lib/router'
import { SORT_LABELS, SORT_ORDERS, sortReleases, type SortOrder } from '../lib/sort'
import { useNow } from '../lib/useNow'
import { useTruncated } from '../lib/useTruncated'
import classes from './Search.module.css'

type GrabState = { status: 'pending' } | { status: 'done' } | { status: 'failed'; message: string }

export function SearchScreen({ query, sort }: { query: string; sort: SortOrder }) {
  const [input, setInput] = useState(query)
  // undefined: the defaults from the settings.
  const [indexerIds, setIndexerIds] = useState<number[] | undefined>(undefined)
  const [categories, setCategories] = useState<number[] | undefined>(undefined)
  const [picker, setPicker] = useState<'indexers' | 'categories' | null>(null)
  const [selected, setSelected] = useState<ReadonlySet<string>>(new Set())
  const [grabs, setGrabs] = useState<ReadonlyMap<string, GrabState>>(new Map())

  const settings = useSettings()
  const indexers = useIndexers()
  const categoryTree = useSearchCategories()
  const search = useSearch(query, { indexerIds, categories })

  // A new search (typed here, or from history) resets the input and the selection.
  const [shownQuery, setShownQuery] = useState(query)
  if (shownQuery !== query) {
    setShownQuery(query)
    setInput(query)
    setSelected(new Set())
  }

  const releases = useMemo(() => {
    const seen = new Set<string>()
    const all: Release[] = []
    for (const page of search.data?.pages ?? []) {
      for (const r of page.releases) {
        if (seen.has(r.guid)) continue
        seen.add(r.guid)
        all.push(r)
      }
    }
    return all
  }, [search.data])
  const sorted = useMemo(() => sortReleases(releases, sort), [releases, sort])

  const configured =
    settings.data?.downloadClientId != null && (settings.data.category ?? '') !== ''

  const submit = (e: SubmitEvent<HTMLFormElement>) => {
    e.preventDefault()
    const q = input.trim()
    // The sort order carries over to the next search.
    if (q !== '') navigate('search', { q, ...(sort === 'relevance' ? {} : { sort }) })
  }

  const toggle = (guid: string) => {
    setSelected((s) => {
      const next = new Set(s)
      if (!next.delete(guid)) next.add(guid)
      return next
    })
  }

  const grab = async () => {
    const guids = [...selected].filter((g) => grabs.get(g)?.status !== 'done')
    if (guids.length === 0) return
    setGrabs((m) => new Map([...m, ...guids.map((g) => [g, { status: 'pending' }] as const)]))
    // All grabs leave in one frame; each row updates as its own answer arrives.
    const outcomes = await Promise.all(
      guids.map(async (guid) => {
        try {
          await rpc.call('grab', { guid })
          setGrabs((m) => new Map(m).set(guid, { status: 'done' }))
          return true
        } catch (e) {
          const message = e instanceof Error ? e.message : String(e)
          setGrabs((m) => new Map(m).set(guid, { status: 'failed', message }))
          return false
        }
      }),
    )
    const ok = outcomes.filter(Boolean).length
    const failed = outcomes.length - ok
    // What was sent leaves the selection; what failed stays, to retry.
    const sent = new Set(guids.filter((_, i) => outcomes[i] === true))
    setSelected((s) => new Set([...s].filter((g) => !sent.has(g))))
    notifications.show({
      color: failed === 0 ? 'signal' : ok === 0 ? 'red' : 'yellow',
      icon: failed === 0 ? <IconCheck size={18} /> : <IconAlertTriangle size={18} />,
      title:
        failed === 0
          ? `Sent ${String(ok)} to qBittorrent`
          : `${String(ok)} sent, ${String(failed)} failed`,
      message:
        failed === 0 ? (
          <Anchor href={href('downloads')} size="sm">
            Watch them in Downloads
          </Anchor>
        ) : (
          'The failed rows say why.'
        ),
    })
  }

  const indexerLabel = describeIndexers(indexerIds ?? settings.data?.indexerIds, indexers.data)
  const categoryLabel = describeCategories(
    categories ?? settings.data?.searchCategories ?? [],
    categoryTree.data ?? [],
  )
  const closePicker = () => {
    setPicker(null)
  }

  return (
    <Stack gap="md">
      <form onSubmit={submit} className={classes.searchForm}>
        <TextInput
          className={classes.searchInput}
          size="md"
          radius="md"
          placeholder="Search releases"
          aria-label="Search releases"
          value={input}
          onChange={(e) => {
            setInput(e.currentTarget.value)
          }}
          leftSection={<IconSearch size={18} stroke={1.8} />}
          rightSection={
            input !== '' && (
              <ActionIcon
                aria-label="Clear search"
                onClick={() => {
                  setInput('')
                }}
              >
                <IconX size={16} />
              </ActionIcon>
            )
          }
          enterKeyHint="search"
          autoComplete="off"
          autoCapitalize="off"
          spellCheck={false}
        />
        <Button type="submit" size="md" className={classes.searchButton}>
          Search
        </Button>
      </form>

      <div className={classes.scopes}>
        <UnstyledButton
          className={classes.scope}
          onClick={() => {
            setPicker('indexers')
          }}
          data-custom={indexerIds !== undefined}
          aria-label={`Indexers: ${indexerLabel}`}
        >
          <IconServer size={14} stroke={2} />
          <span className={classes.scopeLabel}>{capitalize(indexerLabel)}</span>
        </UnstyledButton>
        <UnstyledButton
          className={classes.scope}
          onClick={() => {
            setPicker('categories')
          }}
          data-custom={categories !== undefined}
          aria-label={`Categories: ${categoryLabel}`}
        >
          <IconCategory size={14} stroke={2} />
          <span className={classes.scopeLabel}>{capitalize(categoryLabel)}</span>
        </UnstyledButton>
      </div>

      {releases.length > 0 && (
        <Group gap="xs" justify="space-between" wrap="nowrap">
          <Text size="xs" c="dimmed" className="mono">
            {releases.length} {search.hasNextPage ? 'loaded' : 'results'}
            {sort !== 'relevance' && search.hasNextPage && ' · sorted'}
          </Text>
          <SortMenu
            value={sort}
            onChange={(order) => {
              navigate(
                'search',
                { q: query, ...(order === 'relevance' ? {} : { sort: order }) },
                true,
              )
            }}
          />
        </Group>
      )}

      {settings.isSuccess && !configured && (
        <Alert
          variant="light"
          color="yellow"
          icon={<IconAlertTriangle size={18} />}
          title="Not ready to grab"
        >
          Choose a download client and a category in{' '}
          <Anchor href={href('settings')} inherit>
            Settings
          </Anchor>{' '}
          first.
        </Alert>
      )}

      {query === '' ? (
        <EmptyState icon={<IconSearch size={24} stroke={1.6} />} title="Find something to grab">
          <Text size="sm" maw={320}>
            Searches {indexerLabel} in {categoryLabel} through Prowlarr. Results load as you scroll.
          </Text>
        </EmptyState>
      ) : search.isPending ? (
        <EmptyState icon={<Loader size={22} color="signal" />} title={`Searching for “${query}”`}>
          <Text size="sm">Indexers can take a few seconds.</Text>
        </EmptyState>
      ) : search.isError && releases.length === 0 ? (
        <Alert color="red" variant="light" title="Search failed" icon={<IconAlertTriangle />}>
          <Text size="sm">{search.error.message}</Text>
          <Button
            mt="sm"
            display="block"
            size="xs"
            variant="light"
            color="red"
            onClick={() => {
              void search.refetch()
            }}
          >
            Try again
          </Button>
        </Alert>
      ) : releases.length === 0 ? (
        <EmptyState icon={<IconSearch size={24} stroke={1.6} />} title="Nothing found">
          <Text size="sm">Try other words, indexers or categories.</Text>
        </EmptyState>
      ) : (
        <ReleaseList
          releases={sorted}
          autoLoad={sort === 'relevance'}
          selected={selected}
          grabs={grabs}
          onToggle={toggle}
          hasMore={search.hasNextPage}
          loadingMore={search.isFetchingNextPage}
          error={search.isError ? search.error : null}
          onLoadMore={() => {
            void search.fetchNextPage()
          }}
        />
      )}

      <ActionBar label={`${String(selected.size)} selected`} visible={selected.size > 0}>
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
          onClick={() => {
            void grab()
          }}
          disabled={!configured}
          loading={[...selected].some((g) => grabs.get(g)?.status === 'pending')}
        >
          Grab
        </Button>
      </ActionBar>

      <IndexerPicker
        opened={picker === 'indexers'}
        onClose={closePicker}
        indexers={indexers.data ?? []}
        defaults={settings.data?.indexerIds ?? []}
        value={indexerIds}
        onChange={setIndexerIds}
      />
      <CategorySheet
        opened={picker === 'categories'}
        onClose={closePicker}
        tree={categoryTree.data ?? []}
        defaults={settings.data?.searchCategories ?? []}
        value={categories}
        onChange={setCategories}
      />
    </Stack>
  )
}

function capitalize(text: string): string {
  return text.charAt(0).toUpperCase() + text.slice(1)
}

function describeIndexers(ids: number[] | undefined, indexers: Indexer[] | undefined): string {
  if (ids === undefined || ids.length === 0) return 'all indexers'
  const names = ids.map((id) => indexers?.find((i) => i.id === id)?.name ?? `#${String(id)}`)
  if (names.length <= 2) return names.join(', ')
  return `${String(names.length)} indexers`
}

function SortMenu({ value, onChange }: { value: SortOrder; onChange: (order: SortOrder) => void }) {
  return (
    <Menu position="bottom-end" shadow="md" radius="md" width={190}>
      <Menu.Target>
        <Button
          variant={value === 'relevance' ? 'subtle' : 'light'}
          color={value === 'relevance' ? 'gray' : 'signal'}
          size="compact-sm"
          leftSection={<IconArrowsSort size={15} stroke={1.8} />}
        >
          {SORT_LABELS[value]}
        </Button>
      </Menu.Target>
      <Menu.Dropdown>
        <Menu.Label>Sort results</Menu.Label>
        {SORT_ORDERS.map((order) => (
          <Menu.Item
            key={order}
            onClick={() => {
              onChange(order)
            }}
            rightSection={order === value ? <IconCheck size={15} /> : null}
          >
            {SORT_LABELS[order]}
          </Menu.Item>
        ))}
      </Menu.Dropdown>
    </Menu>
  )
}

const ESTIMATED_ROW = 78

function ReleaseList({
  releases,
  autoLoad,
  selected,
  grabs,
  onToggle,
  hasMore,
  loadingMore,
  error,
  onLoadMore,
}: {
  releases: readonly Release[]
  // Off while sorted: loading more on reaching the end would sort new results in above the reader.
  autoLoad: boolean
  selected: ReadonlySet<string>
  grabs: ReadonlyMap<string, GrabState>
  onToggle: (guid: string) => void
  hasMore: boolean
  loadingMore: boolean
  error: RpcError | null
  onLoadMore: () => void
}) {
  const listRef = useRef<HTMLDivElement>(null)
  // Here rather than in the rows, which unmount when scrolled out of view.
  const [expanded, setExpanded] = useState<ReadonlySet<string>>(new Set())
  const toggleExpanded = useCallback((guid: string) => {
    setExpanded((e) => {
      const next = new Set(e)
      if (!next.delete(guid)) next.add(guid)
      return next
    })
  }, [])
  const [scrollMargin, setScrollMargin] = useState(0)
  useEffect(() => {
    setScrollMargin(listRef.current?.offsetTop ?? 0)
  }, [])

  const virtualizer = useWindowVirtualizer({
    count: releases.length + 1,
    estimateSize: () => ESTIMATED_ROW,
    overscan: 10,
    scrollMargin,
    getItemKey: (i) => releases[i]?.guid ?? 'footer',
  })
  const items = virtualizer.getVirtualItems()
  const last = items.at(-1)

  // Load the next page as the end comes into view.
  useEffect(() => {
    if (
      autoLoad &&
      last &&
      last.index >= releases.length - 5 &&
      hasMore &&
      !loadingMore &&
      error === null
    ) {
      onLoadMore()
    }
  }, [autoLoad, last, releases.length, hasMore, loadingMore, error, onLoadMore])

  const now = useNow(60_000)
  return (
    <div ref={listRef} className={ui.list}>
      <div style={{ height: virtualizer.getTotalSize(), position: 'relative' }}>
        {items.map((item) => {
          const release = releases[item.index]
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
              {release ? (
                <ReleaseRow
                  release={release}
                  selected={selected.has(release.guid)}
                  expanded={expanded.has(release.guid)}
                  grab={grabs.get(release.guid)}
                  onToggle={onToggle}
                  onExpand={toggleExpanded}
                  now={now}
                />
              ) : (
                <div className={ui.listFooter}>
                  {loadingMore ? (
                    <>
                      <Loader size="xs" color="signal" /> Loading more
                    </>
                  ) : error !== null ? (
                    <>
                      <span>{error.message}</span>
                      <Button size="compact-xs" variant="light" onClick={onLoadMore}>
                        Retry
                      </Button>
                    </>
                  ) : hasMore && !autoLoad ? (
                    <Stack gap={6} align="center">
                      <span>Sorted among the {releases.length} results loaded so far</span>
                      <Button size="compact-sm" variant="light" onClick={onLoadMore}>
                        Load more and sort them in
                      </Button>
                    </Stack>
                  ) : hasMore ? (
                    ''
                  ) : (
                    'End of results'
                  )}
                </div>
              )}
            </div>
          )
        })}
      </div>
    </div>
  )
}

const ReleaseRow = memo(function ReleaseRow({
  release,
  selected,
  expanded,
  grab,
  onToggle,
  onExpand,
  now,
}: {
  release: Release
  selected: boolean
  expanded: boolean
  grab: GrabState | undefined
  onToggle: (guid: string) => void
  onExpand: (guid: string) => void
  now: number
}) {
  const titleRef = useRef<HTMLDivElement>(null)
  const truncated = useTruncated(titleRef)
  const age = formatAge(release.publishDate, now)
  const seeders = release.seeders ?? 0
  const id = `release-${release.guid}`
  return (
    <div className={ui.row} data-selected={selected}>
      <label className={ui.rowMain} htmlFor={id}>
        <Checkbox
          id={id}
          className={ui.rowCheck}
          checked={selected}
          onChange={() => {
            onToggle(release.guid)
          }}
          aria-label={release.title}
        />
        <div className={ui.rowBody}>
          <div className={ui.rowTitle} ref={titleRef} data-expanded={expanded}>
            {release.title}
          </div>
          <div className={ui.rowMeta}>
            <span className={cx('mono', classes.size)}>{formatBytes(release.size)}</span>
            <span
              className={cx('mono', classes.peers)}
              data-health={seeders >= 10 ? 'good' : seeders > 0 ? 'low' : 'none'}
              title="Seeders"
            >
              <IconArrowUp size={12} stroke={2.4} />
              {release.seeders ?? '–'}
            </span>
            <span className={cx('mono', classes.peers)} title="Leechers">
              <IconArrowDown size={12} stroke={2.4} />
              {release.leechers ?? '–'}
            </span>
            {age !== null && <span className="mono">{age}</span>}
            <span className={classes.indexer}>{release.indexer}</span>
          </div>
        </div>
      </label>
      <div className={ui.rowAside}>
        <GrabIndicator grab={grab} />
        {(truncated || expanded) && (
          <TitleToggle
            expanded={expanded}
            name={release.title}
            onToggle={() => {
              onExpand(release.guid)
            }}
          />
        )}
        {release.infoUrl !== null && (
          <Tooltip label="Open on the indexer" openDelay={400}>
            <ActionIcon
              component="a"
              href={release.infoUrl}
              target="_blank"
              rel="noreferrer noopener"
              size="lg"
              aria-label={`Open ${release.title} on ${release.indexer}`}
            >
              <IconExternalLink size={18} stroke={1.7} />
            </ActionIcon>
          </Tooltip>
        )}
      </div>
    </div>
  )
})

function GrabIndicator({ grab }: { grab: GrabState | undefined }) {
  if (grab === undefined) return null
  switch (grab.status) {
    case 'pending':
      return <Loader size={16} color="signal" aria-label="Grabbing" />
    case 'done':
      return (
        <Tooltip label="Sent to qBittorrent">
          <span className={classes.grabbed} role="img" aria-label="Sent to qBittorrent">
            <IconCheck size={16} stroke={2.6} />
          </span>
        </Tooltip>
      )
    case 'failed':
      return (
        <Tooltip label={grab.message} multiline w={260}>
          <span className={classes.grabFailed} role="img" aria-label={grab.message}>
            <IconAlertTriangle size={16} stroke={2} />
          </span>
        </Tooltip>
      )
  }
}

function IndexerPicker({
  opened,
  onClose,
  ...rest
}: {
  opened: boolean
  onClose: () => void
  indexers: Indexer[]
  defaults: number[]
  value: number[] | undefined
  onChange: (ids: number[] | undefined) => void
}) {
  return (
    <Sheet opened={opened} onClose={onClose} title="Indexers for this search">
      {/* Remounted on every opening, to start from the current choice. */}
      <IndexerPickerBody key={String(opened)} onClose={onClose} {...rest} />
    </Sheet>
  )
}

function IndexerPickerBody({
  onClose,
  indexers,
  defaults,
  value,
  onChange,
}: {
  onClose: () => void
  indexers: Indexer[]
  defaults: number[]
  value: number[] | undefined
  onChange: (ids: number[] | undefined) => void
}) {
  const [draft, setDraft] = useState<string[]>(() => (value ?? defaults).map(String))

  return (
    <Stack gap="md">
      <Text size="sm" c="dimmed">
        Only for searches from now on in this tab. The defaults are set in Settings.
      </Text>
      <Checkbox.Group value={draft} onChange={setDraft}>
        <Stack gap={0} className={classes.indexerList}>
          {indexers.map((indexer) => (
            <Checkbox.Card
              key={indexer.id}
              value={String(indexer.id)}
              className={classes.indexerOption}
              radius={0}
            >
              <Group wrap="nowrap" gap="sm">
                <Checkbox.Indicator />
                <Text size="sm" fw={500} style={{ flex: 1 }}>
                  {indexer.name}
                </Text>
                {!indexer.enabled && (
                  <Text size="xs" c="dimmed">
                    disabled
                  </Text>
                )}
              </Group>
            </Checkbox.Card>
          ))}
        </Stack>
      </Checkbox.Group>
      <Text size="xs" c="dimmed">
        None selected searches every indexer.
      </Text>
      <Group justify="space-between">
        <Button
          variant="subtle"
          color="gray"
          onClick={() => {
            onChange(undefined)
            onClose()
          }}
        >
          Use defaults
        </Button>
        <Button
          onClick={() => {
            onChange(draft.map(Number))
            onClose()
          }}
        >
          Apply
        </Button>
      </Group>
    </Stack>
  )
}

function CategorySheet({
  opened,
  onClose,
  ...rest
}: {
  opened: boolean
  onClose: () => void
  tree: readonly SearchCategory[]
  defaults: number[]
  value: number[] | undefined
  onChange: (ids: number[] | undefined) => void
}) {
  return (
    <Sheet opened={opened} onClose={onClose} title="Categories for this search">
      {/* Remounted on every opening, to start from the current choice. */}
      <CategorySheetBody key={String(opened)} onClose={onClose} {...rest} />
    </Sheet>
  )
}

function CategorySheetBody({
  onClose,
  tree,
  defaults,
  value,
  onChange,
}: {
  onClose: () => void
  tree: readonly SearchCategory[]
  defaults: number[]
  value: number[] | undefined
  onChange: (ids: number[] | undefined) => void
}) {
  const [draft, setDraft] = useState<number[]>(() => value ?? defaults)
  return (
    <Stack gap="md">
      <Text size="sm" c="dimmed">
        Only for searches from now on in this tab. The defaults are set in Settings. A whole
        category includes its subcategories; none searches every category.
      </Text>
      <CategoryPicker tree={tree} value={draft} onChange={setDraft} label="Categories" />
      <Group justify="space-between">
        <Button
          variant="subtle"
          color="gray"
          onClick={() => {
            onChange(undefined)
            onClose()
          }}
        >
          Use defaults
        </Button>
        <Button
          onClick={() => {
            onChange(draft)
            onClose()
          }}
        >
          Apply
        </Button>
      </Group>
    </Stack>
  )
}
