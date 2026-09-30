#!/usr/bin/env python3
"""从 Firefox 导出 YouTube 认证 Cookie（Netscape 格式），供本应用的 yt-dlp 使用。

为什么需要
    YouTube 会对自动化请求返回 "Sign in to confirm you're not a bot"。本应用通过
    `--cookies <文件>` 携带认证（见 src-tauri/src/services/ytdlp.rs），因此需要一个
    Netscape 格式的 Cookie 文件，并在「设置 → Cookie 文件」里指定其路径。

为什么只支持 Firefox
    Chrome / Chromium / Edge 在 Linux 与 macOS 上把 Cookie 值用系统密钥环
    （libsecret / kwallet / Keychain）加密，纯标准库无法解密；Firefox 将 Cookie
    明文存放于 profile 目录的 cookies.sqlite，可离线读取，无需启动浏览器。

用法
    python3 scripts/export-yt-cookies.py
    python3 scripts/export-yt-cookies.py --profile ~/.mozilla/firefox/xxxx.default
    python3 scripts/export-yt-cookies.py --out ~/my-cookies.txt

安全提示
    输出文件等同于账号登录凭证（明文）。脚本强制权限 0600，默认写到用户主目录
    而非仓库内。请勿提交到 git、分享或放入云同步目录。
"""

import argparse
import configparser
import os
import shutil
import sqlite3
import sys
import tempfile
from http.cookiejar import Cookie, MozillaCookieJar

# 只保留 YouTube 认证所需的域，避免把浏览器里其它站点（GitHub、网盘、公司系统等）
# 的登录凭证一并导出，从而缩小凭证暴露面。
ALLOWED_HOSTS = (".youtube.com", ".google.com", "accounts.google.com", "gds.google.com")

# 判定「已登录 YouTube」的关键 Cookie：缺少则很可能没登录，导出的文件无法通过校验。
LOGIN_MARKER_HOSTS = (".youtube.com",)
LOGIN_MARKER_NAMES = ("LOGIN_INFO", "SID", "SSID", "HSID")


def candidate_roots():
    """返回候选的 Firefox 配置根目录（存在者优先，但保留顺序用于报错提示）。"""
    raw = [
        "~/.mozilla/firefox",  # Linux 原生包
        "~/snap/firefox/common/.mozilla/firefox",  # Linux snap
        "~/.var/app/org.mozilla.firefox/.mozilla/firefox",  # Linux flatpak
        "~/Library/Application Support/Firefox",  # macOS
    ]
    appdata = os.environ.get("APPDATA")
    if appdata:
        raw.append(os.path.join(appdata, "Mozilla", "Firefox"))  # Windows
    return [os.path.expanduser(p) for p in raw]


def _ini_value(cp, ini_dir, section, key):
    """读取 ini 段中的路径值；相对路径按 ini 所在目录解析。"""
    value = cp.get(section, key, fallback=None)
    if not value:
        return None
    if os.path.isabs(value):
        return os.path.normpath(value)
    return os.path.normpath(os.path.join(ini_dir, value))


def _scan_profiles(ini_dir):
    """扫描含 cookies.sqlite 的子目录，按 cookies.sqlite 修改时间从新到旧返回。"""
    found = []
    try:
        for name in os.listdir(ini_dir):
            profile = os.path.join(ini_dir, name)
            db = os.path.join(profile, "cookies.sqlite")
            if os.path.isfile(db):
                found.append((os.path.getmtime(db), profile))
    except OSError:
        return []
    return [profile for _, profile in sorted(found, reverse=True)]


def resolve_default_profile(ini_dir):
    """按 Firefox 自身规则解析默认 profile，失败则退回目录扫描。"""
    ini_path = os.path.join(ini_dir, "profiles.ini")
    if os.path.isfile(ini_path):
        cp = configparser.ConfigParser()
        cp.read(ini_path, encoding="utf-8")
        # [Install*] 段的 Default 记录 Firefox 最后使用的 profile，最准确。
        for section in cp.sections():
            if section.lower().startswith("install") and cp.has_option(section, "default"):
                path = _ini_value(cp, ini_dir, section, "default")
                if path and os.path.isfile(os.path.join(path, "cookies.sqlite")):
                    return path
        # 其次取标记 Default=1 的 profile。
        for section in cp.sections():
            if section.lower().startswith("profile") and cp.get(section, "default", fallback="0") == "1":
                path = _ini_value(cp, ini_dir, section, "path")
                if path and os.path.isfile(os.path.join(path, "cookies.sqlite")):
                    return path
    # 兜底：最近使用过的 profile。
    scanned = _scan_profiles(ini_dir)
    return scanned[0] if scanned else None


