import type { Product } from "./index";

export const productPrice = (product: Product): number => product.price.cents;
