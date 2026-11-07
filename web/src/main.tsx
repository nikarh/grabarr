import '@fontsource-variable/geist'
import '@fontsource-variable/geist-mono'
import '@mantine/core/styles.css'
import '@mantine/notifications/styles.css'
import './styles.css'

import { MantineProvider } from '@mantine/core'
import { Notifications } from '@mantine/notifications'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'

import { App } from './App'
import { theme } from './theme'

const queryClient = new QueryClient({
  defaultOptions: {
    queries: { staleTime: 30_000, refetchOnWindowFocus: false, retry: 1 },
  },
})

const root = document.getElementById('root')
if (root === null) throw new Error('#root is missing')

createRoot(root).render(
  <StrictMode>
    <MantineProvider theme={theme} defaultColorScheme="auto">
      <QueryClientProvider client={queryClient}>
        <Notifications position="top-center" limit={3} />
        <App />
      </QueryClientProvider>
    </MantineProvider>
  </StrictMode>,
)
