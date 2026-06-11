import { Button, Typography } from "@/components/ui";

const WSLEnableContent = ({
  onEnable,
  enabling,
}: {
  readonly onEnable: () => void;
  readonly enabling: boolean;
}) => {
  return (
    <div className="space-y-4 text-center">
      <Typography variant="h3">Enable WSL2</Typography>
      <Typography variant="body" color="muted">
        dotapps needs Windows Subsystem for Linux to run containers.
      </Typography>
      <Button variant="default" onClick={onEnable} disabled={enabling}>
        {enabling ? "Enabling..." : "Enable WSL2"}
      </Button>
    </div>
  );
};

export { WSLEnableContent };
