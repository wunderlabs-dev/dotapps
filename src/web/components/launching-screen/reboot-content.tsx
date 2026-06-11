import { Button, Typography } from "@/components/ui";

const RebootContent = ({ onReboot }: { readonly onReboot: () => void }) => {
  return (
    <div className="space-y-4 text-center">
      <Typography variant="h3">Reboot Required</Typography>
      <Typography variant="body" color="muted">
        WSL2 was enabled. Restart your computer to finish setup.
      </Typography>
      <Button variant="default" onClick={onReboot}>
        Restart Now
      </Button>
    </div>
  );
};

export { RebootContent };
