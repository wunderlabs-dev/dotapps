import type { ReactNode } from "react";

interface LayoutProps {
  readonly sidebar: ReactNode;
  readonly main: ReactNode;
  readonly resourceBar: ReactNode;
}

const Layout = ({ sidebar, main, resourceBar }: LayoutProps) => {
  return (
    <div className="flex h-screen bg-background text-foreground">
      <aside className="flex w-64 shrink-0 flex-col">{sidebar}</aside>
      <div className="flex min-w-0 flex-1 flex-col p-3 pl-0">
        <main className="relative flex-1 overflow-hidden rounded-xl bg-gradient-surface">
          <div className="h-full overflow-y-auto pb-10">{main}</div>
          <div className="absolute right-3 bottom-3 z-20">{resourceBar}</div>
        </main>
      </div>
    </div>
  );
};

export { Layout };
