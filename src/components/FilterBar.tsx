import { useState, useEffect, useRef } from "react";
import {
  Box,
  TextField,
  ToggleButton,
  ToggleButtonGroup,
  FormControl,
  Select,
  MenuItem,
  IconButton,
  Tooltip,
  InputAdornment,
  Typography,
} from "@mui/material";
import {
  Search as SearchIcon,
  Clear as ClearIcon,
  ArrowUpward as AscIcon,
  ArrowDownward as DescIcon,
} from "@mui/icons-material";
import type { FilterState, SortState, Subscription } from "../types";
import { GROUPS } from "../types";

interface FilterBarProps {
  filter: FilterState;
  onFilterChange: (partial: Partial<FilterState>) => void;
  sort: SortState;
  onSortChange: (sort: SortState) => void;
  subscriptions: Subscription[];
  count: number;
}

const ALL_GROUPS = ["全部", "未分组", ...GROUPS.slice(1)]; // 全部 + 未分组 + 学习/娱乐/音乐/科技/其他

export default function FilterBar({
  filter,
  onFilterChange,
  sort,
  onSortChange,
  count,
}: FilterBarProps) {
  const [keyword, setKeyword] = useState(filter.keyword);
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Debounce keyword input
  useEffect(() => {
    if (debounceRef.current) {
      clearTimeout(debounceRef.current);
    }
    debounceRef.current = setTimeout(() => {
      if (keyword !== filter.keyword) {
        onFilterChange({ keyword });
      }
    }, 200);
    return () => {
      if (debounceRef.current) {
        clearTimeout(debounceRef.current);
      }
    };
  }, [keyword, filter.keyword, onFilterChange]);

  // Sync external keyword reset
  useEffect(() => {
    if (filter.keyword === "") {
      setKeyword("");
    }
  }, [filter.keyword]);

  const handleClearKeyword = () => {
    setKeyword("");
    onFilterChange({ keyword: "" });
  };

  const handleSortFieldChange = (field: SortState["field"]) => {
    onSortChange({ ...sort, field });
  };

  const handleSortDirectionToggle = () => {
    onSortChange({ ...sort, direction: sort.direction === "asc" ? "desc" : "asc" });
  };

  return (
    <Box
      sx={{
        display: "flex",
        flexWrap: "wrap",
        alignItems: "center",
        gap: 0.75,
        px: 1,
        py: 0.75,
        borderBottom: 1,
        borderColor: "divider",
      }}
    >
      {/* 关键字搜索 */}
      <TextField
        size="small"
        value={keyword}
        onChange={(e) => setKeyword(e.target.value)}
        placeholder="搜索..."
        sx={{ width: 130, "& .MuiInputBase-root": { fontSize: "0.75rem" } }}
        InputProps={{
          startAdornment: (
            <InputAdornment position="start">
              <SearchIcon sx={{ fontSize: 16 }} />
            </InputAdornment>
          ),
          endAdornment: keyword ? (
            <InputAdornment position="end">
              <IconButton size="small" onClick={handleClearKeyword} sx={{ p: 0 }}>
                <ClearIcon sx={{ fontSize: 14 }} />
              </IconButton>
            </InputAdornment>
          ) : undefined,
        }}
      />

      {/* 平台筛选 */}
      <FormControl size="small" sx={{ minWidth: 72 }}>
        <Select
          value={filter.platform}
          onChange={(e) => onFilterChange({ platform: e.target.value as FilterState["platform"] })}
          displayEmpty
          sx={{ fontSize: "0.7rem" }}
        >
          <MenuItem value="all" dense sx={{ fontSize: "0.7rem" }}>
            全部平台
          </MenuItem>
          <MenuItem value="youtube" dense sx={{ fontSize: "0.7rem" }}>
            YouTube
          </MenuItem>
          <MenuItem value="bilibili" dense sx={{ fontSize: "0.7rem" }}>
            Bilibili
          </MenuItem>
        </Select>
      </FormControl>

      {/* 状态筛选 */}
      <ToggleButtonGroup
        value={filter.status}
        exclusive
        onChange={(_, v) => v && onFilterChange({ status: v })}
        size="small"
        sx={{ "& .MuiToggleButton-root": { px: 1, py: 0.25, fontSize: "0.65rem", textTransform: "none" } }}
      >
        <ToggleButton value="all">全部</ToggleButton>
        <ToggleButton value="active">启用</ToggleButton>
        <ToggleButton value="paused">暂停</ToggleButton>
      </ToggleButtonGroup>

      {/* 分组筛选 */}
      <FormControl size="small" sx={{ minWidth: 64 }}>
        <Select
          value={filter.group}
          onChange={(e) => onFilterChange({ group: e.target.value })}
          displayEmpty
          sx={{ fontSize: "0.7rem" }}
        >
          {ALL_GROUPS.map((g) => (
            <MenuItem key={g} value={g} dense sx={{ fontSize: "0.7rem" }}>
              {g}
            </MenuItem>
          ))}
        </Select>
      </FormControl>

      {/* 健康状态筛选 */}
      <ToggleButtonGroup
        value={filter.health}
        exclusive
        onChange={(_, v) => v && onFilterChange({ health: v })}
        size="small"
        sx={{ "& .MuiToggleButton-root": { px: 1, py: 0.25, fontSize: "0.65rem", textTransform: "none" } }}
      >
        <ToggleButton value="all">健康</ToggleButton>
        <ToggleButton value="ok" sx={{ color: "success.main" }}>正常</ToggleButton>
        <ToggleButton value="warning" sx={{ color: "warning.main" }}>警告</ToggleButton>
        <ToggleButton value="dead" sx={{ color: "error.main" }}>失效</ToggleButton>
        <ToggleButton value="unchecked">未检查</ToggleButton>
      </ToggleButtonGroup>

      {/* 排序 */}
      <Box sx={{ display: "flex", alignItems: "center", gap: 0.25, ml: "auto" }}>
        <FormControl size="small" sx={{ minWidth: 80 }}>
          <Select
            value={sort.field}
            onChange={(e) => handleSortFieldChange(e.target.value as SortState["field"])}
            displayEmpty
            sx={{ fontSize: "0.7rem" }}
          >
            <MenuItem value="name" dense sx={{ fontSize: "0.7rem" }}>
              按名称
            </MenuItem>
            <MenuItem value="created_at" dense sx={{ fontSize: "0.7rem" }}>
              按添加日期
            </MenuItem>
            <MenuItem value="last_health_check" dense sx={{ fontSize: "0.7rem" }}>
              按健康检查
            </MenuItem>
          </Select>
        </FormControl>
        <Tooltip title={sort.direction === "asc" ? "升序" : "降序"}>
          <IconButton size="small" onClick={handleSortDirectionToggle}>
            {sort.direction === "asc" ? <AscIcon fontSize="small" /> : <DescIcon fontSize="small" />}
          </IconButton>
        </Tooltip>
        <Typography variant="caption" color="text.secondary" sx={{ ml: 0.5, minWidth: 40, textAlign: "right" }}>
          {count}
        </Typography>
      </Box>
    </Box>
  );
}
