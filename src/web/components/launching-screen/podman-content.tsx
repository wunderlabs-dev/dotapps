import { Typography } from "@/components/ui";

const PodmanContent = () => {
  return (
    <div className="space-y-4 text-center">
      <Typography variant="h3">Podman Required</Typography>
      <Typography variant="body" color="muted">
        Install Podman to use Opnble on Linux. Visit podman.io for instructions.
      </Typography>
    </div>
  );
};

export { PodmanContent };
