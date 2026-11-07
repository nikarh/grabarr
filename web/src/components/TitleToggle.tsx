import { ActionIcon, Tooltip } from '@mantine/core'
import { IconChevronDown } from '@tabler/icons-react'

import classes from './ui.module.css'

/** Shows a name the row cuts off in full, and back. Rendered only when there is something to show. */
export function TitleToggle({
  expanded,
  onToggle,
  name,
}: {
  expanded: boolean
  onToggle: () => void
  name: string
}) {
  return (
    <Tooltip label={expanded ? 'Show less' : 'Show the full name'} openDelay={400}>
      <ActionIcon
        size="lg"
        onClick={onToggle}
        aria-expanded={expanded}
        aria-label={expanded ? `Show less of ${name}` : `Show the full name of ${name}`}
      >
        <IconChevronDown
          size={18}
          stroke={1.8}
          className={classes.toggleIcon}
          data-expanded={expanded}
        />
      </ActionIcon>
    </Tooltip>
  )
}
