# TheIsle Overlay v2

**Tiếng Việt** · [English](README.en.md)

Bản đồ hiện đè lên màn hình khi chơi **The Isle: Evrima** (map Gateway).
Minimap tròn bám theo cửa sổ game · bản đồ lớn với POI, tên địa danh, waypoint,
vết đường đi · xem chỉ số khủng long của bạn từ panel IslePilot · giao diện
song ngữ Việt/Anh · cài một lần, tự cập nhật.

## Tính năng

- **Minimap tròn** bám góc cửa sổ game, chuột bấm xuyên qua, không cản trở lúc chơi.
  Hướng Bắc luôn ở trên, có mũi tên chỉ hướng đang đi.
- **Bản đồ lớn**: phóng to/thu nhỏ mượt, 9 lớp bật/tắt được (nguồn nước, mỏ muối,
  vũng bùn, khu bảo tồn, vùng di cư, vùng tuần tra AI, khu thức ăn, tên vùng, địa điểm),
  tên địa danh hiện trực tiếp trên bản đồ.
- **Điểm đánh dấu (waypoint)**: bấm chuột phải để cắm, đổi tên, xóa; hiện hướng và
  khoảng cách tới điểm gần nhất.
- **Đường đã đi**: tự ghi theo phiên, khôi phục lại đường đi của phiên trước.
- **Khủng long của bạn**: growth, máu, đói, khát và Prime progress đọc từ panel
  IslePilot của server, có thanh chỉ số gọn ngay dưới minimap.
- **Phím tắt toàn cục** đổi được trong app, song ngữ Việt/Anh, tự cập nhật phiên bản mới.

## Cài đặt

