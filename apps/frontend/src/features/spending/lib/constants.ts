/** Cash activity types tracked by the spending module. */
export const CASH_ACTIVITY_TYPES = [
  "DEPOSIT",
  "WITHDRAWAL",
  "TRANSFER_IN",
  "TRANSFER_OUT",
  "FEE",
  "INTEREST",
] as const;

export type CashActivityType = (typeof CASH_ACTIVITY_TYPES)[number];

export const CASH_ACTIVITY_TYPE_LABELS: Record<CashActivityType, string> = {
  DEPOSIT: "Deposit",
  WITHDRAWAL: "Withdrawal",
  TRANSFER_IN: "Transfer In",
  TRANSFER_OUT: "Transfer Out",
  FEE: "Fee",
  INTEREST: "Interest",
};

/** Activity types that count as outflow (red, negative direction). */
export const OUTFLOW_TYPES: CashActivityType[] = ["WITHDRAWAL", "TRANSFER_OUT", "FEE"];

/** Activity types that count as income (green, positive direction). */
export const INCOME_TYPES: CashActivityType[] = ["DEPOSIT", "TRANSFER_IN", "INTEREST"];
