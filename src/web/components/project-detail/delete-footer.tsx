import { Button } from "@/components/ui";

const DeleteFooter = ({ onRequestDelete }: { readonly onRequestDelete: () => void }) => (
  <div className="py-4">
    <Button variant="danger" size="md" onClick={onRequestDelete}>
      Delete project
    </Button>
  </div>
);

export { DeleteFooter };
