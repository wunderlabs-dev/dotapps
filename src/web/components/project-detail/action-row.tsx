const ActionRow = ({
  children,
  timestamp,
}: {
  readonly children: React.ReactNode;
  readonly timestamp?: string;
}) => (
  <div className="flex items-center gap-3">
    {children}
    {timestamp && <span className="text-foreground-subtle text-xs">{timestamp}</span>}
  </div>
);

export { ActionRow };
