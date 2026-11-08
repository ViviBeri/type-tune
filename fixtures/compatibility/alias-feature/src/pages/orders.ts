import type { OrderSummary } from "@/features/orders/OrderPage";

export const orderId = (order: OrderSummary): string => order.id;
