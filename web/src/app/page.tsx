'use client'
import Image from 'next/image'
import Link from 'next/link'
import { useState } from 'react'

const GITHUB_URL = 'https://github.com/ashwwwin/typeswift'

export default function Home() {
  const [hoveredCard, setHoveredCard] = useState<string | null>(null)

  return (
    <main className="min-h-screen bg-black text-white antialiased">
      {/* Navigation - Clean and minimal */}
      <nav className="fixed top-0 z-50 w-full bg-black/50 backdrop-blur-md border-b border-white/5">
        <div className="mx-auto max-w-7xl px-6">
          <div className="flex h-16 items-center justify-between">
            <Link href="/" className="flex items-center gap-3">
              <Image 
                src="/logo.png" 
                alt="Typeswift" 
                width={32} 
                height={32} 
                className="rounded-lg" 
              />
              <span className="font-semibold text-white">Typeswift</span>
            </Link>

            <div className="flex items-center gap-8">
              <Link href="#features" className="text-sm text-white/60 hover:text-white transition-colors">
                Features
              </Link>
              <Link href="#how" className="text-sm text-white/60 hover:text-white transition-colors">
                How it works
              </Link>
              <Link href={GITHUB_URL} className="text-sm text-white/60 hover:text-white transition-colors">
                GitHub
              </Link>
              <a
                href="#pricing"
                className="text-sm rounded-full bg-white text-black px-5 py-2 font-medium hover:bg-white/90 transition-colors"
              >
                Get it for $19
              </a>
            </div>
          </div>
        </div>
      </nav>

      {/* Hero - Clean with purpose */}
      <section className="relative pt-32 pb-20 px-6">
        {/* Subtle static gradient */}
        <div className="absolute inset-0 -z-10">
          <div className="absolute top-40 left-1/3 w-[600px] h-[600px] bg-purple-600/10 rounded-full blur-3xl" />
          <div className="absolute top-60 right-1/3 w-[600px] h-[600px] bg-blue-600/10 rounded-full blur-3xl" />
        </div>

        <div className="mx-auto max-w-6xl">
          <div className="text-center">
            {/* Logo with refined glow */}
            <div className="inline-flex relative mb-8">
              <div className="absolute inset-0 rounded-2xl bg-gradient-to-r from-purple-600/20 to-blue-600/20 blur-2xl" />
              <Image 
                src="/logo.png" 
                alt="Typeswift" 
                width={80} 
                height={80} 
                className="relative rounded-2xl" 
              />
            </div>
            
            {/* Clear value proposition */}
            <h1 className="text-6xl sm:text-7xl font-bold tracking-tight mb-6">
              <span className="bg-gradient-to-r from-white to-white/80 bg-clip-text text-transparent">
                Type at the speed of speech
              </span>
            </h1>
            
            <p className="text-xl text-white/60 max-w-2xl mx-auto mb-10 leading-relaxed">
              Hold a key. Speak naturally. Watch your words appear instantly. 
              The missing dictation tool for macOS that just works.
            </p>
            
            {/* CTAs with purpose */}
            <div className="flex items-center justify-center gap-4 flex-wrap">
              <a
                href="#pricing"
                className="group relative px-8 py-4 rounded-xl bg-white text-black font-semibold hover:bg-white/90 transition-all"
              >
                Get Typeswift for $19
              </a>
              
              <Link
                href={GITHUB_URL}
                className="px-8 py-4 rounded-xl border border-white/10 font-semibold hover:bg-white/5 transition-all"
              >
                View Source on GitHub
              </Link>
            </div>
          </div>
        </div>
      </section>

      {/* Bento Grid - Thoughtfully designed */}
      <section id="features" className="px-6 py-20">
        <div className="mx-auto max-w-6xl">
          <div className="text-center mb-16">
            <h2 className="text-4xl font-bold mb-4">Built different. Works better.</h2>
            <p className="text-lg text-white/60">Everything about Typeswift is designed for real-world use</p>
          </div>

          <div className="grid grid-cols-1 md:grid-cols-4 gap-4 auto-rows-[200px]">
            {/* Main feature - The magic moment */}
            <div 
              className="group relative md:col-span-2 md:row-span-2 rounded-3xl bg-gradient-to-br from-purple-600/10 to-transparent border border-white/10 p-8 overflow-hidden"
              onMouseEnter={() => setHoveredCard('main')}
              onMouseLeave={() => setHoveredCard(null)}
            >
              <div className="relative z-10 h-full flex flex-col">
                <div className="text-5xl mb-4">🎙️</div>
                <h3 className="text-2xl font-bold mb-3">Push-to-talk for typing</h3>
                <p className="text-white/60 text-lg leading-relaxed flex-1">
                  No apps to open. No buttons to click. Just hold Fn and speak. 
                  Your words appear exactly where your cursor is—in any app, any text field, anywhere on macOS.
                </p>
                <div className="mt-6 flex items-center gap-4 text-sm">
                  <div className="flex items-center gap-2">
                    <div className="w-2 h-2 rounded-full bg-green-400"></div>
                    <span className="text-white/40">Always ready</span>
                  </div>
                  <div className="flex items-center gap-2">
                    <div className="w-2 h-2 rounded-full bg-green-400"></div>
                    <span className="text-white/40">System-wide</span>
                  </div>
                </div>
              </div>
              {/* Subtle animation on hover */}
              <div className={`absolute inset-0 bg-gradient-to-br from-purple-600/5 to-blue-600/5 transition-opacity duration-500 ${hoveredCard === 'main' ? 'opacity-100' : 'opacity-0'}`} />
            </div>

            {/* Lightning fast */}
            <div 
              className="group relative md:col-span-2 md:row-span-1 rounded-3xl bg-gradient-to-br from-blue-600/10 to-transparent border border-white/10 p-6 overflow-hidden"
              onMouseEnter={() => setHoveredCard('fast')}
              onMouseLeave={() => setHoveredCard(null)}
            >
              <div className="relative z-10">
                <div className="flex items-start justify-between mb-3">
                  <div>
                    <div className="text-3xl mb-3">⚡</div>
                    <h3 className="text-xl font-bold mb-2">Instant transcription</h3>
                    <p className="text-white/60">
                      Parakeet v3 model via FluidAudio. Zero network latency. ~100MB memory.
                    </p>
                  </div>
                  <div className="text-right">
                    <div className="text-3xl font-bold text-blue-400">0ms</div>
                    <div className="text-xs text-white/40 mt-1">LATENCY</div>
                  </div>
                </div>
              </div>
              <div className={`absolute inset-0 bg-gradient-to-r from-blue-600/5 to-transparent transition-opacity duration-500 ${hoveredCard === 'fast' ? 'opacity-100' : 'opacity-0'}`} />
            </div>

            {/* 100% Private */}
            <div 
              className="group relative rounded-3xl bg-zinc-900 border border-white/10 p-6 overflow-hidden"
              onMouseEnter={() => setHoveredCard('private')}
              onMouseLeave={() => setHoveredCard(null)}
            >
              <div className="relative z-10">
                <div className="text-3xl mb-3">🔒</div>
                <h3 className="text-lg font-bold mb-2">Your privacy</h3>
                <p className="text-white/60 text-sm">
                  100% offline. No cloud. No accounts. No tracking.
                </p>
              </div>
              <div className={`absolute inset-0 bg-white/5 transition-opacity duration-500 ${hoveredCard === 'private' ? 'opacity-100' : 'opacity-0'}`} />
            </div>

            {/* Works everywhere */}
            <div 
              className="group relative rounded-3xl bg-zinc-900 border border-white/10 p-6 overflow-hidden"
              onMouseEnter={() => setHoveredCard('everywhere')}
              onMouseLeave={() => setHoveredCard(null)}
            >
              <div className="relative z-10">
                <div className="text-3xl mb-3">🌐</div>
                <h3 className="text-lg font-bold mb-2">Universal</h3>
                <p className="text-white/60 text-sm">
                  Works in every app. Slack, email, code editors, browsers.
                </p>
              </div>
              <div className={`absolute inset-0 bg-white/5 transition-opacity duration-500 ${hoveredCard === 'everywhere' ? 'opacity-100' : 'opacity-0'}`} />
            </div>

            {/* Open Source */}
            <div 
              className="group relative md:col-span-2 rounded-3xl bg-gradient-to-br from-green-600/10 to-transparent border border-white/10 p-6 overflow-hidden"
              onMouseEnter={() => setHoveredCard('open')}
              onMouseLeave={() => setHoveredCard(null)}
            >
              <div className="relative z-10 flex items-center gap-4">
                <div className="text-3xl">🛠️</div>
                <div className="flex-1">
                  <h3 className="text-lg font-bold mb-1">Open source, MIT licensed</h3>
                  <p className="text-white/60 text-sm">
                    Built with Rust, GPUI & FluidAudio. Inspect the code, build from source, customize everything.
                  </p>
                </div>
              </div>
              <div className={`absolute inset-0 bg-gradient-to-r from-green-600/5 to-transparent transition-opacity duration-500 ${hoveredCard === 'open' ? 'opacity-100' : 'opacity-0'}`} />
            </div>

            {/* Menu bar app */}
            <div 
              className="group relative rounded-3xl bg-zinc-900 border border-white/10 p-6 overflow-hidden"
              onMouseEnter={() => setHoveredCard('menubar')}
              onMouseLeave={() => setHoveredCard(null)}
            >
              <div className="relative z-10">
                <div className="text-3xl mb-3">🖥️</div>
                <h3 className="text-lg font-bold mb-2">Native macOS</h3>
                <p className="text-white/60 text-sm">
                  Lightweight menu bar app. No electron. Pure Swift.
                </p>
              </div>
              <div className={`absolute inset-0 bg-white/5 transition-opacity duration-500 ${hoveredCard === 'menubar' ? 'opacity-100' : 'opacity-0'}`} />
            </div>

            {/* Customizable */}
            <div 
              className="group relative rounded-3xl bg-zinc-900 border border-white/10 p-6 overflow-hidden"
              onMouseEnter={() => setHoveredCard('custom')}
              onMouseLeave={() => setHoveredCard(null)}
            >
              <div className="relative z-10">
                <div className="text-3xl mb-3">⌨️</div>
                <h3 className="text-lg font-bold mb-2">Your hotkey</h3>
                <p className="text-white/60 text-sm">
                  Use Fn or set any key combination you prefer.
                </p>
              </div>
              <div className={`absolute inset-0 bg-white/5 transition-opacity duration-500 ${hoveredCard === 'custom' ? 'opacity-100' : 'opacity-0'}`} />
            </div>
          </div>
        </div>
      </section>

      {/* How it works - With style */}
      <section id="how" className="px-6 py-20 bg-gradient-to-b from-transparent via-purple-600/5 to-transparent">
        <div className="mx-auto max-w-5xl">
          <div className="text-center mb-16">
            <h2 className="text-4xl font-bold mb-4">Simple as 1-2-3</h2>
            <p className="text-lg text-white/60">No learning curve. Just natural.</p>
          </div>
          
          <div className="grid md:grid-cols-3 gap-8">
            <div className="group relative">
              <div className="absolute inset-0 rounded-2xl bg-gradient-to-br from-purple-600/20 to-transparent blur-xl opacity-50" />
              <div className="relative rounded-2xl border border-purple-600/20 bg-black/50 backdrop-blur p-8 text-center">
                <div className="w-16 h-16 rounded-full bg-gradient-to-br from-purple-600 to-purple-700 flex items-center justify-center text-2xl font-bold mx-auto mb-4">
                  1
                </div>
                <h3 className="text-xl font-bold mb-2">Hold your key</h3>
                <p className="text-white/60">Press and hold Fn (or your custom hotkey)</p>
              </div>
            </div>

            <div className="group relative">
              <div className="absolute inset-0 rounded-2xl bg-gradient-to-br from-blue-600/20 to-transparent blur-xl opacity-50" />
              <div className="relative rounded-2xl border border-blue-600/20 bg-black/50 backdrop-blur p-8 text-center">
                <div className="w-16 h-16 rounded-full bg-gradient-to-br from-blue-600 to-blue-700 flex items-center justify-center text-2xl font-bold mx-auto mb-4">
                  2
                </div>
                <h3 className="text-xl font-bold mb-2">Speak naturally</h3>
                <p className="text-white/60">Talk at your normal pace, in any language</p>
              </div>
            </div>

            <div className="group relative">
              <div className="absolute inset-0 rounded-2xl bg-gradient-to-br from-green-600/20 to-transparent blur-xl opacity-50" />
              <div className="relative rounded-2xl border border-green-600/20 bg-black/50 backdrop-blur p-8 text-center">
                <div className="w-16 h-16 rounded-full bg-gradient-to-br from-green-600 to-green-700 flex items-center justify-center text-2xl font-bold mx-auto mb-4">
                  3
                </div>
                <h3 className="text-xl font-bold mb-2">Release to type</h3>
                <p className="text-white/60">Your words appear instantly where you're typing</p>
              </div>
            </div>
          </div>
        </div>
      </section>

      {/* Time savings - The real value */}
      <section className="px-6 py-20 bg-gradient-to-b from-transparent via-blue-600/5 to-transparent">
        <div className="mx-auto max-w-6xl">
          <div className="text-center mb-12">
            <h2 className="text-4xl font-bold mb-4">Speaking is 3x faster than typing</h2>
            <p className="text-lg text-white/60">The average person types 40 words per minute but speaks 150+</p>
          </div>

          <div className="grid md:grid-cols-3 gap-6 mb-12">
            <div className="relative rounded-2xl border border-white/10 bg-black/50 p-8 text-center">
              <div className="text-5xl font-bold bg-gradient-to-r from-purple-400 to-blue-400 bg-clip-text text-transparent mb-2">
                3-4x
              </div>
              <div className="text-lg font-semibold mb-2">Faster than typing</div>
              <p className="text-sm text-white/50">Most people speak at 150-200 WPM vs 40 WPM typing</p>
            </div>
            
            <div className="relative rounded-2xl border border-white/10 bg-black/50 p-8 text-center">
              <div className="text-5xl font-bold bg-gradient-to-r from-blue-400 to-green-400 bg-clip-text text-transparent mb-2">
                2 hrs
              </div>
              <div className="text-lg font-semibold mb-2">Saved per day</div>
              <p className="text-sm text-white/50">For heavy computer users who type frequently</p>
            </div>
            
            <div className="relative rounded-2xl border border-white/10 bg-black/50 p-8 text-center">
              <div className="text-5xl font-bold bg-gradient-to-r from-green-400 to-purple-400 bg-clip-text text-transparent mb-2">
                500+
              </div>
              <div className="text-lg font-semibold mb-2">Hours per year</div>
              <p className="text-sm text-white/50">That's 3 weeks of time back in your life</p>
            </div>
          </div>

        </div>
      </section>

      {/* Tech specs - Minimal but present */}
      <section className="px-6 py-20 border-t border-white/5">
        <div className="mx-auto max-w-6xl">
          <div className="grid md:grid-cols-2 gap-12">
            <div>
              <h3 className="text-2xl font-bold mb-6">Lightning fast, tiny footprint</h3>
              <div className="space-y-4">
                <div className="flex items-center justify-between py-3 border-b border-white/5">
                  <span className="text-white/60">Memory usage</span>
                  <span className="font-mono font-semibold">~100MB</span>
                </div>
                <div className="flex items-center justify-between py-3 border-b border-white/5">
                  <span className="text-white/60">Response time</span>
                  <span className="font-mono font-semibold">&lt;50ms</span>
                </div>
                <div className="flex items-center justify-between py-3 border-b border-white/5">
                  <span className="text-white/60">CPU usage</span>
                  <span className="font-mono font-semibold">&lt;5%</span>
                </div>
              </div>
            </div>

            <div>
              <h3 className="text-2xl font-bold mb-6">Built with the best tools</h3>
              <div className="space-y-4">
                <div className="flex items-center justify-between py-3 border-b border-white/5">
                  <span className="text-white/60">Core</span>
                  <span className="font-mono font-semibold">Rust + GPUI</span>
                </div>
                <div className="flex items-center justify-between py-3 border-b border-white/5">
                  <span className="text-white/60">Speech Engine</span>
                  <span className="font-mono font-semibold">FluidAudio</span>
                </div>
                <div className="flex items-center justify-between py-3 border-b border-white/5">
                  <span className="text-white/60">Model</span>
                  <span className="font-mono font-semibold">Parakeet v3</span>
                </div>
              </div>
            </div>
          </div>
        </div>
      </section>

      {/* Pricing - Clear and simple */}
      <section id="pricing" className="px-6 py-20">
        <div className="mx-auto max-w-5xl">
          <div className="text-center mb-12">
            <h2 className="text-4xl font-bold mb-4">Ready to type faster?</h2>
            <p className="text-lg text-white/60">Choose your path</p>
          </div>
          
          <div className="grid md:grid-cols-2 gap-8">
            {/* Open source option */}
            <div className="rounded-2xl border border-white/10 bg-black p-8">
              <div className="mb-6">
                <h3 className="text-2xl font-bold mb-2">Open Source</h3>
                <div className="text-4xl font-bold">Free</div>
              </div>
              
              <ul className="space-y-3 mb-8">
                <li className="flex items-start gap-3">
                  <svg className="w-5 h-5 text-white/40 mt-0.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 13l4 4L19 7" />
                  </svg>
                  <span className="text-white/80">Full source code on GitHub</span>
                </li>
                <li className="flex items-start gap-3">
                  <svg className="w-5 h-5 text-white/40 mt-0.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 13l4 4L19 7" />
                  </svg>
                  <span className="text-white/80">Build with Cargo + Swift</span>
                </li>
                <li className="flex items-start gap-3">
                  <svg className="w-5 h-5 text-white/40 mt-0.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 13l4 4L19 7" />
                  </svg>
                  <span className="text-white/80">MIT license</span>
                </li>
                <li className="flex items-start gap-3">
                  <svg className="w-5 h-5 text-white/40 mt-0.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 13l4 4L19 7" />
                  </svg>
                  <span className="text-white/80">Customize everything</span>
                </li>
              </ul>
              
              <Link
                href={GITHUB_URL}
                className="block w-full rounded-xl border border-white/20 py-3 text-center font-semibold hover:bg-white/5 transition-colors"
              >
                View on GitHub
              </Link>
            </div>

            {/* Pre-built option */}
            <div className="relative rounded-2xl border-2 border-purple-600/30 bg-gradient-to-br from-purple-600/5 to-transparent p-8">
              <div className="absolute -top-3 left-8 bg-gradient-to-r from-purple-600 to-blue-600 text-xs font-semibold px-3 py-1 rounded-full">
                READY TO USE
              </div>
              
              <div className="mb-6">
                <h3 className="text-2xl font-bold mb-2">Pre-built App</h3>
                <div className="text-4xl font-bold">$19</div>
              </div>
              
              <ul className="space-y-3 mb-8">
                <li className="flex items-start gap-3">
                  <svg className="w-5 h-5 text-purple-400 mt-0.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 13l4 4L19 7" />
                  </svg>
                  <span className="text-white/80">Download and use immediately</span>
                </li>
                <li className="flex items-start gap-3">
                  <svg className="w-5 h-5 text-purple-400 mt-0.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 13l4 4L19 7" />
                  </svg>
                  <span className="text-white/80">Code signed & notarized</span>
                </li>
                <li className="flex items-start gap-3">
                  <svg className="w-5 h-5 text-purple-400 mt-0.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 13l4 4L19 7" />
                  </svg>
                  <span className="text-white/80">Automatic updates</span>
                </li>
                <li className="flex items-start gap-3">
                  <svg className="w-5 h-5 text-purple-400 mt-0.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 13l4 4L19 7" />
                  </svg>
                  <span className="text-white/80">Support development</span>
                </li>
              </ul>
              
              <a
                href="#"
                className="block w-full rounded-xl bg-gradient-to-r from-purple-600 to-blue-600 py-3 text-center font-semibold text-white hover:opacity-90 transition-opacity"
              >
                Get Typeswift for $19
              </a>
            </div>
          </div>

          <p className="text-center text-sm text-white/40 mt-8">
            30-day money back guarantee · Lifetime updates · Support indie development
          </p>
        </div>
      </section>

      {/* Footer */}
      <footer className="border-t border-white/5 px-6 py-12">
        <div className="mx-auto max-w-6xl">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              <Image 
                src="/logo.png" 
                alt="Typeswift" 
                width={24} 
                height={24} 
                className="rounded-md opacity-60" 
              />
              <span className="text-sm text-white/40">© 2024 Typeswift</span>
            </div>
            
            <div className="flex items-center gap-6">
              <Link href={GITHUB_URL} className="text-sm text-white/40 hover:text-white/60 transition-colors">
                GitHub
              </Link>
              <Link href="#" className="text-sm text-white/40 hover:text-white/60 transition-colors">
                Privacy
              </Link>
              <span className="text-sm text-white/40">MIT License</span>
            </div>
          </div>
        </div>
      </footer>
    </main>
  )
}