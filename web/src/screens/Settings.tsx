import {
  Alert,
  Autocomplete,
  Button,
  Checkbox,
  Group,
  Loader,
  Select,
  Stack,
  Text,
} from '@mantine/core'
import { notifications } from '@mantine/notifications'
import { IconAlertTriangle, IconCheck } from '@tabler/icons-react'
import { useState, type ReactNode } from 'react'

import type { Settings } from '../api/generated/model'
import {
  useCategories,
  useDownloadClients,
  useIndexers,
  useSaveSettings,
  useSettings,
} from '../api/hooks'
import { ActionBar } from '../components/ui'
import classes from './Settings.module.css'

export function SettingsScreen() {
  const settings = useSettings()
  if (settings.isPending) {
    return (
      <Group justify="center" p="xl">
        <Loader color="signal" size="sm" />
      </Group>
    )
  }
  if (settings.isError) {
    return (
      <Alert color="red" variant="light" icon={<IconAlertTriangle />} title="Can't load settings">
        {settings.error.message}
      </Alert>
    )
  }
  // Remounted when the saved settings change, so the form starts from them.
  return <SettingsForm key={JSON.stringify(settings.data)} saved={settings.data} />
}

function same(a: Settings, b: Settings): boolean {
  return (
    a.downloadClientId === b.downloadClientId &&
    (a.category ?? '') === (b.category ?? '') &&
    a.indexerIds.length === b.indexerIds.length &&
    a.indexerIds.every((id, i) => id === b.indexerIds[i])
  )
}

function SettingsForm({ saved }: { saved: Settings }) {
  const [draft, setDraft] = useState<Settings>(saved)

  const clients = useDownloadClients()
  const indexers = useIndexers()
  const categories = useCategories(draft.downloadClientId)
  const save = useSaveSettings()
  const dirty = !same(draft, saved)

  const submit = () => {
    save.mutate(draft, {
      onSuccess: () => {
        notifications.show({
          color: 'signal',
          icon: <IconCheck size={18} />,
          title: 'Settings saved',
          message: null,
        })
      },
      onError: (e) => {
        notifications.show({ color: 'red', title: 'Saving failed', message: e.message })
      },
    })
  }

  return (
    <Stack gap="lg">
      <div>
        <Text fw={650} size="xl" lts="-0.02em">
          Settings
        </Text>
        <Text size="sm" c="dimmed">
          Where grabs go. Shared by everyone using this instance.
        </Text>
      </div>

      <Section
        title="Download client"
        description="A qBittorrent client from Prowlarr. Grabs are sent to it, and Downloads shows it."
      >
        {clients.isError ? (
          <Text size="sm" c="red">
            {clients.error.message}
          </Text>
        ) : (
          <Select
            placeholder={clients.isPending ? 'Loading…' : 'Choose a download client'}
            aria-label="Download client"
            data={(clients.data ?? []).map((c) => ({
              value: String(c.id),
              label: c.enabled ? c.name : `${c.name} (disabled)`,
            }))}
            value={draft.downloadClientId === null ? null : String(draft.downloadClientId)}
            onChange={(v) => {
              setDraft((d) => ({ ...d, downloadClientId: v === null ? null : Number(v) }))
            }}
            nothingFoundMessage="Prowlarr has no qBittorrent download client"
            allowDeselect={false}
            size="md"
            classNames={{ input: classes.input }}
          />
        )}
      </Section>

      <Section
        title="Category"
        description="Grabs get this category, and Downloads shows only torrents in it. A new one is created on the first grab."
      >
        <Autocomplete
          placeholder={draft.downloadClientId === null ? 'Choose a client first' : 'e.g. grabarr'}
          aria-label="Category"
          disabled={draft.downloadClientId === null}
          data={categories.data ?? []}
          value={draft.category ?? ''}
          onChange={(v) => {
            setDraft((d) => ({ ...d, category: v === '' ? null : v }))
          }}
          size="md"
          classNames={{ input: classes.input }}
          error={categories.isError ? categories.error.message : undefined}
        />
      </Section>

      <Section
        title="Default indexers"
        description="Searched unless a search picks others. None selected searches all of them."
      >
        {indexers.isError ? (
          <Text size="sm" c="red">
            {indexers.error.message}
          </Text>
        ) : indexers.isPending ? (
          <Loader size="xs" color="signal" />
        ) : (
          <Checkbox.Group
            value={draft.indexerIds.map(String)}
            onChange={(v) => {
              setDraft((d) => ({ ...d, indexerIds: v.map(Number).sort((a, b) => a - b) }))
            }}
          >
            <div className={classes.indexers}>
              {indexers.data.map((indexer) => (
                <Checkbox.Card
                  key={indexer.id}
                  value={String(indexer.id)}
                  className={classes.indexer}
                  radius="md"
                >
                  <Group wrap="nowrap" gap="sm">
                    <Checkbox.Indicator size="sm" />
                    <Text size="sm" fw={500} truncate style={{ flex: 1 }}>
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
            </div>
          </Checkbox.Group>
        )}
      </Section>

      <ActionBar label="Unsaved changes" visible={dirty}>
        <Button
          variant="subtle"
          color="gray"
          onClick={() => {
            setDraft(saved)
          }}
        >
          Discard
        </Button>
        <Button onClick={submit} loading={save.isPending}>
          Save
        </Button>
      </ActionBar>
    </Stack>
  )
}

function Section({
  title,
  description,
  children,
}: {
  title: string
  description: string
  children: ReactNode
}) {
  return (
    <section className={classes.section}>
      <div className={classes.sectionHead}>
        <Text fw={600}>{title}</Text>
        <Text size="sm" c="dimmed">
          {description}
        </Text>
      </div>
      {children}
    </section>
  )
}
