import { SvgIcon, type SvgIconProps } from "@/components/icon/svg-icon";

const SvgIconOpenableLogo = (props: Omit<SvgIconProps, "children">) => {
  return (
    <SvgIcon viewBox="0 0 73 24" {...props}>
      <path d="M35 15V9H41V15H35Z" fill="currentColor" />
      <path
        d="M28.4031 0C32.6747 0 35 2.31429 35 6.58286V17.4171C35 21.6857 32.6747 24 28.4031 24H6.61417C2.32529 24 0 21.6857 0 17.4171V6.58286C0 2.31429 2.32529 0 6.61417 0H28.4031ZM6.02854 16.6457C6.02854 18.3429 6.28691 18.6 7.99213 18.6H27.0251C28.7131 18.6 28.9715 18.3429 28.9715 16.6457V7.35429C28.9715 5.65714 28.7131 5.4 27.0251 5.4H7.99213C6.28691 5.4 6.02854 5.65714 6.02854 7.35429V16.6457Z"
        fill="currentColor"
      />
      <path
        d="M66.3319 0C70.6496 0 73 2.31429 73 6.58286V17.4171C73 21.6857 70.6496 24 66.3319 24H41V18.6H64.9217C66.6453 18.6 66.9064 18.3429 66.9064 16.6457V7.35429C66.9064 5.65714 66.6453 5.4 64.9217 5.4H41V0H66.3319Z"
        fill="currentColor"
      />
    </SvgIcon>
  );
};

export { SvgIconOpenableLogo };
