import { useState } from "react";
import { Button, Input, Typography } from "@/components/ui";

interface DeleteConfirmationProps {
  readonly projectName: string;
  readonly removing: boolean;
  readonly onConfirm: () => void;
  readonly onCancel: () => void;
}

const DeleteConfirmation = ({
  projectName,
  removing,
  onConfirm,
  onCancel,
}: DeleteConfirmationProps) => {
  const [inputValue, setInputValue] = useState("");

  const isConfirmed = inputValue === projectName;
  const handleInputChange = (e: React.ChangeEvent<HTMLInputElement>) =>
    setInputValue(e.target.value);

  return (
    <div className="flex flex-1 flex-col items-center justify-center p-8">
      <Typography variant="h2" className="mb-4">
        Delete {projectName} ?
      </Typography>
      <Typography variant="body" color="muted" className="mb-6 max-w-sm text-center">
        This action is irreversible. The project and all its local data will be permanently removed.
      </Typography>
      <div className="mb-6 w-full max-w-sm">
        <Typography variant="caption" color="muted" className="mb-2 block">
          Type the project name to confirm:
        </Typography>
        <Input
          value={inputValue}
          onChange={handleInputChange}
          placeholder={projectName}
          autoFocus
        />
      </div>
      <div className="flex items-center gap-3">
        <Button variant="outline" onClick={onCancel} disabled={removing}>
          Cancel
        </Button>
        <Button variant="danger" onClick={onConfirm} disabled={!isConfirmed || removing}>
          {removing ? "Deleting..." : "Delete"}
        </Button>
      </div>
    </div>
  );
};

export type { DeleteConfirmationProps };
export { DeleteConfirmation };
