import { forwardRef, type ReactNode, useCallback, useEffect, useId, useRef } from "react";
import { cn } from "@/lib/cn";
import { ModalContext } from "./modal-context";

const FOCUSABLE_SELECTOR =
  'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])';

interface ModalProps {
  readonly isOpen: boolean;
  readonly onClose: () => void;
  readonly children: ReactNode;
  readonly className?: string;
}

const getFocusableElements = (container: HTMLElement) => {
  return container.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR);
};

const trapFocus = (modal: HTMLElement, event: KeyboardEvent) => {
  const focusableElements = getFocusableElements(modal);
  if (focusableElements.length === 0) return;

  const firstElement = focusableElements[0];
  const lastElement = focusableElements[focusableElements.length - 1];
  const wrapTarget = event.shiftKey ? lastElement : firstElement;
  const edgeElement = event.shiftKey ? firstElement : lastElement;

  if (document.activeElement === edgeElement) {
    event.preventDefault();
    wrapTarget.focus();
  }
};

const useFocusManagement = (isOpen: boolean, modalRef: React.RefObject<HTMLDivElement | null>) => {
  const previouslyFocusedRef = useRef<HTMLElement | null>(null);

  useEffect(() => {
    if (isOpen) {
      const active = document.activeElement;
      previouslyFocusedRef.current = active instanceof HTMLElement ? active : null;
      requestAnimationFrame(() => {
        const modal = modalRef.current;
        if (!modal) return;
        const focusable = getFocusableElements(modal);
        (focusable.length > 0 ? focusable[0] : modal).focus();
      });
    } else if (previouslyFocusedRef.current) {
      previouslyFocusedRef.current.focus();
      previouslyFocusedRef.current = null;
    }
  }, [isOpen, modalRef]);
};

const useModalKeyboard = (
  isOpen: boolean,
  onClose: () => void,
  modalRef: React.RefObject<HTMLDivElement | null>,
) => {
  const handleKeyDown = useCallback(
    (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        onClose();
        return;
      }
      if (event.key === "Tab" && modalRef.current) {
        trapFocus(modalRef.current, event);
      }
    },
    [onClose, modalRef],
  );

  useEffect(() => {
    if (!isOpen) return;
    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, [isOpen, handleKeyDown]);
};

const useBodyScrollLock = (isOpen: boolean) => {
  useEffect(() => {
    document.body.style.overflow = isOpen ? "hidden" : "";
    return () => {
      document.body.style.overflow = "";
    };
  }, [isOpen]);
};

const assignRefs = (
  node: HTMLDivElement | null,
  modalRef: React.MutableRefObject<HTMLDivElement | null>,
  forwardedRef: React.ForwardedRef<HTMLDivElement>,
) => {
  modalRef.current = node;
  if (typeof forwardedRef === "function") {
    forwardedRef(node);
  } else if (forwardedRef) {
    forwardedRef.current = node;
  }
};

const Modal = forwardRef<HTMLDivElement, ModalProps>(
  ({ isOpen, onClose, children, className }, ref) => {
    const modalId = useId();
    const headerId = `${modalId}-header`;
    const modalRef = useRef<HTMLDivElement>(null);

    useFocusManagement(isOpen, modalRef);
    useModalKeyboard(isOpen, onClose, modalRef);
    useBodyScrollLock(isOpen);

    if (!isOpen) return null;

    return (
      // biome-ignore lint/a11y/useKeyWithClickEvents: keyboard handling done via document keydown listener
      // biome-ignore lint/a11y/noStaticElementInteractions: overlay is a backdrop click target
      <div
        data-slot="modal-overlay"
        className="modal-overlay fixed inset-0 z-50 flex items-center justify-center"
        onClick={onClose}
      >
        <div
          ref={(node) => assignRefs(node, modalRef, ref)}
          data-slot="modal"
          role="dialog"
          aria-modal="true"
          aria-labelledby={headerId}
          tabIndex={-1}
          className={cn(
            "mx-4 max-h-modal w-full max-w-lg overflow-auto rounded-lg border border-border bg-surface shadow-lg",
            className,
          )}
          onClick={(e) => e.stopPropagation()}
          onKeyDown={(e) => e.stopPropagation()}
        >
          <ModalContext.Provider value={{ headerId }}>{children}</ModalContext.Provider>
        </div>
      </div>
    );
  },
);
Modal.displayName = "Modal";

export type { ModalProps };
export { Modal };
