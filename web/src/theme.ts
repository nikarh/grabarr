import { createTheme, type MantineColorsTuple } from '@mantine/core'

// The single accent: an acid lime "signal", used sparingly for what's actionable or alive.
const signal: MantineColorsTuple = [
  '#f7fde6',
  '#eefbcc',
  '#ddf79a',
  '#cbf264',
  '#c4f042',
  '#b2dd2c',
  '#97bd1c',
  '#759411',
  '#556c08',
  '#364503',
]

// Ink: cool near-blacks for the dark scheme (Mantine reads text from [0], surfaces from [6]..[8]).
const dark: MantineColorsTuple = [
  '#e9ebee',
  '#c3c8cf',
  '#8d949e',
  '#5f6670',
  '#3b4149',
  '#2a2f36',
  '#1c2026',
  '#14171c',
  '#0e1014',
  '#090a0d',
]

export const theme = createTheme({
  primaryColor: 'signal',
  // Shade 6 in light mode keeps text on filled buttons dark in both schemes.
  primaryShade: { light: 6, dark: 4 },
  colors: { signal, dark },
  autoContrast: true,
  luminanceThreshold: 0.3,
  fontFamily: "'Geist Variable', ui-sans-serif, system-ui, sans-serif",
  fontFamilyMonospace: "'Geist Mono Variable', ui-monospace, monospace",
  headings: {
    fontFamily: "'Geist Variable', ui-sans-serif, system-ui, sans-serif",
    fontWeight: '600',
  },
  defaultRadius: 'md',
  radius: { xs: '4px', sm: '6px', md: '10px', lg: '14px', xl: '20px' },
  cursorType: 'pointer',
  focusRing: 'auto',
  components: {
    Button: { defaultProps: { fw: 600, autoContrast: true } },
    ActionIcon: { defaultProps: { variant: 'subtle', color: 'gray' } },
    Checkbox: { defaultProps: { radius: 'sm' } },
    Progress: { defaultProps: { size: 4, radius: 'xl' } },
    Badge: { defaultProps: { radius: 'sm', variant: 'light' } },
    Notification: { defaultProps: { radius: 'lg' }, classNames: { root: 'app-notification' } },
  },
})
