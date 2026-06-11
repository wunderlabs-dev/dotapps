import { SvgIconCheck } from "@/components/icon/svg-icon-check";
import { SvgIconCopy } from "@/components/icon/svg-icon-copy";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { useCopyFeedback } from "@/hooks/use-copy-feedback";
import { cn } from "@/lib/cn";

interface CopyInputProps {
  readonly value: string;
  readonly className?: string;
}

const CopyInput = ({ value, className }: CopyInputProps) => {
  const { copied, handleCopy } = useCopyFeedback(value);

  return (
    <div data-slot="copy-input" className={cn("flex flex-col gap-3", className)}>
      <Input value={value} readOnly />
      <Button variant={copied ? "success" : "outline"} size="sm" onClick={handleCopy}>
        {copied ? <SvgIconCheck size="sm" /> : <SvgIconCopy size="sm" />}
        {copied ? "Copied" : "Copy"}
      </Button>
    </div>
  );
};

export type { CopyInputProps };
export { CopyInput };
