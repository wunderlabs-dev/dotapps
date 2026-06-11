import type { ReactNode } from "react";

import { Button } from "@/components/ui";

interface DetailActionButtonProps {
  readonly icon: ReactNode;
  readonly onClick: () => void;
  readonly label: string;
}

const DetailActionButton = ({ icon, onClick, label }: DetailActionButtonProps) => (
  <Button
    size="icon"
    variant="outline"
    onClick={onClick}
    aria-label={label}
    className="bg-surface-elevated text-foreground-subtle hover:text-foreground"
  >
    {icon}
  </Button>
);

export { DetailActionButton };
