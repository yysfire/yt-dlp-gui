import { describe, it, expect } from "vitest";
// `?raw` 由 `vite/client` 声明（见 src/vite-env.d.ts）——直接读入共享 fixture 的原文，
// 与 Rust 侧 `include_str!` 读的是**同一个物理文件**。
import fixtureRaw from "../../../src-tauri/tests/fixtures/download_record.json?raw";
import type { DownloadRecord, DownloadRecordStatus } from "@/types";

/**
 * Rust ↔ TS 的 `DownloadRecord` 契约基准（前端侧）。
 *
 * 两侧类型是**手写断言、无校验机制**（Rust 用 serde 派生结构，TS 用 interface），
 * 因此用**共享 JSON fixture + 两侧各一个测试**兜底：改 fixture ⇒ 两侧必有一侧变红。
 * Rust 侧对应测试见 `src-tauri/src/models/download.rs` 的
 * `test_download_record_matches_shared_fixture`。
 */

/** `DownloadRecordStatus` 的全部成员，用于运行时断言 fixture 的 status 合法。 */
const RECORD_STATUSES: readonly DownloadRecordStatus[] = [
  "downloading",
  "completed",
  "failed",
  "paused",
  "cancelled",
  "waiting",
  "deleted",
  "retrying",
];

describe("DownloadRecord 契约（Rust ↔ TS 共享 fixture）", () => {
  const parsed = JSON.parse(fixtureRaw) as DownloadRecord;

  /**
   * 编译期契约：与 fixture 同形的字面量必须满足 `DownloadRecord`。
   *
   * TS 类型一旦新增必填字段（本字面量缺字段 → 编译失败）或删除字段
   * （本字面量的多余属性 → `satisfies`/赋值报错），这里都会红。
   */
  const expected: DownloadRecord = {
    id: "0f0a9c1e-6b0a-4a7c-9f1b-2d3e4f5a6b7c",
    subscription_id: "1a2b3c4d-5e6f-4a8b-9c0d-1e2f3a4b5c6d",
    video_id: "dQw4w9WgXcQ",
    video_title: "示例视频",
    video_url: "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
    file_path: "/home/user/Videos/yt-dlp/示例视频.mp4",
    file_size: 123456789,
    status: "completed",
    error_message: null,
    downloaded_at: "2026-10-02T12:34:56.789+00:00",
    quality: "1080p",
    retry_count: 2,
    last_retry_at: "2026-10-02T12:30:00+00:00",
  };

  it("fixture 与 TS 契约字面量逐字段一致", () => {
    // `error_message` 为 None ⇒ 落库时该键**缺省**（serde `skip_serializing_if`），
    // 因而在比较期望里显式置 `undefined`：`toEqual` 会忽略值为 undefined 的键。
    expect(parsed).toEqual({ ...expected, error_message: undefined });
  });

  it("status 落在 DownloadRecordStatus 联合类型内", () => {
    expect(RECORD_STATUSES).toContain(parsed.status);
  });

  it("fixture 含 quality / retry_count / last_retry_at 三个新键", () => {
    expect(Object.keys(parsed)).toEqual(
      expect.arrayContaining(["quality", "retry_count", "last_retry_at"]),
    );
    expect(parsed.quality).toBe("1080p");
    expect(parsed.retry_count).toBe(2);
    expect(parsed.last_retry_at).toBe("2026-10-02T12:30:00+00:00");
  });

  it("fixture 缺省 error_message 键（None 不落库，而不是 null）", () => {
    expect("error_message" in parsed).toBe(false);
  });
});
