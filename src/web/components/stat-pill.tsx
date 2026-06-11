const StatPill = ({
  label,
  value,
  valueClass,
}: {
  readonly label: string;
  readonly value: string;
  readonly valueClass?: string;
}) => (
  <span className="rounded-lg bg-pill-bg px-2 py-1">
    <span className="text-foreground-subtle">{label}</span>{" "}
    <span className={valueClass ?? "text-foreground"}>{value}</span>
  </span>
);

export { StatPill };
