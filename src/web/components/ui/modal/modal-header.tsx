import { forwardRef, type HTMLAttributes } from "react";
import { cn } from "@/lib/cn";
import { useModalContext } from "./modal-context";

type ModalHeaderProps = HTMLAttributes<HTMLDivElement>;

const ModalHeader = forwardRef<HTMLDivElement, ModalHeaderProps>(({ className, ...props }, ref) => {
  const context = useModalContext();

  return (
    <div
      ref={ref}
      id={context?.headerId}
      data-slot="modal-header"
      className={cn("border-border border-b px-4 py-3", className)}
      {...props}
    />
  );
});
ModalHeader.displayName = "ModalHeader";

export type { ModalHeaderProps };
export { ModalHeader };
