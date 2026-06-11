import { SvgIcon, type SvgIconProps } from "@/components/icon/svg-icon";

const SvgIconClose = (props: Omit<SvgIconProps, "children">) => {
  return (
    <SvgIcon {...props}>
      <path d="M12.2,3.8c-0.3-0.3-0.7-0.3-1,0L8,7l-3.2-3.2c-0.3-0.3-0.7-0.3-1,0s-0.3,0.7,0,1L7,8l-3.2,3.2c-0.3,0.3-0.3,0.7,0,1s0.7,0.3,1,0L8,9l3.2,3.2c0.3,0.3,0.7,0.3,1,0s0.3-0.7,0-1L9,8l3.2-3.2C12.5,4.5,12.5,4.1,12.2,3.8z" />
    </SvgIcon>
  );
};

export { SvgIconClose };
