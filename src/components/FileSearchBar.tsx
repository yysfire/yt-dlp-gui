import { TextField, InputAdornment } from "@mui/material";
import { Search as SearchIcon } from "@mui/icons-material";

interface FileSearchBarProps {
  value: string;
  onChange: (value: string) => void;
}

/**
 * Search input for filtering downloaded videos by title.
 * Updates parent state on every keystroke for real-time filtering.
 */
export default function FileSearchBar({ value, onChange }: FileSearchBarProps) {
  return (
    <TextField
      fullWidth
      size="small"
      placeholder="搜索已下载视频..."
      value={value}
      onChange={(e) => onChange(e.target.value)}
      InputProps={{
        startAdornment: (
          <InputAdornment position="start">
            <SearchIcon fontSize="small" />
          </InputAdornment>
        ),
      }}
      sx={{ mb: 1.5, "& .MuiOutlinedInput-root": { borderRadius: 1 } }}
    />
  );
}
