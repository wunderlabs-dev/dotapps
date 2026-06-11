import { SvgIcon, type SvgIconProps } from "@/components/icon/svg-icon";

const SvgIconNextjs = (props: Omit<SvgIconProps, "children">) => {
  return (
    <SvgIcon {...props}>
      <circle cx="8" cy="8" r="8" />
      <path d="M5.8 11.2V4.8h.9l4.5 5V4.8h.8v6.4h-.7L6.6 6v5.2z" fill="var(--color-background)" />
    </SvgIcon>
  );
};

export { SvgIconNextjs };
