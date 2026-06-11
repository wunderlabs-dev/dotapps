import { Typography } from "@/components/ui";

const OrDivider = () => (
  <div className="my-6 flex items-center gap-3">
    <div className="h-px flex-1 bg-border" />
    <Typography variant="body" className="font-semibold">
      Or
    </Typography>
    <div className="h-px flex-1 bg-border" />
  </div>
);

export { OrDivider };
