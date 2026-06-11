import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { ForkBadge } from "./fork-badge";

describe("ForkBadge", () => {
  it("renders the Fork label", () => {
    render(<ForkBadge />);
    expect(screen.getByText("Fork")).toBeInTheDocument();
  });
});
