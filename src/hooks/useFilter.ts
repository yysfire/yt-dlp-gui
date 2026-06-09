import { useMemo, useState, useCallback } from "react";
import type { Subscription, FilterState, SortState } from "../types";

export const DEFAULT_FILTER: FilterState = {
  platform: "all",
  status: "all",
  group: "全部",
  health: "all",
  keyword: "",
};

export const DEFAULT_SORT: SortState = { field: "name", direction: "asc" };

export function useFilter(subscriptions: Subscription[]) {
  const [filter, setFilterState] = useState<FilterState>(DEFAULT_FILTER);
  const [sort, setSortState] = useState<SortState>(DEFAULT_SORT);

  const setFilter = useCallback((partial: Partial<FilterState>) => {
    setFilterState((prev) => ({ ...prev, ...partial }));
  }, []);

  const filtered = useMemo(() => {
    return subscriptions.filter((sub) => {
      if (filter.platform !== "all" && sub.platform !== filter.platform) return false;
      if (filter.status === "active" && sub.paused) return false;
      if (filter.status === "paused" && !sub.paused) return false;
      if (filter.group !== "全部") {
        if (filter.group === "未分组" && sub.group_name !== "未分组") return false;
        if (filter.group !== "未分组" && sub.group_name !== filter.group) return false;
      }
      if (filter.health !== "all") {
        if (filter.health === "unchecked" && sub.health_status !== null) return false;
        if (filter.health === "ok" && sub.health_status !== "ok") return false;
        if (filter.health === "warning" && sub.health_status !== "warning") return false;
        if (filter.health === "dead" && sub.health_status !== "dead") return false;
      }
      if (filter.keyword) {
        const kw = filter.keyword.toLowerCase();
        if (!sub.channel_name.toLowerCase().includes(kw) && !sub.url.toLowerCase().includes(kw)) return false;
      }
      return true;
    });
  }, [subscriptions, filter]);

  const sorted = useMemo(() => {
    const list = [...filtered];
    const dir = sort.direction === "asc" ? 1 : -1;
    list.sort((a, b) => {
      switch (sort.field) {
        case "name":
          return dir * a.channel_name.localeCompare(b.channel_name, "zh-CN", { sensitivity: "base" });
        case "created_at":
          return dir * a.created_at.localeCompare(b.created_at);
        case "last_health_check": {
          if (!a.last_health_check && !b.last_health_check) return 0;
          if (!a.last_health_check) return 1;
          if (!b.last_health_check) return -1;
          return dir * a.last_health_check.localeCompare(b.last_health_check);
        }
        default:
          return 0;
      }
    });
    return list;
  }, [filtered, sort]);

  return { filter, setFilter, sort, setSort: setSortState, filtered: sorted, count: filtered.length };
}
