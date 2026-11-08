import type { OrderSummary } from "@/features/orders/OrderPage";

export const totalFor = (order: OrderSummary): number => order.total.cents;
