import './globals.css'
import type { Metadata } from 'next'

export const metadata: Metadata = {
  title: 'Typeswift — Push‑to‑Talk Transcription for macOS',
  description:
    'Privacy‑first, on‑device speech‑to‑text. Hold a hotkey, speak, and Typeswift types anywhere on macOS.',
  icons: [{ rel: 'icon', url: '/favicon.ico' }],
  openGraph: {
    title: 'Typeswift — Push‑to‑Talk Transcription for macOS',
    description:
      'Privacy‑first, on‑device speech‑to‑text. Hold a hotkey, speak, and Typeswift types anywhere on macOS.',
    images: [{ url: '/logo.png' }],
    url: 'https://typeswift.app',
    siteName: 'Typeswift',
  },
  twitter: {
    card: 'summary',
    title: 'Typeswift — Push‑to‑Talk Transcription for macOS',
    description:
      'Privacy‑first, on‑device speech‑to‑text. Hold a hotkey, speak, and Typeswift types anywhere on macOS.',
    images: ['/logo.png'],
  },
}

export default function RootLayout({
  children,
}: {
  children: React.ReactNode
}) {
  return (
    <html lang="en" className="dark">
      <body className="antialiased bg-black text-white font-system">{children}</body>
    </html>
  )
}
