import { Drawer, Modal } from '@mantine/core'
import { useMediaQuery } from '@mantine/hooks'
import type { CSSProperties, ReactNode } from 'react'

import type { TorrentStatus } from '../api/generated/model'
import { STATUS } from '../lib/status'
import classes from './ui.module.css'

/**
 * A bar floating above the content, with actions on a selection or on unsaved changes. While it shows, the page
 * gets room at the bottom so it never covers the last row.
 */
export function ActionBar({
  label,
  visible,
  children,
}: {
  label: ReactNode
  visible: boolean
  children: ReactNode
}) {
  return (
    <div
      className={classes.actionBar}
      role="toolbar"
      aria-label="Actions"
      aria-hidden={!visible}
      inert={!visible}
      data-floating-bar={visible ? 'visible' : 'hidden'}
    >
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
        radius="lg"
        overlayProps={{ backgroundOpacity: 0.55, blur: 3 }}
        // The gutter is reserved (see styles.css): hiding the scrollbar would shift the page.
        removeScrollProps={{ removeScrollBar: false }}
        styles={{
          content: {
            height: 'auto',
            maxHeight: '88dvh',
          },
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
      removeScrollProps={{ removeScrollBar: false }}
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
