import { SvgIcon, type SvgIconProps } from "@/components/icon/svg-icon";

const SvgIconDeploy = (props: Omit<SvgIconProps, "children">) => {
  return (
    <SvgIcon {...props}>
      <path d="M8,1c-0.1,0-0.3,0.1-0.4,0.1L4.1,4.6C3.9,4.8,3.9,5.1,4.1,5.3c0.2,0.2,0.5,0.2,0.7,0L7.5,2.6V10c0,0.3,0.2,0.5,0.5,0.5s0.5-0.2,0.5-0.5V2.6l2.7,2.7c0.2,0.2,0.5,0.2,0.7,0c0.2-0.2,0.2-0.5,0-0.7L8.4,1.1C8.3,1.1,8.1,1,8,1z M13,12.5H3c-0.3,0-0.5,0.2-0.5,0.5s0.2,0.5,0.5,0.5h10c0.3,0,0.5-0.2,0.5-0.5S13.3,12.5,13,12.5z" />
    </SvgIcon>
  );
};

export { SvgIconDeploy };
