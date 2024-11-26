import { ReactNode } from 'react';

interface LayoutProps {
  children: ReactNode;
}

export default function Layout({ children }: LayoutProps) {
  return (
    <div className="min-h-screen bg-gradient-to-br from-black via-gray-900 to-black text-white">
      <header className="border-b border-primary/20 backdrop-blur-sm">
        <div className="container mx-auto p-4">
          <nav className="flex items-center justify-between">
            <div className="flex items-center gap-3">
              <img 
                src="/Polkadot_Token_Pink.png" 
                alt="Polkadot Logo" 
                className="w-8 h-8 animate-pulse filter brightness-125"
                style={{
                  animation: 'pulse 2s cubic-bezier(0.4, 0, 0.6, 1) infinite'
                }}
              />
              <span className="text-xl font-bold text-primary">DOT Inflation</span>
            </div>
          </nav>
        </div>
      </header>
      <main className="min-h-[calc(100vh-4rem)]">
        {children}
      </main>
    </div>
  );
}
