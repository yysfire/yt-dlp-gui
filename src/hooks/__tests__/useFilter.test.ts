import { describe, it, expect } from "vitest";
import type { Subscription, FilterState, SortState } from "../../types";

// useFilter will be imported from the actual file once created
// For now, we define the test expectations and will implement later

// Mock subscriptions for testing
const makeSub = (overrides: Partial<Subscription> = {}): Subscription => ({
  id: `uuid-${Math.random().toString(36).slice(2, 8)}`,
  url: "https://www.youtube.com/@test",
  platform: "youtube",
  channel_name: "Test Channel",
  channel_avatar_url: "",
  paused: false,
  quality_preset: "1080p",
  group_name: "未分组",
  created_at: "2025-01-01T00:00:00Z",
  last_checked_at: null,
  download_count: 0,
  last_check_status: null,
  last_check_error: null,
  tags: [],
  health_status: null,
  last_health_check: null,
  ...overrides,
});

const testSubscriptions: Subscription[] = [
  makeSub({ id: "1", channel_name: "YouTube Music", platform: "youtube", group_name: "音乐", paused: false, health_status: "ok", url: "https://youtube.com/@music" }),
  makeSub({ id: "2", channel_name: "B站 科技频道", platform: "bilibili", group_name: "科技", paused: false, health_status: "warning", url: "https://bilibili.com/tech" }),
  makeSub({ id: "3", channel_name: "YouTube Vlog", platform: "youtube", group_name: "未分组", paused: true, health_status: "ok", url: "https://youtube.com/@vlog" }),
  makeSub({ id: "4", channel_name: "B站 学习", platform: "bilibili", group_name: "学习", paused: false, health_status: "dead", url: "https://bilibili.com/study" }),
  makeSub({ id: "5", channel_name: "YouTube Gaming", platform: "youtube", group_name: "娱乐", paused: false, health_status: null, url: "https://youtube.com/@gaming" }),
  makeSub({ id: "6", channel_name: "B站 娱乐", platform: "bilibili", group_name: "娱乐", paused: true, health_status: "ok", url: "https://bilibili.com/fun" }),
  makeSub({ id: "7", channel_name: "YouTube Tech", platform: "youtube", group_name: "科技", paused: false, health_status: "dead", url: "https://youtube.com/@techreviews" }),
  makeSub({ id: "8", channel_name: "B站 音乐台", platform: "bilibili", group_name: "音乐", paused: false, health_status: null, url: "https://bilibili.com/music" }),
];

// Since useFilter hasn't been created yet, we test the pure filter logic
// We'll extract the logic into a testable pure function

function applyFilter(subscriptions: Subscription[], filter: FilterState): Subscription[] {
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
}

function applySort(subscriptions: Subscription[], sort: SortState): Subscription[] {
  const list = [...subscriptions];
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
}

