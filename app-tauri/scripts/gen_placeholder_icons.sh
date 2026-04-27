#!/usr/bin/env bash
# 用 macOS 自带的 sips 命令从系统 App 里拷贝图标作为占位。
# 日后替换为正式 logo：cargo tauri icon path/to/your-logo.png
set -e

cd "$(dirname "$0")/.."
mkdir -p icons

# 候选源图（按优先级）：Stocks → Calculator → Launchpad
CANDIDATES=(
  "/System/Applications/Stocks.app/Contents/Resources/AppIcon.icns"
  "/System/Applications/Calculator.app/Contents/Resources/AppIcon.icns"
  "/System/Applications/Launchpad.app/Contents/Resources/AppIcon.icns"
)

SRC=""
for c in "${CANDIDATES[@]}"; do
  if [ -f "$c" ]; then SRC="$c"; break; fi
done

if [ -z "$SRC" ]; then
  echo "❌ 找不到可用的系统 AppIcon.icns，请手动放一张 1024x1024 PNG 到 app-tauri/icons/source.png 后运行 cargo tauri icon"
  exit 1
fi

echo "📦 从 $SRC 生成占位图标"
cp "$SRC" icons/icon.icns

TMP=$(mktemp -t aura_icon)
sips -s format png "$SRC" --out "$TMP.png" > /dev/null

sips -z 32  32  "$TMP.png" --out icons/32x32.png           > /dev/null
sips -z 128 128 "$TMP.png" --out icons/128x128.png         > /dev/null
sips -z 256 256 "$TMP.png" --out "icons/128x128@2x.png"    > /dev/null

rm -f "$TMP.png"

echo "✅ 占位图标已生成到 app-tauri/icons/"
echo "   日后替换：cargo tauri icon path/to/your-logo.png"
