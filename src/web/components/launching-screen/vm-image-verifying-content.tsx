import { Spinner, Typography } from "@/components/ui";

const VmImageVerifyingContent = () => (
  <div className="flex flex-col items-center gap-3">
    <Spinner size="md" />
    <Typography variant="caption" color="muted">
      Verifying VM image...
    </Typography>
  </div>
);

export { VmImageVerifyingContent };
