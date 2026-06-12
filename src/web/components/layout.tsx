import type { ReactNode } from "react";

interface LayoutProps {
  readonly main: ReactNode;
}

const Layout = ({ main }: LayoutProps) => {
  return (
    <div className="flex h-screen bg-background text-foreground">
      <div className="flex min-w-0 flex-1 flex-col p-3">
        <main className="relative flex-1 overflow-hidden rounded-xl bg-gradient-surface">
          <div className="h-full overflow-y-auto">{main}</div>
        </main>
      </div>
    </div>
  );
};

export { Layout };
