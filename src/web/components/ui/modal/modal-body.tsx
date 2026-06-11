import { forwardRef, type HTMLAttributes } from "react";
import { cn } from "@/lib/cn";

type ModalBodyProps = HTMLAttributes<HTMLDivElement>;

const ModalBody = forwardRef<HTMLDivElement, ModalBodyProps>(({ className, ...props }, ref) => {
  return <div ref={ref} data-slot="modal-body" className={cn("px-4 py-4", className)} {...props} />;
});
ModalBody.displayName = "ModalBody";

export type { ModalBodyProps };
export { ModalBody };
