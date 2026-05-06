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

// Zod schema for InterestForm validation
export const interestFormSchema = z
  .object({
    accountId: z.string().min(1, { message: "Please select an account." }),
    activityDate: z.date({ required_error: "Please select a date." }),
    symbol: z.string().optional().nullable(),
    existingAssetId: z.string().nullable().optional(),
    exchangeMic: z.string().nullable().optional(),
    amount: z.coerce
      .number({
        required_error: "Please enter an amount.",
        invalid_type_error: "Amount must be a number.",
      })
      .positive({ message: "Amount must be greater than 0." }),
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
    symbolQuoteCcy: z.string().nullable().optional(),
    symbolInstrumentType: z.string().nullable().optional(),
  })
  .superRefine((data, ctx) => {
    const isStakingReward = data.subtype === ACTIVITY_SUBTYPES.STAKING_REWARD;
    if (!isStakingReward) return;

    if (!data.symbol?.trim()) {
      ctx.addIssue({
        code: z.ZodIssueCode.custom,
        path: ["symbol"],
        message: "Reward asset is required.",
      });
    }
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
        message: "Enter either interest amount or FMV per unit.",
      });
    }
  });

export type InterestFormValues = z.infer<typeof interestFormSchema>;

interface InterestFormProps {
  accounts: AccountSelectOption[];
  defaultValues?: Partial<InterestFormValues>;
  onSubmit: (data: InterestFormValues) => void | Promise<void>;
  onCancel?: () => void;
  isLoading?: boolean;
  isEditing?: boolean;
}

export function InterestForm({
  accounts,
  defaultValues,
  onSubmit,
  onCancel,
  isLoading = false,
  isEditing = false,
}: InterestFormProps) {
  const { data: settings } = useSettings();
  const baseCurrency = settings?.baseCurrency;

  // Compute initial account and currency for defaultValues
  const initialAccountId =
    defaultValues?.accountId ?? (accounts.length === 1 ? accounts[0].value : "");
  const initialAccount = accounts.find((a) => a.value === initialAccountId);
  const initialCurrency = defaultValues?.currency?.trim() || initialAccount?.currency;

  const form = useForm<InterestFormValues>({
    resolver: zodResolver(interestFormSchema) as Resolver<InterestFormValues>,
    mode: "onSubmit", // Validate only on submit - works correctly with default values
    defaultValues: {
      accountId: initialAccountId,
      activityDate: new Date(),
      symbol: null,
      amount: undefined,
      quantity: undefined,
      unitPrice: undefined,
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
  const isStakingReward = subtype === ACTIVITY_SUBTYPES.STAKING_REWARD;
  const interestMode = subtype ?? INCOME_MODE_CASH;

  useEffect(() => {
    if (!isStakingReward) return;
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
  }, [getFieldState, getValues, isStakingReward, quantity, setValue, unitPrice]);

  const handleInterestModeChange = (value: string) => {
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
              <div className="text-sm font-medium">Interest type</div>
              <RadioGroup
                value={interestMode}
                onValueChange={handleInterestModeChange}
                className="flex flex-wrap gap-4"
              >
                <div className="flex items-center space-x-2">
                  <RadioGroupItem value={INCOME_MODE_CASH} id="interest-type-cash" />
                  <Label
                    htmlFor="interest-type-cash"
                    className="cursor-pointer text-sm font-normal"
                  >
                    Cash
                  </Label>
                </div>
                <div className="flex items-center space-x-2">
                  <RadioGroupItem
                    value={ACTIVITY_SUBTYPES.STAKING_REWARD}
                    id="interest-type-staking-reward"
                  />
                  <Label
                    htmlFor="interest-type-staking-reward"
                    className="cursor-pointer text-sm font-normal"
                  >
                    Staking reward
                  </Label>
                </div>
              </RadioGroup>
            </div>

            {/* Optional Symbol (e.g., for bond interest) */}
            <SymbolSearch
              name="symbol"
              label={isStakingReward ? "Reward asset" : "Symbol (optional)"}
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

            {isStakingReward && (
              <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
                <QuantityInput name="quantity" label="Received quantity" />
                <AmountInput
                  name="unitPrice"
                  label="FMV per unit"
                  labelHelpText={FMV_PER_UNIT_HELP_TEXT}
                  maxDecimalPlaces={4}
                  currency={currency}
                />
              </div>
            )}

            <AmountInput
              name="amount"
              label={isStakingReward ? "Interest amount" : "Amount"}
              currency={currency}
            />

            {/* Advanced Options */}
            <AdvancedOptionsSection
              currencyName="currency"
              fxRateName="fxRate"
              activityType={ActivityType.INTEREST}
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
            {isEditing ? "Update" : "Add Interest"}
          </Button>
        </div>
      </form>
    </FormProvider>
  );
}
