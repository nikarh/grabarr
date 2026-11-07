import { IconDownload, IconSearch, IconSettings } from '@tabler/icons-react'
import type { ReactNode } from 'react'

import { useConnectionState } from '../api/hooks'
import { href, type Screen } from '../lib/router'
import { cx } from '../lib/cx'
import { LogoMark } from './Logo'
import classes from './Shell.module.css'

const TABS: { screen: Screen; label: string; icon: typeof IconSearch }[] = [
  { screen: 'search', label: 'Search', icon: IconSearch },
  { screen: 'downloads', label: 'Downloads', icon: IconDownload },
  { screen: 'settings', label: 'Settings', icon: IconSettings },
]

const CONNECTION_LABEL = {
  open: 'Live',
  connecting: 'Connecting',
  closed: 'Offline',
} as const

export function Shell({
  screen,
  activeDownloads,
  children,
}: {
  screen: Screen
  activeDownloads: number
  children: ReactNode
}) {
  const connection = useConnectionState()
  return (
    <>
      <header className={classes.header}>
        <div className={classes.headerInner}>
          <a className={classes.brand} href={href('search')}>
            <span className={classes.brandMark}>
              <LogoMark />
            </span>
            grabarr
          </a>
          <nav className={classes.nav} aria-label="Main">
            {TABS.map(({ screen: s, label, icon: Icon }) => (
              <a
                key={s}
                className={classes.navLink}
                href={href(s)}
                aria-current={s === screen ? 'page' : undefined}
              >
                <Icon size={17} stroke={1.8} />
                {label}
              </a>
            ))}
          </nav>
          <div className={classes.spacer} />
          <div className={classes.connection} role="status">
            <span className={classes.dot} data-state={connection} />
            {CONNECTION_LABEL[connection]}
          </div>
        </div>
      </header>
      <main className={classes.main}>{children}</main>
      <nav className={classes.tabbar} aria-label="Main">
        {TABS.map(({ screen: s, label, icon: Icon }) => (
          <a
            key={s}
            className={classes.tab}
            href={href(s)}
            aria-current={s === screen ? 'page' : undefined}
          >
            <span className={classes.tabIcon}>
              <Icon size={22} stroke={1.7} />
              {s === 'downloads' && activeDownloads > 0 && (
                <span className={cx(classes.tabBadge, 'mono')}>{activeDownloads}</span>
              )}
            </span>
            {label}
          </a>
        ))}
      </nav>
    </>
  )
}
