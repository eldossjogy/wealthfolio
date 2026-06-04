import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { TickerAvatar } from "./ticker-avatar";

describe("TickerAvatar", () => {
  it("renders cash symbols with a painted avatar background", () => {
    render(<TickerAvatar symbol="CASH:USD" />);

    const label = screen.getByTitle("CASH:USD");
    const avatarFallback = label.parentElement;

    expect(label).toHaveTextContent("$");
    expect(avatarFallback).toHaveClass("bg-primary/80", "dark:bg-primary/20", "text-white");
  });

  it("uses currency-specific cash labels", () => {
    render(<TickerAvatar symbol="CASH:CAD" />);

    expect(screen.getByTitle("CASH:CAD")).toHaveTextContent("C$");
  });
});
