import type { OrderFilter } from "../features/orders/model";

export const visibleStates = (filter: OrderFilter): string[] => [filter.state];
