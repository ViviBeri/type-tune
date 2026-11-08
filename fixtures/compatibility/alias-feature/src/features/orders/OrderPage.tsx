export type OrderSummary = {
  id: string;
  total: Money;
};

type Money = {
  cents: number;
};
