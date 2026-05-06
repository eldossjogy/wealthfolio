import { useSettings } from "@/hooks/use-settings";
import { ACTIVITY_SUBTYPES, ActivityType } from "@/lib/constants";
import { roundDecimal } from "@/lib/utils";
import { zodResolver } from "@hookform/resolvers/zod";
import { Button } from "@wealthfolio/ui/components/ui/button";
import { Card, CardContent } from "@wealthfolio/ui/components/ui/card";
import { Icons } from "@wealthfolio/ui/components/ui/icons";
import { Label } from "@wealthfolio/ui/components/ui/label";
import { RadioGroup, RadioGroupItem } from "@wealthfolio/ui/components/ui/radio-group";
import { useEffect, useMemo } from "react";
import { FormProvider, useForm, type Resolver } from "react-hook-form";
import { z } from "zod";
import {
  AccountSelect,
  AdvancedOptionsSection,
  AmountInput,
  createValidatedSubmit,
  DatePicker,
  NotesInput,
  QuantityInput,
  SymbolSearch,
  type AccountSelectOption,
} from "./fields";

const FMV_PER_UNIT_HELP_TEXT =
  "Fair market value per share or token at the time you received it. Used to calculate income amount and cost basis.";
const INCOME_MODE_CASH = "CASH";

// Zod schema for DividendForm validation
export const dividendFormSchema = z
  .object({
    accountId: z.string().min(1, { message: "Please select an account." }),
    symbol: z.string().min(1, { message: "Please enter a symbol." }),
    existingAssetId: z.string().nullable().optional(),
    exchangeMic: z.string().nullable().optional(),
    activityDate: z.date({ required_error: "Please select a date." }),
    amount: z.coerce
      .number({
        required_error: "Please enter an amount.",
        invalid_type_error: "Amount must be a number.",
      })
      .positive({ message: "Amount must be greater than 0." }),
    comment: z.string().optional().nullable(),
    // Advanced options
    currency: z.string().min(1, { message: "Currency is required." }),
    fxRate: z.coerce
      .number({
        invalid_type_error: "FX Rate must be a number.",
      })
      .positive({ message: "FX Rate must be positive." })
      .optional(),
    subtype: z.string().optional().nullable(),
    unitPrice: z.coerce
      .number({
        invalid_type_error: "FMV per unit must be a number.",
      })
      .positive({ message: "FMV per unit must be greater than 0." })
      .optional(),
    quantity: z.coerce
      .number({
        invalid_type_error: "Received quantity must be a number.",
      })
      .positive({ message: "Received quantity must be greater than 0." })
      .optional(),
    symbolQuoteCcy: z.string().nullable().optional(),
    symbolInstrumentType: z.string().nullable().optional(),
  })
  .superRefine((data, ctx) => {
    const isAssetBacked =
      data.subtype === ACTIVITY_SUBTYPES.DRIP ||
      data.subtype === ACTIVITY_SUBTYPES.DIVIDEND_IN_KIND;
    if (!isAssetBacked) return;

    if (!data.quantity) {
      ctx.addIssue({
        code: z.ZodIssueCode.custom,
        path: ["quantity"],
        message: "Received quantity is required.",
      });
    }
    if (!data.unitPrice) {
      if (data.amount) return;
      ctx.addIssue({
        code: z.ZodIssueCode.custom,
        path: ["unitPrice"],
        message: "Enter either dividend amount or FMV per unit.",
      });
    }
  });

export type DividendFormValues = z.infer<typeof dividendFormSchema>;

interface DividendFormProps {
  accounts: AccountSelectOption[];
  defaultValues?: Partial<DividendFormValues>;
  onSubmit: (data: DividendFormValues) => void | Promise<void>;
  onCancel?: () => void;
  isLoading?: boolean;
  isEditing?: boolean;
  /** Whether to show manual symbol input instead of search */
  isManualSymbol?: boolean;
  /** Asset currency (from selected symbol) for advanced options */
  assetCurrency?: string;
}

