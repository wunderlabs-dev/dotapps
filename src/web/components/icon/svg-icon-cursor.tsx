import { SvgIcon, type SvgIconProps } from "@/components/icon/svg-icon";

const SvgIconCursor = (props: Omit<SvgIconProps, "children">) => {
  return (
    <SvgIcon {...props}>
      <path
        d="M9.125 8.504V14.585L3.875 11.547V5.466L9.125 8.504ZM16.125 11.547L11.904 13.989L16.125 6.661V11.547ZM14.226 3.354H5.774L10 0.908L14.226 3.354Z"
        fill="currentColor"
        stroke="currentColor"
        strokeWidth={1.75}
      />
    </SvgIcon>
  );
};

export { SvgIconCursor };
