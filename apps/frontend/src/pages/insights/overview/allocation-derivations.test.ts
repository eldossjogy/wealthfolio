import type { Account, Holding } from "@/lib/types";
import { describe, expect, it } from "vitest";
import { computeValueStrip } from "./allocation-derivations";

function holding({
  id,
  accountId,
  holdingType,
  localCurrency,
  baseCurrency = "USD",
  localValue,
  baseValue,
  dayChangeBase,
  prevCloseBase,
}: {
  id: string;
  accountId: string;
  holdingType: "cash" | "security";
  localCurrency: string;
  baseCurrency?: string;
  localValue: number;
  baseValue: number;
  dayChangeBase?: number;
  prevCloseBase?: number;
}): Holding {
  return {
    id,
    accountId,
    holdingType,
    localCurrency,
    baseCurrency,
    marketValue: { local: localValue, base: baseValue },
    dayChange: dayChangeBase == null ? null : { local: dayChangeBase, base: dayChangeBase },
    prevCloseValue: prevCloseBase == null ? null : { local: prevCloseBase, base: prevCloseBase },
    quantity: 1,
    weight: 0,
    asOfDate: "2026-05-30",
  } as Holding;
}

function account(id: string, name: string, group?: string): Account {
  return {
    id,
    name,
    group,
    accountType: "SECURITIES",
    balance: 0,
    currency: "USD",
    isDefault: false,
    isActive: true,
    isArchived: false,
    trackingMode: "NOT_SET",
    createdAt: new Date("2026-05-30"),
    updatedAt: new Date("2026-05-30"),
  } as Account;
}

describe("allocation dashboard derivations", () => {
  it("derives total exposure and cash-by-currency from holdings", () => {
    const data = computeValueStrip(
      [
        holding({
          id: "equity-usd",
          accountId: "taxable",
          holdingType: "security",
          localCurrency: "USD",
          localValue: 500,
          baseValue: 500,
          dayChangeBase: 10,
          prevCloseBase: 490,
        }),
        holding({
          id: "equity-cad",
          accountId: "rrsp",
          holdingType: "security",
          localCurrency: "CAD",
          localValue: 350,
          baseValue: 250,
          dayChangeBase: -5,
          prevCloseBase: 255,
        }),
        holding({
          id: "cash-usd",
          accountId: "taxable",
          holdingType: "cash",
          localCurrency: "USD",
          localValue: 100,
          baseValue: 100,
        }),
        holding({
          id: "cash-cad",
          accountId: "taxable",
          holdingType: "cash",
          localCurrency: "CAD",
          localValue: 70,
          baseValue: 50,
        }),
      ],
      [account("taxable", "Taxable"), account("rrsp", "RRSP")],
    );

    expect(data.total).toBe(900);
    expect(data.cash).toBe(150);
    expect(data.invested).toBe(750);
    expect(data.accountsCount).toBe(2);

    const usdExposure = data.currencySplit.find((row) => row.currency === "USD");
    const cadExposure = data.currencySplit.find((row) => row.currency === "CAD");
    expect(usdExposure?.value).toBe(600);
    expect(usdExposure?.percentage).toBeCloseTo(66.67, 2);
    expect(cadExposure?.value).toBe(300);
    expect(cadExposure?.percentage).toBeCloseTo(33.33, 2);

    const usdCash = data.cashCurrencySplit.find((row) => row.currency === "USD");
    const cadCash = data.cashCurrencySplit.find((row) => row.currency === "CAD");
    expect(usdCash?.value).toBe(100);
    expect(usdCash?.percentage).toBeCloseTo(66.67, 2);
    expect(cadCash?.value).toBe(70);
    expect(cadCash?.percentage).toBeCloseTo(33.33, 2);
  });
});