export function DividendForm({
  accounts,
  defaultValues,
  onSubmit,
  onCancel,
  isLoading = false,
  isEditing = false,
  isManualSymbol = false,
  assetCurrency,
}: DividendFormProps) {
  const { data: settings } = useSettings();
  const baseCurrency = settings?.baseCurrency;

  // Compute initial account and currency for defaultValues
  const initialAccountId =
    defaultValues?.accountId ?? (accounts.length === 1 ? accounts[0].value : "");
  const initialAccount = accounts.find((a) => a.value === initialAccountId);
  const initialCurrency =
    defaultValues?.currency?.trim() || assetCurrency?.trim() || initialAccount?.currency;

  const form = useForm<DividendFormValues>({
    resolver: zodResolver(dividendFormSchema) as Resolver<DividendFormValues>,
    mode: "onSubmit", // Validate only on submit - works correctly with default values
    defaultValues: {
      accountId: initialAccountId,
      symbol: "",
      activityDate: new Date(),
      amount: undefined,
      comment: null,
      fxRate: undefined,
      subtype: null,
      ...defaultValues,
      currency: defaultValues?.currency?.trim() || initialCurrency,
    },
  });

  const { watch } = form;
  const { getFieldState, getValues, setValue } = form;
  const accountId = watch("accountId");
  const currency = watch("currency");
  const subtype = watch("subtype");
  const quantity = watch("quantity");
  const unitPrice = watch("unitPrice");
  const isAssetBacked =
    subtype === ACTIVITY_SUBTYPES.DRIP || subtype === ACTIVITY_SUBTYPES.DIVIDEND_IN_KIND;
  const dividendMode = subtype ?? INCOME_MODE_CASH;

  useEffect(() => {
    if (!isAssetBacked) return;
    const q = Number(quantity);
    const p = Number(unitPrice);
    const currentAmount = Number(getValues("amount"));
    const quantityIsDirty = getFieldState("quantity").isDirty;
    const unitPriceIsDirty = getFieldState("unitPrice").isDirty;
    const shouldAutoSetAmount =
      quantityIsDirty || unitPriceIsDirty || !(Number.isFinite(currentAmount) && currentAmount > 0);
    if (q > 0 && p > 0 && shouldAutoSetAmount) {
      const computedAmount = roundDecimal(q * p);
      if (currentAmount !== computedAmount) {
        setValue("amount", computedAmount, {
          shouldDirty: quantityIsDirty || unitPriceIsDirty,
          shouldValidate: false,
        });
      }
    }
  }, [getFieldState, getValues, isAssetBacked, quantity, setValue, unitPrice]);

  const handleDividendModeChange = (value: string) => {
    setValue("subtype", value === INCOME_MODE_CASH ? null : value, {
      shouldDirty: true,
      shouldValidate: true,
    });
    if (value === INCOME_MODE_CASH) {
      setValue("quantity", undefined, { shouldDirty: true, shouldValidate: false });
      setValue("unitPrice", undefined, { shouldDirty: true, shouldValidate: false });
    }
  };

  // Get account currency from selected account
  const selectedAccount = useMemo(
    () => accounts.find((a) => a.value === accountId),
    [accounts, accountId],
  );
  const accountCurrency = selectedAccount?.currency;

  const handleSubmit = createValidatedSubmit(form, async (data) => {
    await onSubmit(data);
  });

  return (
    <FormProvider {...form}>
      <form onSubmit={handleSubmit} className="space-y-4">
        <Card>
          <CardContent className="space-y-6 pt-4">
            {/* Account Selection */}
            <AccountSelect name="accountId" accounts={accounts} currencyName="currency" />

            <div className="space-y-2">
              <div className="text-sm font-medium">Dividend type</div>
              <RadioGroup
                value={dividendMode}
                onValueChange={handleDividendModeChange}
                className="flex flex-wrap gap-4"
              >
                <div className="flex items-center space-x-2">
                  <RadioGroupItem value={INCOME_MODE_CASH} id="dividend-type-cash" />
                  <Label
                    htmlFor="dividend-type-cash"
                    className="cursor-pointer text-sm font-normal"
                  >
                    Cash
                  </Label>
                </div>
                <div className="flex items-center space-x-2">
                  <RadioGroupItem value={ACTIVITY_SUBTYPES.DRIP} id="dividend-type-drip" />
                  <Label
                    htmlFor="dividend-type-drip"
                    className="cursor-pointer text-sm font-normal"
                  >
                    DRIP
                  </Label>
                </div>
                <div className="flex items-center space-x-2">
                  <RadioGroupItem
                    value={ACTIVITY_SUBTYPES.DIVIDEND_IN_KIND}
                    id="dividend-type-in-kind"
                  />
                  <Label
                    htmlFor="dividend-type-in-kind"
                    className="cursor-pointer text-sm font-normal"
                  >
                    In kind
                  </Label>
                </div>
              </RadioGroup>
            </div>

            {/* Symbol Search/Input */}
            <SymbolSearch
              name="symbol"
              label="Asset"
              isManualAsset={isManualSymbol}
              exchangeMicName="exchangeMic"
              currencyName="currency"
              quoteCcyName="symbolQuoteCcy"
              instrumentTypeName="symbolInstrumentType"
              existingAssetIdName="existingAssetId"
            />
            <input type="hidden" {...form.register("symbolQuoteCcy")} />
            <input type="hidden" {...form.register("symbolInstrumentType")} />
            <input type="hidden" {...form.register("existingAssetId")} />

            {/* Date Picker */}
            <DatePicker name="activityDate" label="Date" />

            {/* Amount */}
            {isAssetBacked && (
              <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
                <QuantityInput
                  name="quantity"
                  label={
                    subtype === ACTIVITY_SUBTYPES.DRIP ? "Reinvested quantity" : "Received quantity"
                  }
                />
                <AmountInput
                  name="unitPrice"
                  label={subtype === ACTIVITY_SUBTYPES.DRIP ? "Reinvestment price" : "FMV per unit"}
                  labelHelpText={
                    subtype === ACTIVITY_SUBTYPES.DRIP ? undefined : FMV_PER_UNIT_HELP_TEXT
                  }
                  maxDecimalPlaces={4}
                  currency={currency}
                />
              </div>
            )}

            <AmountInput
              name="amount"
              label={isAssetBacked ? "Dividend amount" : "Amount"}
              currency={currency}
            />

            {/* Advanced Options */}
            <AdvancedOptionsSection
              currencyName="currency"
              fxRateName="fxRate"
              activityType={ActivityType.DIVIDEND}
              assetCurrency={assetCurrency}
              accountCurrency={accountCurrency}
              baseCurrency={baseCurrency}
              showSubtype={false}
            />

            {/* Notes */}
            <NotesInput name="comment" label="Notes" placeholder="Add an optional note..." />
          </CardContent>
        </Card>

        {/* Action Buttons */}
        <div className="flex justify-end gap-2">
          {onCancel && (
            <Button type="button" variant="outline" onClick={onCancel} disabled={isLoading}>
              Cancel
            </Button>
          )}
          <Button type="submit" disabled={isLoading}>
            {isLoading && <Icons.Spinner className="mr-2 h-4 w-4 animate-spin" />}
            {isEditing ? (
              <Icons.Check className="mr-2 h-4 w-4" />
            ) : (
              <Icons.Plus className="mr-2 h-4 w-4" />
            )}
            {isEditing ? "Update" : "Add Dividend"}
          </Button>
        </div>
      </form>
    </FormProvider>
  );
}