Tải file `TheIsle Overlay_x.x.x_x64-setup.exe` từ
[Releases](https://github.com/toantranct/theisle-overlay/releases) và chạy.
Lần đầu mở app sẽ tải dữ liệu bản đồ (~3 MB) về máy.

Yêu cầu: **Windows 10/11 64-bit**. WebView2 (thường đã có sẵn trên Windows 11;
nếu thiếu, installer tự tải về).

> Windows có thể hiện cảnh báo SmartScreen vì installer chưa ký số.
> Bấm **More info → Run anyway**.

## Nhẹ cỡ nào?

Số đo thật trên máy: **Intel Core i5-14400F (10 nhân/16 luồng), 32 GB RAM,
RTX 3060 Ti, Windows 11 Pro build 26200, độ phân giải 100%** — bản release v2.1.0:

| Hạng mục | Dung lượng |
|---|---|
| File cài đặt | **4,3 MB** |
| File chạy sau khi cài | 17,8 MB |
| Dữ liệu bản đồ tải lần đầu | 2,9 MB (ảnh nền 2,6 MB + dữ liệu điểm 0,3 MB) |
| **Tổng chiếm ổ cứng** | **~21 MB** |

| Lúc chạy | Số đo |
|---|---|
| RAM (mở cả bản đồ lớn + minimap) | ~528 MB trên 8 tiến trình |
| RAM riêng tiến trình app | 50 MB |
| CPU lúc rảnh (không thao tác) | **0,3%** (≈5% của một luồng trong 16 luồng) |

Vì sao RAM như vậy: giao diện chạy trên WebView2 (Edge) nên có nhiều tiến trình
phụ, và ảnh bản đồ 3900×3908 px khi giải nén chiếm sẵn ~60 MB. Con số trên là
**trường hợp nặng nhất** — lúc chơi game bạn thường ẩn bản đồ lớn (`Ctrl+Alt+F`)
và chỉ để minimap, khi đó Windows thu hồi bớt bộ nhớ của cửa sổ đã ẩn.

App **không có vòng lặp vẽ lại**: chỉ vẽ khi có dữ liệu mới, nên gần như không
tốn CPU khi bạn đang chơi.

## Lưu ý cần biết

1. **Chế độ hình ảnh của game**: overlay không thể hiện đè lên **Toàn màn hình
   độc quyền (Exclusive Fullscreen)** — đây là giới hạn của Windows với mọi
   overlay ngoài tiến trình. Hãy dùng **Cửa sổ** hoặc **Toàn màn hình không viền**.
   App tự đọc cấu hình game và cảnh báo nếu bạn đang để sai chế độ.
2. **Vị trí không tự động cập nhật**: bạn phải tự bấm `Tab` → **Asset Location**
   trong game mỗi khi muốn cập nhật vị trí. Đây là *chủ đích*, không phải thiếu sót
   — xem mục an toàn anti-cheat bên dưới.
3. **Hướng đi cần hai lần chép tọa độ** cách nhau ít nhất 20 m; mẫu cũ quá 10 phút
   thì hướng hết hạn để tránh chỉ sai.
4. **Không mở được hai bản cùng lúc** — phím tắt toàn cục mang tính độc quyền,
   hai bản chạy song song sẽ tranh nhau.
5. **Phím tắt bị ứng dụng khác chiếm**: app báo ngay khi khởi động, đổi lại trong
   tab Cài đặt.
6. **Tính năng "Khủng long của bạn"** chỉ hỗ trợ server dùng nền tảng IslePilot
   (`xxx.islepilot.eu`). Nó đọc dữ liệu bằng cách phân tích HTML trang web của
   server (không có API chính thức), nên **có thể hỏng khi IslePilot đổi giao diện**
   — app sẽ báo khi phát hiện server vừa cập nhật. Nếu phần này lỗi, các tính năng
   bản đồ **không bị ảnh hưởng**.
7. **Nên hỏi admin server** trước khi dùng thường xuyên — một số server có luật
   riêng về công cụ bên thứ ba. Tùy chọn lấy vị trí tự động từ live map mặc định TẮT.
8. **Cookie đăng nhập panel** được mã hóa bằng Windows DPAPI, chỉ giải được bằng
   tài khoản Windows của bạn trên chính máy đó.
9. **SmartScreen** sẽ cảnh báo ở lần cài đầu vì installer chưa ký số (chứng chỉ ký
   số tốn phí hằng năm). Bản cập nhật tự động về sau không bị hỏi lại.

## An toàn với anti-cheat

Game chạy Easy Anti-Cheat cấp kernel. App này an toàn vì **không bao giờ đụng
vào tiến trình game**:

- Vị trí chỉ lấy từ **clipboard** khi bạn tự bấm Tab → "Asset Location" trong
  game — app chỉ đọc lại thứ game tự đưa ra.
- Phím tắt dùng `RegisterHotKey` (API hợp tác của Windows), **không phải**
  keyboard hook.
- Chỉ số khủng long lấy qua **HTTPS tới website của chính server** (panel
  IslePilot) — cũng không liên quan gì tới tiến trình game.
- Không bao giờ: đọc bộ nhớ game, inject DLL, hook DirectX, giả lập phím,
  bắt gói mạng, tự chép tọa độ theo timer, chia sẻ vị trí giữa người chơi.

CI có bước grep chặn mọi call site API cấm (`scripts/check-forbidden-apis.ps1`).
Danh sách API được phép nằm ở đầu `src-tauri/src/win/mod.rs`.

## Phát triển

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

## Phát hành

1. Thêm secrets vào GitHub repo: `TAURI_SIGNING_PRIVATE_KEY` (nội dung
   `~/.tauri/theisle-overlay.key`) và `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`
   (mật khẩu của khóa đó — giữ ngoài repo).
2. Tăng `version` trong `src-tauri/tauri.conf.json` và `package.json`.
3. `git tag v2.x.x && git push --tags` — workflow `release.yml` build NSIS,
   ký update artifact, sinh `latest.json` và tạo GitHub Release.
4. App đang chạy sẽ tự thấy bản mới và mời cập nhật.

Nếu đổi tên repo/chủ sở hữu, sửa `plugins.updater.endpoints` trong
`src-tauri/tauri.conf.json`.

## Kiến trúc

- `src-tauri/crates/overlay-core` — logic thuần (parse tọa độ, transform
  world↔pixel, tracker) + toàn bộ test suite port từ bản Python. Frontend
  **không bao giờ** tự tính transform; mọi payload mang sẵn cả cm lẫn pixel.
- `src-tauri/src` — Win32 (ranh giới an toàn trong `win/`), clipboard watcher,
  hotkeys, settings/store (giữ nguyên đường dẫn + format của bản Python cũ —
  người dùng cũ không mất dữ liệu), fetch dữ liệu, quản lý cửa sổ minimap.
- `src-tauri/src/islepilot` — tích hợp panel IslePilot (đọc `/me` + `/map`,
  cookie mã hóa DPAPI); chạy thread riêng, lỗi ở đây không ảnh hưởng bản đồ.
- `src/main` — cửa sổ chính (Svelte 5 + Tailwind + Leaflet CRS.Simple).
- `src/minimap` — entry riêng, canvas thuần, không framework: webview chạy
  cạnh game hàng giờ phải tối giản, chỉ vẽ khi có sự kiện (0% CPU idle).

Dữ liệu bản đồ **tải khi chạy lần đầu, không đóng gói** — basemap thuộc
VulnonaMAP (phái sinh từ tài sản game của Afterthought LLC); bản sao cá nhân
trên máy người dùng khác với việc app tái phân phối dữ liệu đó.

## Nguồn dữ liệu

- Basemap: [VulnonaMAP](https://vulnona.com/game/map/) (Coco.N) — ghép từ ảnh
  chụp trong game. Bản quyền hình ảnh: Afterthought LLC (The Isle).
- POI: [myislemap.com](https://myislemap.com/), VulnonaMAP, hướng dẫn Steam
  của wiredredman.

Không liên kết với Afterthought LLC.
