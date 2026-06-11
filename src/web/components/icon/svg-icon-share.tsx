import { SvgIcon, type SvgIconProps } from "@/components/icon/svg-icon";

const NODE_RADIUS = "2";

const SvgIconShare = (props: Omit<SvgIconProps, "children">) => {
  return (
    <SvgIcon {...props}>
      <circle cx="12" cy="3" r={NODE_RADIUS} />
      <circle cx="12" cy="13" r={NODE_RADIUS} />
      <circle cx="4" cy="8" r={NODE_RADIUS} />
      <path d="M5.7 7L10.3 4M5.7 9L10.3 12" stroke="currentColor" strokeWidth="1.5" fill="none" />
    </SvgIcon>
  );
};

export { SvgIconShare };
