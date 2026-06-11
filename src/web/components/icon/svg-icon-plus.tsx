import { SvgIcon, type SvgIconProps } from "@/components/icon/svg-icon";

const SvgIconPlus = (props: Omit<SvgIconProps, "children">) => {
  return (
    <SvgIcon {...props}>
      <path d="M8,1.5c-0.3,0-0.5,0.2-0.5,0.5v5.5H2C1.7,7.5,1.5,7.7,1.5,8S1.7,8.5,2,8.5h5.5V14c0,0.3,0.2,0.5,0.5,0.5s0.5-0.2,0.5-0.5V8.5H14c0.3,0,0.5-0.2,0.5-0.5S14.3,7.5,14,7.5H8.5V2C8.5,1.7,8.3,1.5,8,1.5z" />
    </SvgIcon>
  );
};

export { SvgIconPlus };
