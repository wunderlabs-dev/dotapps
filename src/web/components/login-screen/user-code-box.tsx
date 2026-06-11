import { Typography } from "@/components/ui";

const UserCodeBox = ({ userCode }: { readonly userCode: string }) => {
  return (
    <div className="rounded border border-border bg-surface-elevated p-4 text-center">
      <Typography variant="caption" color="muted">
        Enter this code on GitHub:
      </Typography>
      <Typography variant="h2" color="default">
        {userCode}
      </Typography>
    </div>
  );
};

export { UserCodeBox };
