import { ActionIcon, Checkbox, Text, TextInput, UnstyledButton } from '@mantine/core'
import { IconChevronDown, IconSearch, IconX } from '@tabler/icons-react'
import { useState } from 'react'

import type { SearchCategory } from '../api/generated/model'
import classes from './CategoryPicker.module.css'

/** "Movies/HD" under "Movies" reads as "HD". */
function shortName(parent: SearchCategory, sub: SearchCategory): string {
  const prefix = `${parent.name}/`
  return sub.name.startsWith(prefix) ? sub.name.slice(prefix.length) : sub.name
}

/**
 * Categories as an inline checklist: a whole category, or some of its subcategories (expand it, or filter). Inline
 * rather than a dropdown, so it works the same in a bottom sheet on a phone as on a settings page.
 */
export function CategoryPicker({
  tree,
  value,
  onChange,
  label,
}: {
  tree: readonly SearchCategory[]
  value: readonly number[]
  onChange: (ids: number[]) => void
  label: string
}) {
  const [filter, setFilter] = useState('')
  const [expanded, setExpanded] = useState<ReadonlySet<number>>(new Set())
  const selected = new Set(value)
  const needle = filter.trim().toLowerCase()

  const update = (change: (ids: Set<number>) => void) => {
    const next = new Set(selected)
    change(next)
    onChange([...next].sort((a, b) => a - b))
  }

  const toggleExpanded = (id: number) => {
    setExpanded((e) => {
      const next = new Set(e)
      if (!next.delete(id)) next.add(id)
      return next
    })
  }

  const groups = tree.flatMap((parent) => {
    const parentMatches = needle === '' || parent.name.toLowerCase().includes(needle)
    const subs = parentMatches
      ? parent.subcategories
      : parent.subcategories.filter((s) => s.name.toLowerCase().includes(needle))
    if (!parentMatches && subs.length === 0) return []
    return [{ parent, subs, open: needle !== '' || expanded.has(parent.id) }]
  })

  return (
    <div className={classes.root}>
      <TextInput
        size="md"
        placeholder="Filter, e.g. hd"
        aria-label={`Filter ${label.toLowerCase()}`}
        value={filter}
        onChange={(e) => {
          setFilter(e.currentTarget.value)
        }}
        leftSection={<IconSearch size={16} stroke={1.8} />}
        rightSection={
          filter !== '' && (
            <ActionIcon
              aria-label="Clear the filter"
              onClick={() => {
                setFilter('')
              }}
            >
              <IconX size={15} />
            </ActionIcon>
          )
        }
        classNames={{ input: classes.filter }}
      />
      <div className={classes.list} role="group" aria-label={label}>
        {groups.length === 0 && (
          <Text size="sm" c="dimmed" p="md">
            No such category
          </Text>
        )}
        {groups.map(({ parent, subs, open }) => {
          const whole = selected.has(parent.id)
          const picked = parent.subcategories.filter((s) => selected.has(s.id)).length
          return (
            <div key={parent.id} className={classes.group}>
              <div className={classes.parent}>
                <Checkbox
                  className={classes.check}
                  label={parent.name}
                  description={
                    whole
                      ? 'All of it'
                      : picked > 0
                        ? `${String(picked)} of ${String(parent.subcategories.length)}`
                        : undefined
                  }
                  checked={whole}
                  indeterminate={!whole && picked > 0}
                  onChange={(e) => {
                    const on = e.currentTarget.checked
                    update((ids) => {
                      if (!on) {
                        ids.delete(parent.id)
                        return
                      }
                      // The whole category makes picked subcategories redundant.
                      ids.add(parent.id)
                      for (const sub of parent.subcategories) ids.delete(sub.id)
                    })
                  }}
                />
                {needle === '' && parent.subcategories.length > 0 && (
                  <UnstyledButton
                    className={classes.expand}
                    onClick={() => {
                      toggleExpanded(parent.id)
                    }}
                    aria-expanded={open}
                    aria-label={`${open ? 'Hide' : 'Show'} the subcategories of ${parent.name}`}
                  >
                    <IconChevronDown size={18} stroke={1.8} data-open={open} />
                  </UnstyledButton>
                )}
              </div>
              {open &&
                subs.map((sub) => (
                  <Checkbox
                    key={sub.id}
                    className={classes.sub}
                    label={shortName(parent, sub)}
                    // A whole category includes every subcategory.
                    checked={whole || selected.has(sub.id)}
                    disabled={whole}
                    onChange={(e) => {
                      const on = e.currentTarget.checked
                      update((ids) => {
                        if (on) ids.add(sub.id)
                        else ids.delete(sub.id)
                      })
                    }}
                  />
                ))}
            </div>
          )
        })}
      </div>
      <div className={classes.summary}>
        <Text size="xs" c="dimmed">
          {value.length === 0 ? 'None picked: every category is searched.' : null}
          {value.length > 0 && `${String(value.length)} picked`}
        </Text>
        {value.length > 0 && (
          <UnstyledButton
            className={classes.clear}
            onClick={() => {
              onChange([])
            }}
          >
            Clear
          </UnstyledButton>
        )}
      </div>
    </div>
  )
}
