export type Product = {
  sku: string;
  price: Currency;
};

type Currency = {
  cents: number;
};
