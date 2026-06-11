import { ToggleSwitch, Typography } from "@/components/ui";

const ToggleRow = ({
  label,
  description,
  checked,
  onChange,
}: {
  readonly label: string;
  readonly description: string;
  readonly checked: boolean;
  readonly onChange: () => void;
}) => {
  return (
    <div className="flex items-center justify-between gap-4 p-4">
      <div className="flex flex-col gap-1">
        <Typography variant="body">{label}</Typography>
        <Typography variant="caption" color="subtle">
          {description}
        </Typography>
      </div>
      <ToggleSwitch on={checked} onToggle={onChange} color="success" />
    </div>
  );
};

export { ToggleRow };
