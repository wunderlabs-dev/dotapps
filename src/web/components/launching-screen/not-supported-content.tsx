import { Typography } from "@/components/ui";

const NotSupportedContent = () => {
  return (
    <div className="space-y-4 text-center">
      <Typography variant="h3">Not Supported</Typography>
      <Typography variant="body" color="muted">
        This version of Windows is not supported. Please upgrade to Windows 10 version 2004 or
        later.
      </Typography>
    </div>
  );
};

export { NotSupportedContent };
