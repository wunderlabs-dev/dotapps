import { SvgIcon, type SvgIconProps } from "@/components/icon/svg-icon";

const SvgIconPower = (props: Omit<SvgIconProps, "children">) => {
  return (
    <SvgIcon {...props}>
      <path d="M8,0.5C7.7,0.5,7.5,0.7,7.5,1v6c0,0.3,0.2,0.5,0.5,0.5S8.5,7.3,8.5,7V1C8.5,0.7,8.3,0.5,8,0.5z M12.6,3.4c-0.2-0.2-0.5-0.2-0.7,0s-0.2,0.5,0,0.7c1.2,1.2,1.9,2.8,1.9,4.4c0,3.5-2.8,6.3-6.3,6.3S1.3,12,1.3,8.5c0-1.7,0.7-3.3,1.9-4.4c0.2-0.2,0.2-0.5,0-0.7s-0.5-0.2-0.7,0C1.1,4.7,0.3,6.5,0.3,8.5c0,4,3.3,7.3,7.3,7.3s7.3-3.3,7.3-7.3C14.8,6.5,14,4.7,12.6,3.4z" />
    </SvgIcon>
  );
};

export { SvgIconPower };
