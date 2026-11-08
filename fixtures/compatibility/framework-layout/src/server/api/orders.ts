import type { OrderFilter } from "../../features/orders/model";

export const queryOrders = (filter: OrderFilter): string => filter.state;