def find_profile(explicit):
    """确定要使用的 profile 目录；explicit 可以是 profile 目录或含 profiles.ini 的目录。"""
    if explicit:
        target = os.path.expanduser(explicit)
        if not os.path.isdir(target):
            sys.exit(f"错误：路径不存在或不是目录：{target}")
        if os.path.isfile(os.path.join(target, "cookies.sqlite")):
            return target
        resolved = resolve_default_profile(target)
        if resolved:
            return resolved
        sys.exit(f"错误：{target} 下未找到 cookies.sqlite")

    roots = candidate_roots()
    for ini_dir in roots:
        if os.path.isdir(ini_dir):
            resolved = resolve_default_profile(ini_dir)
            if resolved:
                return resolved
    sys.exit(
        "错误：未找到 Firefox 的 profile。请确认已安装 Firefox 并在其中登录过 YouTube，"
        "或使用 --profile 手动指定 profile 目录。\n已检查：\n  " + "\n  ".join(roots)
    )


def load_cookies(profile_dir):
    """从 cookies.sqlite 读取全部 Cookie。Firefox 运行中也可安全读取，仍先复制副本避免锁竞争。"""
    src = os.path.join(profile_dir, "cookies.sqlite")
    if not os.path.isfile(src):
        sys.exit(f"错误：找不到 {src}")

    fd, tmp = tempfile.mkstemp(suffix=".sqlite")
    os.close(fd)
    try:
        shutil.copy2(src, tmp)
        con = sqlite3.connect(tmp)
        try:
            return con.execute(
                "SELECT host, name, value, path, expiry, isSecure, isHttpOnly FROM moz_cookies"
            ).fetchall()
        finally:
            con.close()
    finally:
        os.remove(tmp)


def write_netscape(rows, out_path):
    """写出 Netscape 格式 Cookie 文件，返回 (保留条数, 命中的域计数)。"""
    jar = MozillaCookieJar(out_path)
    for host, name, value, path, expiry, secure, httponly in rows:
        if host not in ALLOWED_HOSTS:
            continue
        jar.set_cookie(
            Cookie(
                version=0,
                name=name,
                value=value,
                port=None,
                port_specified=False,
                domain=host,
                domain_specified=bool(host),
                domain_initial_dot=host.startswith("."),
                path=path or "/",
                path_specified=True,
                secure=bool(secure),
                expires=int(expiry) if expiry else None,
                discard=not expiry,
                comment=None,
                comment_url=None,
                rest={"HttpOnly": None} if httponly else {},
                rfc2109=False,
            )
        )
    # 按 jar 实际内容统计：set_cookie 会对相同 (domain, path, name) 去重，
    # 直接累加扫描行会让明细之和与总数不符。
    domains = {}
    for cookie in jar:
        domains[cookie.domain] = domains.get(cookie.domain, 0) + 1
    # ignore_discard/ignore_expires：会话与已过期 Cookie 也写出，由 yt-dlp 决定是否采用。
    jar.save(ignore_discard=True, ignore_expires=True)
    os.chmod(out_path, 0o600)
    return len(jar), domains


def check_logged_in(rows):
    """返回 YouTube 登录态的关键 Cookie 名（缺失即视为未登录）。"""
    present = {name for host, name, *_ in rows if host in LOGIN_MARKER_HOSTS}
    return [n for n in LOGIN_MARKER_NAMES if n in present]


def main():
    parser = argparse.ArgumentParser(
        description="从 Firefox 导出 YouTube 认证 Cookie（Netscape 格式），供本应用的 yt-dlp 使用。",
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    parser.add_argument(
        "--profile",
        help="Firefox profile 目录，或含 profiles.ini 的目录（默认自动探测）",
    )
    parser.add_argument(
        "--out",
        default="~/youtube-cookies.txt",
        help="输出文件路径（默认 ~/youtube-cookies.txt）",
    )
    args = parser.parse_args()

    profile = find_profile(args.profile)
    out_path = os.path.expanduser(args.out)

    rows = load_cookies(profile)
    markers = check_logged_in(rows)

    # 登录态校验先于写文件：否则未登录时会用空文件覆盖掉用户原有的有效 Cookie。
    if not markers:
        print(f"profile : {profile}")
        print(f"扫描    : {len(rows)} 条 Cookie")
        print()
        print("错误：未检测到 YouTube 登录态 Cookie（LOGIN_INFO / SID / SSID / HSID）。", file=sys.stderr)
        print("      请先在 Firefox 中登录 YouTube 再重新运行本脚本。", file=sys.stderr)
        print(f"      已中止，未写出 {out_path}。", file=sys.stderr)
        return 1

    kept, domains = write_netscape(rows, out_path)

    print(f"profile : {profile}")
    print(f"扫描    : {len(rows)} 条 Cookie")
    print(f"保留    : {kept} 条（仅 YouTube / Google 域）")
    for host in sorted(domains):
        print(f"          {host}: {domains[host]}")
    print(f"输出    : {out_path}（权限 0600）")
    print(f"登录态  : 命中 {', '.join(markers)}")
    print()
    print("下一步：在应用「设置 → Cookie 文件」中选择该文件，然后重试。")
    print("注意：Cookie 会过期，再次出现 “Sign in to confirm you're not a bot” 时需重新导出。")
    return 0


if __name__ == "__main__":
    sys.exit(main())
