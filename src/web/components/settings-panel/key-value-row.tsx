import { Typography } from "@/components/ui";

interface KeyValueRowProps {
  readonly label: string;
  readonly value: string;
}

const KeyValueRow = ({ label, value }: KeyValueRowProps) => (
  <div className="flex items-baseline justify-between gap-4">
    <Typography variant="body" color="subtle">
      {label}
    </Typography>
    <Typography variant="body" className="font-mono">
      {value}
    </Typography>
  </div>
);

export type { KeyValueRowProps };
export { KeyValueRow };
