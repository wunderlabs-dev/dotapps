import { Typography } from "@/components/ui";

const VirtualizationContent = () => {
  return (
    <div className="space-y-4 text-center">
      <Typography variant="h3">Virtualization Disabled</Typography>
      <Typography variant="body" color="muted">
        Enable CPU virtualization (VT-x / AMD-V) in your BIOS settings, then relaunch Opnble.
      </Typography>
    </div>
  );
};

export { VirtualizationContent };
