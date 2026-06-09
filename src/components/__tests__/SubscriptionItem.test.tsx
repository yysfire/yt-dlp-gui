import { describe, it, expect } from "vitest";
import React from "react";

/**
 * highlightText 函数 - 字面搜索高亮
 *
 * 功能：在文本中查找 keyword，将匹配部分用 <mark> 标签包裹，其余保持不变。
 * 规则：字面匹配（不触发正则），大小写不敏感，递归处理 JSX 节点。
 *
 * 注意：此函数将在 SubscriptionItem.tsx 中实现，这里测试其纯函数逻辑。
 */

/**
 * 纯函数版本：用于验证高亮逻辑正确性
 */
function highlightText(text: string, keyword: string): (string | React.JSX.Element)[] {
  if (!keyword || !text) {
    return [text];
  }

  const lowerText = text.toLowerCase();
  const lowerKw = keyword.toLowerCase();

  if (!lowerText.includes(lowerKw)) {
    return [text];
  }

  const escapedKw = keyword.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

  // Split by keyword, case-insensitive, preserving original case
  const regex = new RegExp(`(${escapedKw})`, "gi");
  const parts = text.split(regex);

  const result: (string | React.JSX.Element)[] = [];
  for (let i = 0; i < parts.length; i++) {
    if (parts[i] === "") continue;
    if (parts[i] && parts[i].toLowerCase() === lowerKw) {
      result.push(
        <mark key={i} className="bg-yellow-200 dark:bg-yellow-600">
          {parts[i]}
        </mark>,
      );
    } else {
      result.push(parts[i]);
    }
  }

  return result.length > 0 ? result : [text];
}

describe("highlightText 函数", () => {
  describe("基本高亮", () => {
    it("关键字出现在文本中间", () => {
      const result = highlightText("Hello World Test", "World");
      expect(result).toHaveLength(3);
      expect(result[0]).toBe("Hello ");
      // result[1] is a JSX element
      expect(result[2]).toBe(" Test");
    });

    it("关键字出现在文本开头", () => {
      const result = highlightText("World Hello", "World");
      expect(result).toHaveLength(2);
    });

    it("关键字出现在文本结尾", () => {
      const result = highlightText("Hello World", "World");
      expect(result).toHaveLength(2);
    });

    it("整个文本等于关键字", () => {
      const result = highlightText("World", "World");
      expect(result).toHaveLength(1);
    });
  });

  describe("空值和边界情况", () => {
    it("空关键字返回原文本", () => {
      const result = highlightText("Hello World", "");
      expect(result).toEqual(["Hello World"]);
    });

    it("空文本返回空文本", () => {
      const result = highlightText("", "test");
      expect(result).toEqual([""]);
    });

    it("关键字未匹配时返回原文本", () => {
      const result = highlightText("Hello World", "xyz");
      expect(result).toEqual(["Hello World"]);
    });
  });

  describe("大小写不敏感", () => {
    it("小写关键字匹配大写文本", () => {
      const result = highlightText("HELLO WORLD", "world");
      expect(result).toHaveLength(2);
    });

    it("大写关键字匹配小写文本", () => {
      const result = highlightText("hello world", "WORLD");
      expect(result).toHaveLength(2);
    });

    it("混合大小写匹配", () => {
      const result = highlightText("Hello World", "WoRlD");
      expect(result).toHaveLength(2);
    });
  });

  describe("正则特殊字符 - 字面匹配", () => {
    it("点号 . 当作普通字符", () => {
      const result = highlightText("file.txt", ".");
      // Should match the dot literally, not "any character"
      expect(result).toHaveLength(3);
    });

    it("星号 * 当作普通字符", () => {
      const result = highlightText("a * b", "*");
      expect(result).toHaveLength(3);
    });

    it("加号 + 当作普通字符", () => {
      const result = highlightText("a + b", "+");
      expect(result).toHaveLength(3);
    });

    it("问号 ? 当作普通字符", () => {
      const result = highlightText("what?", "?");
      expect(result).toHaveLength(2);
    });

    it("括号 () 当作普通字符", () => {
      const result = highlightText("test (abc)", "(abc)");
      expect(result).toHaveLength(2);
    });

    it("方括号 [] 当作普通字符", () => {
      const result = highlightText("test [abc]", "[abc]");
      expect(result).toHaveLength(2);
    });

    it("美元符号 $ 当作普通字符", () => {
      const result = highlightText("price $100", "$100");
      expect(result).toHaveLength(2);
    });

    it("脱字符 ^ 当作普通字符", () => {
      const result = highlightText("a^2", "^");
      expect(result).toHaveLength(3);
    });

    it("反斜杠 当作普通字符", () => {
      const result = highlightText("path\\to\\file", "\\");
      expect(result).toHaveLength(5);
    });

    it("管道符 | 当作普通字符", () => {
      const result = highlightText("a|b", "|");
      expect(result).toHaveLength(3);
    });
  });

  describe("多关键字", () => {
    it("多个相同关键字在文本中出现多次", () => {
      const result = highlightText("test test test", "test");
      expect(result).toHaveLength(5);
    });

    it("重叠关键字不会导致问题", () => {
      const result = highlightText("aaa", "aa");
      // split produces ["", "aa", "a"], after filtering empty strings: <mark>aa</mark>, "a"
      expect(result).toHaveLength(2);
    });
  });

  describe("中文字符", () => {
    it("中文关键字高亮", () => {
      const result = highlightText("这是一个测试频道", "测试");
      expect(result).toHaveLength(3);
      expect(result[0]).toBe("这是一个");
      expect(result[2]).toBe("频道");
    });

    it("中文多关键字", () => {
      const result = highlightText("B站 科技频道", "B站");
      expect(result).toHaveLength(2);
    });
  });
});
