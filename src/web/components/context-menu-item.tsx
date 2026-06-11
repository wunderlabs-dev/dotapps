const ContextMenuItem = ({
  onClick,
  children,
  danger = false,
}: {
  readonly onClick: () => void;
  readonly children: React.ReactNode;
  readonly danger?: boolean;
}) => {
  return (
    <button
      type="button"
      onClick={onClick}
      className={`w-full px-3 py-2 text-left text-sm hover:bg-surface-hover ${
        danger ? "text-terminal-red hover:text-terminal-red" : "text-foreground"
      }`}
    >
      {children}
    </button>
  );
};

export { ContextMenuItem };
