import { forwardRef, type HTMLAttributes } from "react";
import { cn } from "@/lib/cn";

type ModalFooterProps = HTMLAttributes<HTMLDivElement>;

const ModalFooter = forwardRef<HTMLDivElement, ModalFooterProps>(({ className, ...props }, ref) => {
  return (
    <div
      ref={ref}
      data-slot="modal-footer"
      className={cn("flex justify-end gap-2 border-border border-t px-4 py-3", className)}
      {...props}
    />
  );
});
ModalFooter.displayName = "ModalFooter";

export type { ModalFooterProps };
export { ModalFooter };
