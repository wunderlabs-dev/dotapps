import { Button } from "@/components/ui";

const ConnectButton = ({ onStart }: { readonly onStart: () => Promise<void> }) => {
  return (
    <Button
      variant="default"
      size="lg"
      onClick={() => {
        onStart();
      }}
      className="w-full rounded-full border-accent-warning bg-accent-warning hover:bg-transparent hover:text-accent-warning"
    >
      <svg
        aria-hidden="true"
        width="20"
        height="20"
        viewBox="0 0 20 20"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.75"
        strokeLinecap="round"
        strokeLinejoin="round"
      >
        <path d="M15.833 17.5v-1.667a3.333 3.333 0 0 0-3.333-3.333H7.5a3.333 3.333 0 0 0-3.333 3.333V17.5" />
        <circle cx="10" cy="5.833" r="3.333" />
      </svg>
      Connect with GitHub
    </Button>
  );
};

export { ConnectButton };
