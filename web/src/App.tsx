import { useTorrents } from './api/hooks'
import { Shell } from './components/Shell'
import { useRoute } from './lib/router'
import { parseSort } from './lib/sort'
import { isActive } from './lib/status'
import { DownloadsScreen } from './screens/Downloads'
import { SearchScreen } from './screens/Search'
import { SettingsScreen } from './screens/Settings'

export function App() {
  const route = useRoute()
  // One subscription for the whole app: the tab badge and the Downloads screen share it.
  const torrents = useTorrents()
  const active =
    torrents.status === 'loading' || torrents.torrents === null
      ? 0
      : [...torrents.torrents.values()].filter(isActive).length

  return (
    <Shell screen={route.screen} activeDownloads={active}>
      {route.screen === 'search' && (
        <SearchScreen
          query={route.params.get('q') ?? ''}
          sort={parseSort(route.params.get('sort'))}
        />
      )}
      {route.screen === 'downloads' && <DownloadsScreen state={torrents} />}
      {route.screen === 'settings' && <SettingsScreen />}
    </Shell>
  )
}
