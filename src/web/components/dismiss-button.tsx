import { SvgIconClose } from "@/components/icon";
import { Button } from "@/components/ui";

const DismissButton = ({ onClick }: { readonly onClick: () => void }) => {
  return (
    <Button
      variant="ghost"
      size="icon-sm"
      onClick={onClick}
      aria-label="Dismiss error"
      className="ml-4 text-foreground-muted hover:text-foreground"
    >
      <SvgIconClose size="sm" />
    </Button>
  );
};

export { DismissButton };
