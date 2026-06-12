import type { ReactNode } from "react";

interface LayoutProps {
  readonly main: ReactNode;
}

const Layout = ({ main }: LayoutProps) => {
  return (
    <div className="flex h-screen flex-col overflow-hidden bg-background text-foreground">
      <div className="min-h-0 flex-1">{main}</div>
    </div>
  );
};

export { Layout };
