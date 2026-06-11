import { useRef } from "react";

import { Divider } from "@/components/ui";
import { useClickOutside } from "@/hooks/use-click-outside";

import { ContextMenuItem } from "./context-menu-item";

interface ProjectContextMenuProps {
  readonly x: number;
  readonly y: number;
  readonly onClose: () => void;
  readonly onSwitchBranch: () => void;
  readonly onPullLatest: () => void;
  readonly onOpenInCursor: () => void;
  readonly onRestart: () => void;
  readonly onRemove: () => void;
}

const ProjectContextMenu = ({
  x,
  y,
  onClose,
  onSwitchBranch,
  onPullLatest,
  onOpenInCursor,
  onRestart,
  onRemove,
}: ProjectContextMenuProps) => {
  const menuRef = useRef<HTMLDivElement>(null);
  useClickOutside(menuRef, onClose);

  const withClose = (handler: () => void) => {
    return () => {
      handler();
      onClose();
    };
  };

  return (
    <div
      ref={menuRef}
      className="fixed z-50 rounded border border-border bg-surface py-1 shadow-lg"
      style={{ left: x, top: y }}
    >
      <ContextMenuItem onClick={withClose(onSwitchBranch)}>Switch branch</ContextMenuItem>
      <ContextMenuItem onClick={withClose(onPullLatest)}>Pull latest</ContextMenuItem>
      <ContextMenuItem onClick={withClose(onOpenInCursor)}>Open in Cursor</ContextMenuItem>
      <ContextMenuItem onClick={withClose(onRestart)}>Restart</ContextMenuItem>
      <Divider className="my-1" />
      <ContextMenuItem onClick={withClose(onRemove)} danger>
        Remove project
      </ContextMenuItem>
    </div>
  );
};

export { ProjectContextMenu };