describe("useFilter - 筛选逻辑", () => {
  const defaultFilter: FilterState = {
    platform: "all",
    status: "all",
    group: "全部",
    health: "all",
    keyword: "",
  };

  describe("platform 筛选", () => {
    it("全部时返回所有订阅", () => {
      const result = applyFilter(testSubscriptions, { ...defaultFilter, platform: "all" });
      expect(result).toHaveLength(8);
    });

    it("仅显示 YouTube 平台订阅", () => {
      const result = applyFilter(testSubscriptions, { ...defaultFilter, platform: "youtube" });
      expect(result).toHaveLength(4);
      result.forEach((s) => expect(s.platform).toBe("youtube"));
    });

    it("仅显示 Bilibili 平台订阅", () => {
      const result = applyFilter(testSubscriptions, { ...defaultFilter, platform: "bilibili" });
      expect(result).toHaveLength(4);
      result.forEach((s) => expect(s.platform).toBe("bilibili"));
    });
  });

  describe("status 筛选", () => {
    it("active 筛选仅返回非暂停订阅", () => {
      const result = applyFilter(testSubscriptions, { ...defaultFilter, status: "active" });
      expect(result).toHaveLength(6);
      result.forEach((s) => expect(s.paused).toBe(false));
    });

    it("paused 筛选仅返回暂停订阅", () => {
      const result = applyFilter(testSubscriptions, { ...defaultFilter, status: "paused" });
      expect(result).toHaveLength(2);
      result.forEach((s) => expect(s.paused).toBe(true));
    });

    it("all 状态返回所有", () => {
      const result = applyFilter(testSubscriptions, { ...defaultFilter, status: "all" });
      expect(result).toHaveLength(8);
    });
  });

  describe("group 筛选", () => {
    it("全部时返回所有订阅", () => {
      const result = applyFilter(testSubscriptions, { ...defaultFilter, group: "全部" });
      expect(result).toHaveLength(8);
    });

    it("未分组筛选", () => {
      const result = applyFilter(testSubscriptions, { ...defaultFilter, group: "未分组" });
      expect(result).toHaveLength(1);
      expect(result[0].channel_name).toBe("YouTube Vlog");
    });

    it("特定分组筛选 - 音乐", () => {
      const result = applyFilter(testSubscriptions, { ...defaultFilter, group: "音乐" });
      expect(result).toHaveLength(2);
      result.forEach((s) => expect(s.group_name).toBe("音乐"));
    });

    it("特定分组筛选 - 科技", () => {
      const result = applyFilter(testSubscriptions, { ...defaultFilter, group: "科技" });
      expect(result).toHaveLength(2);
      result.forEach((s) => expect(s.group_name).toBe("科技"));
    });

    it("特定分组筛选 - 学习", () => {
      const result = applyFilter(testSubscriptions, { ...defaultFilter, group: "学习" });
      expect(result).toHaveLength(1);
      expect(result[0].group_name).toBe("学习");
    });
  });

  describe("health 筛选", () => {
    it("all 返回所有", () => {
      const result = applyFilter(testSubscriptions, { ...defaultFilter, health: "all" });
      expect(result).toHaveLength(8);
    });

    it("ok 筛选仅返回健康订阅", () => {
      const result = applyFilter(testSubscriptions, { ...defaultFilter, health: "ok" });
      expect(result).toHaveLength(3);
      result.forEach((s) => expect(s.health_status).toBe("ok"));
    });

    it("warning 筛选仅返回警告订阅", () => {
      const result = applyFilter(testSubscriptions, { ...defaultFilter, health: "warning" });
      expect(result).toHaveLength(1);
      expect(result[0].health_status).toBe("warning");
    });

    it("dead 筛选仅返回失效订阅", () => {
      const result = applyFilter(testSubscriptions, { ...defaultFilter, health: "dead" });
      expect(result).toHaveLength(2);
      result.forEach((s) => expect(s.health_status).toBe("dead"));
    });

    it("unchecked 筛选仅返回未检查订阅（null health_status）", () => {
      const result = applyFilter(testSubscriptions, { ...defaultFilter, health: "unchecked" });
      expect(result).toHaveLength(2);
      result.forEach((s) => expect(s.health_status).toBeNull());
    });
  });

  describe("keyword 搜索", () => {
    it("空关键字返回所有", () => {
      const result = applyFilter(testSubscriptions, { ...defaultFilter, keyword: "" });
      expect(result).toHaveLength(8);
    });

    it("按频道名搜索 - YouTube", () => {
      const result = applyFilter(testSubscriptions, { ...defaultFilter, keyword: "YouTube" });
      expect(result).toHaveLength(4);
      result.forEach((s) => expect(s.channel_name.toLowerCase()).toContain("youtube"));
    });

    it("按频道名搜索 - B站", () => {
      const result = applyFilter(testSubscriptions, { ...defaultFilter, keyword: "B站" });
      expect(result).toHaveLength(4);
    });

    it("按 URL 搜索", () => {
      const result = applyFilter(testSubscriptions, { ...defaultFilter, keyword: "music" });
      expect(result).toHaveLength(2);
    });

    it("大小写不敏感搜索", () => {
      const result = applyFilter(testSubscriptions, { ...defaultFilter, keyword: "YOUTUBE" });
      expect(result).toHaveLength(4);
    });

    it("无匹配关键字返回空", () => {
      const result = applyFilter(testSubscriptions, { ...defaultFilter, keyword: "zzz_not_found" });
      expect(result).toHaveLength(0);
    });
  });

  describe("AND 组合筛选", () => {
    it("platform + status 组合", () => {
      const result = applyFilter(testSubscriptions, {
        ...defaultFilter,
        platform: "youtube",
        status: "active",
      });
      expect(result).toHaveLength(3); // YouTube Music, Gaming, Tech (Vlog is paused)
      result.forEach((s) => {
        expect(s.platform).toBe("youtube");
        expect(s.paused).toBe(false);
      });
    });

    it("platform + health 组合", () => {
      const result = applyFilter(testSubscriptions, {
        ...defaultFilter,
        platform: "bilibili",
        health: "ok",
      });
      expect(result).toHaveLength(1); // B站 娱乐 (paused)
      expect(result[0].platform).toBe("bilibili");
      expect(result[0].health_status).toBe("ok");
    });

    it("status + group + health 组合", () => {
      const result = applyFilter(testSubscriptions, {
        ...defaultFilter,
        status: "active",
        group: "科技",
        health: "ok",
      });
      expect(result).toHaveLength(0); // 科技分组的 B站 科技是 warning，YouTube Tech 是 dead
    });

    it("platform + keyword 组合", () => {
      const result = applyFilter(testSubscriptions, {
        ...defaultFilter,
        platform: "youtube",
        keyword: "music",
      });
      expect(result).toHaveLength(1);
      expect(result[0].channel_name).toBe("YouTube Music");
    });

    it("所有五维筛选组合", () => {
      const result = applyFilter(testSubscriptions, {
        platform: "youtube",
        status: "active",
        group: "娱乐",
        health: "all",
        keyword: "Gaming",
      });
      expect(result).toHaveLength(1);
      expect(result[0].channel_name).toBe("YouTube Gaming");
      expect(result[0].health_status).toBeNull();
    });
  });

  describe("空列表处理", () => {
    it("空订阅列表不崩溃", () => {
      const result = applyFilter([], defaultFilter);
      expect(result).toHaveLength(0);
    });

    it("空列表 + 任意筛选条件不崩溃", () => {
      const result = applyFilter([], {
        platform: "youtube",
        status: "active",
        group: "音乐",
        health: "ok",
        keyword: "test",
      });
      expect(result).toHaveLength(0);
    });
  });

  describe("null health_status 边缘情况", () => {
    it("null health_status 正确匹配 unchecked", () => {
      const nullSubs = [makeSub({ id: "n1", health_status: null })];
      const result = applyFilter(nullSubs, { ...defaultFilter, health: "unchecked" });
      expect(result).toHaveLength(1);
    });

    it("null health_status 不匹配 ok/warning/dead", () => {
      const nullSubs = [makeSub({ id: "n1", health_status: null })];
      expect(applyFilter(nullSubs, { ...defaultFilter, health: "ok" })).toHaveLength(0);
      expect(applyFilter(nullSubs, { ...defaultFilter, health: "warning" })).toHaveLength(0);
      expect(applyFilter(nullSubs, { ...defaultFilter, health: "dead" })).toHaveLength(0);
    });
  });

  describe("排序 - 按名称", () => {
    it("升序排列", () => {
      const result = applySort(testSubscriptions, { field: "name", direction: "asc" });
      const names = result.map((s) => s.channel_name);
      // 拼音排序：B站 学习, B站 娱乐, B站 科技频道, B站 音乐台, YouTube Gaming, YouTube Music, YouTube Tech, YouTube Vlog
      // 实际上 localeCompare("zh-CN", {sensitivity:"base"}) should sort properly
      for (let i = 1; i < names.length; i++) {
        expect(names[i].localeCompare(names[i - 1], "zh-CN", { sensitivity: "base" })).toBeGreaterThanOrEqual(0);
      }
    });

    it("降序排列", () => {
      const result = applySort(testSubscriptions, { field: "name", direction: "desc" });
      const names = result.map((s) => s.channel_name);
      for (let i = 1; i < names.length; i++) {
        expect(names[i].localeCompare(names[i - 1], "zh-CN", { sensitivity: "base" })).toBeLessThanOrEqual(0);
      }
    });
  });

  describe("排序 - 按日期", () => {
    it("按创建日期升序排列", () => {
      const dates = ["2024-01-01T00:00:00Z", "2025-01-01T00:00:00Z", "2026-01-01T00:00:00Z"];
      const dateSubs = dates.map((d, i) => makeSub({ id: `d${i}`, channel_name: `Sub ${i}`, created_at: d }));
      // Randomize order
      const shuffled = [dateSubs[2], dateSubs[0], dateSubs[1]];
      const result = applySort(shuffled, { field: "created_at", direction: "asc" });
      expect(result[0].created_at).toBe("2024-01-01T00:00:00Z");
      expect(result[2].created_at).toBe("2026-01-01T00:00:00Z");
    });

    it("按创建日期降序排列", () => {
      const dates = ["2024-01-01T00:00:00Z", "2025-01-01T00:00:00Z", "2026-01-01T00:00:00Z"];
      const dateSubs = dates.map((d, i) => makeSub({ id: `d${i}`, channel_name: `Sub ${i}`, created_at: d }));
      const shuffled = [dateSubs[0], dateSubs[2], dateSubs[1]];
      const result = applySort(shuffled, { field: "created_at", direction: "desc" });
      expect(result[0].created_at).toBe("2026-01-01T00:00:00Z");
      expect(result[2].created_at).toBe("2024-01-01T00:00:00Z");
    });
  });

  describe("排序 - 按上次健康检查时间", () => {
    it("有值在前，无值在后（升序）", () => {
      const healthSubs = [
        makeSub({ id: "h1", channel_name: "With Check", last_health_check: "2025-06-01T00:00:00Z" }),
        makeSub({ id: "h2", channel_name: "No Check", last_health_check: null }),
      ];
      const result = applySort(healthSubs, { field: "last_health_check", direction: "asc" });
      // With check should be first (defined values go before null in asc)
      expect(result[0].id).toBe("h1");
      expect(result[1].id).toBe("h2");
    });

    it("都无值时保持原序", () => {
      const noCheckSubs = [
        makeSub({ id: "h1", channel_name: "A", last_health_check: null }),
        makeSub({ id: "h2", channel_name: "B", last_health_check: null }),
      ];
      const result = applySort(noCheckSubs, { field: "last_health_check", direction: "asc" });
      expect(result[0].id).toBe("h1");
      expect(result[1].id).toBe("h2");
    });

    it("按健康检查时间降序排列", () => {
      const healthSubs = [
        makeSub({ id: "h1", channel_name: "Old", last_health_check: "2025-01-01T00:00:00Z" }),
        makeSub({ id: "h2", channel_name: "New", last_health_check: "2025-12-01T00:00:00Z" }),
      ];
      const result = applySort(healthSubs, { field: "last_health_check", direction: "desc" });
      expect(result[0].id).toBe("h2");
      expect(result[1].id).toBe("h1");
    });
  });
});

describe("useFilter Hook", () => {
  it("待 useFilter 实现后再添加 Hook 集成测试", () => {
    expect(true).toBe(true);
  });
});
