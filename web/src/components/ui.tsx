import { Drawer, Modal } from '@mantine/core'
import { useMediaQuery } from '@mantine/hooks'
import type { CSSProperties, ReactNode } from 'react'

import type { TorrentStatus } from '../api/generated/model'
import { STATUS } from '../lib/status'
import classes from './ui.module.css'

/** A bar floating above the content with actions on the current selection. */
export function ActionBar({ label, children }: { label: ReactNode; children: ReactNode }) {
  return (
    <div className={classes.actionBar} role="toolbar" aria-label="Selection">
      <div className={classes.actionBarCount}>{label}</div>
      {children}
    </div>
  )
}

export function EmptyState({
  icon,
  title,
  children,
}: {
  icon: ReactNode
  title: ReactNode
  children?: ReactNode
}) {
  return (
    <div className={classes.empty}>
      <div className={classes.emptyIcon}>{icon}</div>
      <div className={classes.emptyTitle}>{title}</div>
      {children}
    </div>
  )
}

/** A bottom sheet on phones, a dialog elsewhere. */
export function Sheet({
  opened,
  onClose,
  title,
  children,
}: {
  opened: boolean
  onClose: () => void
  title: ReactNode
  children: ReactNode
}) {
  const phone = useMediaQuery('(max-width: 48em)', true)
  if (phone) {
    return (
      <Drawer
        opened={opened}
        onClose={onClose}
        title={title}
        position="bottom"
        size="auto"
        radius="lg"
        overlayProps={{ backgroundOpacity: 0.55, blur: 3 }}
        styles={{
          content: { maxHeight: '88dvh', paddingBottom: 'var(--safe-bottom)' },
          title: { fontWeight: 650, fontSize: 17 },
        }}
      >
        {children}
      </Drawer>
    )
  }
  return (
    <Modal
      opened={opened}
      onClose={onClose}
      title={title}
      centered
      radius="lg"
      overlayProps={{ backgroundOpacity: 0.55, blur: 3 }}
      styles={{ title: { fontWeight: 650, fontSize: 17 } }}
    >
      {children}
    </Modal>
  )
}

export function StatusLabel({ status }: { status: TorrentStatus }) {
  const s = STATUS[status]
  return (
    <span
      className={classes.status}
      data-live={s.live === true}
      style={{ '--status-color': s.color } as CSSProperties}
    >
      {s.label}
    </span>
  )
}
