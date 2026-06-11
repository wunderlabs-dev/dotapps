/* eslint-disable local/no-multi-comp -- SettingsSection sub-components are part of one compound primitive */
import type { ReactNode } from "react";

import { Typography } from "@/components/ui";
import { cn } from "@/lib/cn";

interface SettingsSectionProps {
  readonly title: string;
  readonly children: ReactNode;
}

const SettingsSection = ({ title, children }: SettingsSectionProps) => (
  <section>
    <Typography variant="overline" as="h2" color="muted" className="mb-2 px-1">
      {title}
    </Typography>
    {children}
  </section>
);

interface SettingsSectionBodyProps extends React.HTMLAttributes<HTMLDivElement> {
  readonly divided?: boolean;
}

const SettingsSectionBody = ({
  divided = false,
  className,
  ...props
}: SettingsSectionBodyProps) => (
  <div
    className={cn(
      "rounded-2xl bg-surface shadow-inset-bevel",
      divided ? "divide-y divide-border-subtle" : "p-6",
      className,
    )}
    {...props}
  />
);

export type { SettingsSectionBodyProps, SettingsSectionProps };
export { SettingsSection, SettingsSectionBody };
