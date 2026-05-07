import { Link } from "react-router-dom";

import { Icons } from "@wealthfolio/ui";

export function SpendingBackLink() {
  return (
    <Link
      to="/settings/spending"
      className="text-muted-foreground hover:text-foreground inline-flex items-center gap-1.5 text-sm font-medium underline-offset-4 hover:underline"
    >
      <Icons.ArrowLeft className="h-4 w-4" />
      Back to Spending Tracker
    </Link>
  );
}
