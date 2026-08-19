# TheIsle Overlay v2

Bản đồ hiện đè lên màn hình khi chơi **The Isle: Evrima** (map Gateway).
Song ngữ Việt/Anh · minimap tròn bám theo cửa sổ game · bản đồ lớn với POI,
waypoint, vết đường đi · cài một lần, tự cập nhật.

Map overlay for **The Isle: Evrima** (Gateway). Bilingual VI/EN · circular
minimap pinned to the game window · full map with POIs, waypoints and travel
trails · one-click install with auto-update.

## Cài đặt / Install

Tải file `TheIsle Overlay_x.x.x_x64-setup.exe` từ
[Releases](https://github.com/toantranct/theisle-overlay/releases) và chạy.
Lần đầu mở app sẽ tải dữ liệu bản đồ (~3 MB) về máy.

> Windows có thể hiện cảnh báo SmartScreen vì installer chưa ký số.
> Bấm **More info → Run anyway**.

## An toàn với anti-cheat / Anti-cheat safety

Game chạy Easy Anti-Cheat cấp kernel. App này an toàn vì **không bao giờ đụng
vào tiến trình game**:

- Vị trí chỉ lấy từ **clipboard** khi bạn tự bấm Tab → "Asset Location" trong
  game — app chỉ đọc lại thứ game tự đưa ra.
- Phím tắt dùng `RegisterHotKey` (API hợp tác của Windows), **không phải**
  keyboard hook.
- Không bao giờ: đọc bộ nhớ game, inject DLL, hook DirectX, giả lập phím,
  bắt gói mạng, tự chép tọa độ theo timer, chia sẻ vị trí giữa người chơi.

CI có bước grep chặn mọi call site API cấm (`scripts/check-forbidden-apis.ps1`).
Danh sách API được phép nằm ở đầu `src-tauri/src/win/mod.rs`.

## Phát triển / Development

Yêu cầu: Node 22+, Rust stable (MSVC), WebView2.

```powershell
npm install
npx tauri dev                        # chạy dev

# Lái UI không cần mở game:
$env:THEISLE_REPLAY = "path\to\replay_sample.txt"; npx tauri dev

# Test
npm run check                        # svelte-check
cd src-tauri; cargo test --workspace # toàn bộ test Rust
cargo clippy --workspace -- -D warnings
..\scripts\check-forbidden-apis.ps1

# Sau mỗi lần map game cập nhật:
cargo run --bin verify_data --features devtools
cargo test -p theisle-overlay --lib -- --ignored parse_real_cache
```

Lưu ý: `.cargo/config.toml` đặt `target-dir` ra ngoài thư mục OneDrive.

## Phát hành / Release

1. Thêm secrets vào GitHub repo: `TAURI_SIGNING_PRIVATE_KEY` (nội dung
   `~/.tauri/theisle-overlay.key`) và `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`
   (mật khẩu của khóa đó — giữ ngoài repo).
2. Tăng `version` trong `src-tauri/tauri.conf.json` và `package.json`.
3. `git tag v2.x.x && git push --tags` — workflow `release.yml` build NSIS,
   ký update artifact, sinh `latest.json` và tạo GitHub Release.
4. App đang chạy sẽ tự thấy bản mới và mời cập nhật.

Nếu đổi tên repo/chủ sở hữu, sửa `plugins.updater.endpoints` trong
`src-tauri/tauri.conf.json`.

## Kiến trúc / Architecture

- `src-tauri/crates/overlay-core` — logic thuần (parse tọa độ, transform
  world↔pixel, tracker) + toàn bộ test suite port từ bản Python. Frontend
  **không bao giờ** tự tính transform; mọi payload mang sẵn cả cm lẫn pixel.
- `src-tauri/src` — Win32 (ranh giới an toàn trong `win/`), clipboard watcher,
  hotkeys, settings/store (giữ nguyên đường dẫn + format của bản Python cũ —
  người dùng cũ không mất dữ liệu), fetch dữ liệu, quản lý cửa sổ minimap.
- `src/main` — cửa sổ chính (Svelte 5 + Tailwind + Leaflet CRS.Simple).
- `src/minimap` — entry riêng, canvas thuần, không framework: webview chạy
  cạnh game hàng giờ phải tối giản, chỉ vẽ khi có sự kiện (0% CPU idle).

Dữ liệu bản đồ **tải khi chạy lần đầu, không đóng gói** — basemap thuộc
VulnonaMAP (phái sinh từ tài sản game của Afterthought LLC); bản sao cá nhân
trên máy người dùng khác với việc app tái phân phối dữ liệu đó.

## Nguồn dữ liệu / Credits

- Basemap: [VulnonaMAP](https://vulnona.com/game/map/) (Coco.N) — ghép từ ảnh
  chụp trong game. Bản quyền hình ảnh: Afterthought LLC (The Isle).
- POI: [myislemap.com](https://myislemap.com/), VulnonaMAP, hướng dẫn Steam
  của wiredredman.

Không liên kết với Afterthought LLC. / Unaffiliated with Afterthought LLC.
