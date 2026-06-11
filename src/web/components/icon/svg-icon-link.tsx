import { SvgIcon, type SvgIconProps } from "@/components/icon/svg-icon";

const SvgIconLink = (props: Omit<SvgIconProps, "children">) => {
  return (
    <SvgIcon viewBox="0 0 20 20" {...props}>
      <path
        d="M6.25 15C3.489 15 1.25 12.761 1.25 10C1.25 7.239 3.489 5 6.25 5M13.75 15C16.511 15 18.75 12.761 18.75 10C18.75 7.239 16.511 5 13.75 5M5.625 10H14.375"
        stroke="currentColor"
        strokeWidth="1.75"
        strokeLinecap="round"
        fill="none"
      />
    </SvgIcon>
  );
};

export { SvgIconLink };
