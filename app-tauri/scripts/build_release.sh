#!/usr/bin/env bash
# ===============================================================
# Aura Trade macOS 一键发布构建
#
# 流程：
#   1. cargo tauri build --bundles app   （只出 .app，不做 DMG）
#   2. codesign --sign -                 （ad-hoc 签名）
#   3. 校验签名
#   4. hdiutil create                    （用已签名的 .app 打 DMG）
#   5. xattr 去掉 quarantine 位（本机直接打开无警告）
#   6. 把产物拷贝到 ~/Desktop/
#
# 输出：
#   ~/Desktop/Aura Trade.app          ← 双击即用
#   ~/Desktop/Aura Trade 0.1.0.dmg    ← 分发给朋友
#
# 若想用 Apple Developer 证书：
#   export AURA_SIGN_ID="Developer ID Application: Your Name (XXXXXXXXXX)"
#   然后运行本脚本
# ===============================================================
set -euo pipefail

cd "$(dirname "$0")/../.."   # → 项目根
ROOT="$(pwd)"
APP_SRC="$ROOT/target/release/bundle/macos/Aura Trade.app"
APP_NAME="Aura Trade"
VERSION=$(grep -m1 '^version' app-tauri/Cargo.toml | sed -E 's/.*"(.+)".*/\1/')
DMG_NAME="${APP_NAME} ${VERSION}.dmg"
DESK="$HOME/Desktop"

# 签名身份：若环境变量存在用正式证书，否则 ad-hoc
SIGN_ID="${AURA_SIGN_ID:--}"

echo "▶ 1/5 构建 .app..."
cd app-tauri
cargo tauri build --bundles app
cd "$ROOT"
[ -d "$APP_SRC" ] || { echo "❌ build 未生成 .app: $APP_SRC"; exit 1; }

echo "▶ 2/5 代码签名 (sign id = '$SIGN_ID')..."
codesign --force --deep --sign "$SIGN_ID" "$APP_SRC"

echo "▶ 3/5 校验签名..."
codesign --verify --deep --strict --verbose=2 "$APP_SRC" 2>&1 | tail -3

echo "▶ 4/5 构建 DMG..."
DMG_TMP="$ROOT/target/release/bundle/dmg"
mkdir -p "$DMG_TMP"
STAGING="$(mktemp -d -t aura_dmg)"
# staging 里放 .app + Applications 软链接
cp -R "$APP_SRC" "$STAGING/"
ln -s /Applications "$STAGING/Applications"
DMG_OUT="$DMG_TMP/${DMG_NAME}"
rm -f "$DMG_OUT"
hdiutil create -volname "$APP_NAME" -srcfolder "$STAGING" \
    -ov -format UDZO "$DMG_OUT" > /dev/null
rm -rf "$STAGING"

# DMG 也签（可选但推荐，避免 Gatekeeper 首次打开多一层确认）
codesign --force --sign "$SIGN_ID" "$DMG_OUT" 2>/dev/null || true

echo "▶ 5/5 去 quarantine + 拷贝到 ~/Desktop/"
# 清除 extended attributes（去掉 com.apple.quarantine 让本机直接双击无警告）
xattr -cr "$APP_SRC"

# 拷贝 .app 到桌面（直接可双击）
rm -rf "$DESK/${APP_NAME}.app"
cp -R "$APP_SRC" "$DESK/"
# 拷贝 DMG 到桌面（用于分发）
rm -f "$DESK/${DMG_NAME}"
cp "$DMG_OUT" "$DESK/"

APP_SIZE=$(du -sh "$DESK/${APP_NAME}.app" | cut -f1)
DMG_SIZE=$(du -sh "$DESK/${DMG_NAME}" | cut -f1)

echo ""
echo "✅ 构建完成！"
echo "   📱 $DESK/${APP_NAME}.app    ($APP_SIZE)"
echo "   💿 $DESK/${DMG_NAME}  ($DMG_SIZE)"
echo ""
echo "双击 .app 即可运行。分发请用 DMG。"
echo ""
if [ "$SIGN_ID" = "-" ]; then
  echo "⚠️  当前为 ad-hoc 签名（仅本机免警告）。分发给朋友时他们首次打开需：右键 → 打开"
  echo "    如果要做正式分发，设置 AURA_SIGN_ID 环境变量为开发者证书名后重跑本脚本。"
fi
