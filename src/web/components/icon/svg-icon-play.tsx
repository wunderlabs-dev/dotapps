import { SvgIcon, type SvgIconProps } from "@/components/icon/svg-icon";

const SvgIconPlay = (props: Omit<SvgIconProps, "children">) => {
  return (
    <SvgIcon {...props}>
      <path
        fillRule="evenodd"
        clipRule="evenodd"
        d="M13.5 6.3 6.1 1.8C4.9 1 3.3 2 3.3 3.5v9c0 1.5 1.5 2.5 2.8 1.7l7.4-4.5c1.2-.7 1.2-2.7 0-3.4"
      />
    </SvgIcon>
  );
};

export { SvgIconPlay };
