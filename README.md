# TheIsle Overlay — Navigation HUD + Nhìn Đêm Community Fork

**Tiếng Việt** · [English](README.en.md)

> **Bản fork cộng đồng tập trung vào điều hướng**, duy trì bởi
> [@nguyenduytamgithub](https://github.com/nguyenduytamgithub). Mã được phát
> triển từ bản công khai
> [`toantranct/theisle-overlay` v1.5.2](https://github.com/toantranct/theisle-overlay/tree/v1.5.2)
> (`f628a18`). Tác giả và dự án gốc: **Trần Quốc Toản**.

Nguồn hiện tại là ứng viên tích hợp **v1.7.4 Navigation + Magnifier Night
Boost**. Mục tiêu là làm vị trí, hướng đi, đường đã đi, waypoint và khả năng
quan sát cảnh tối hữu dụng hơn trong lúc chơi — không đọc bộ nhớ game và không
can thiệp Easy Anti-Cheat. Bản công khai chỉ được phát hành sau khi vượt qua
nghiệm thu cảnh đêm thật và kiểm tra điều hướng trong game.

Upstream 2.x hiện là bản phát hành đóng mã nguồn với các tính năng Pro riêng.
Fork này **không có** Voice, vị trí bạn bè, Skin editor hay realtime Pro của
2.x; bảng so sánh bên dưới chỉ đối chiếu với bản mã nguồn mở **v1.5.2**.

Bản đồ hiện đè lên màn hình khi chơi **The Isle: Evrima** (map Gateway).
Minimap tròn bám theo cửa sổ game · bản đồ lớn với POI, tên địa danh, waypoint,
vết đường đi · chỉ số khủng long + Garage (Gacha) với **xem 3D** từ hệ thống
IslePilot, đăng nhập Steam **một lần dùng cho mọi server** · giao diện song ngữ
Việt/Anh · bộ cài Windows gọn, cập nhật thủ công từ Releases của fork.

## Fork này cải thiện gì?

| Khi đang chơi | Upstream mã mở v1.5.2 | Ứng viên tích hợp v1.7.4 |
|---|---|---|
| Nhịp lấy vị trí IslePilot | Mặc định 10 giây | Mặc định **5 giây**; giữ nguyên nếu bạn đã tự đặt giá trị khác |
| Chuyển động giữa hai mẫu server | Chấm vị trí nhảy theo từng lần trả về | Chạy thẳng 4 giây, giảm dần tới 12 giây rồi giữ; hiệu chỉnh **300/650 ms** theo độ lệch |
| Tọa độ lỗi/nhảy xa | Có thể kéo đường mòn lệch hàng kilomet | Mẫu bất khả thi bị cách ly và cắt dự đoán cũ; chỉ xác nhận đổi chỗ xa khi có **hai mẫu nhất quán** |
| Hướng đi | Chủ yếu suy ra từ quãng đường đã đi | Tách riêng yaw server và hướng chuyển động; đổi nguồn sau 1 giây ổn định, chống xoay vòng qua 0° |
| Waypoint | Mũi tên tới điểm gần nhất | Chọn đúng một điểm làm đích; bản đồ lớn, minimap và HUD dùng **cùng một đích** |
| Dẫn đường trên màn hình game | Không có HUD riêng | Mũi tên đích Bắc-lên ổn định + câu lệnh **ĐI THẲNG / CHẾCH / RẼ / QUAY LẠI** và chữ hướng Đông Tây Nam Bắc |
| Khi dữ liệu chậm/mất | Khó biết chấm đang mới hay cũ | Ghi rõ **ĐANG BÁM / ĐANG ƯỚC LƯỢNG / CHỜ SERVER** |
| Khi Alt-Tab | Minimap tự ẩn theo game | HUD cũng tự ẩn, tự bám lại cửa sổ game và tự phục hồi nếu WebView chết |
| Cảnh đêm quá tối | Không có nút chỉnh sáng chuyên dụng | Windows Magnification tăng tương phản từ cảnh đang hiển thị; nút **NHÌN ĐÊM** + `Ctrl+Alt+N`, cường độ 0–100, tự ẩn/dọn theo game |

Điểm quan trọng: phần hiển thị 30 FPS là **ước tính cục bộ có giới hạn**, không
bịa thành tọa độ thật. Tọa độ thật vẫn được server xác nhận theo chu kỳ 5 giây. Server không
có live map thì dùng `Tab` → **Asset Location** như trước.

▶️ **Video hướng dẫn cài đặt & sử dụng:**

[![Video hướng dẫn TheIsle Overlay](https://img.youtube.com/vi/R2IzwqHapuw/hqdefault.jpg)](https://y2u.be/R2IzwqHapuw)

![Minimap và chỉ số khủng long đè lên game](docs/screenshot-ingame.jpg)

![Bản đồ lớn với tên địa danh và các lớp POI](docs/screenshot-fullmap.png)

![Tab Khủng long của bạn với chỉ số và Prime progress](docs/screenshot-dino.png)

## Tính năng

- **Minimap tròn** bám góc cửa sổ game, chuột bấm xuyên qua, không cản trở lúc chơi.
  Hướng Bắc luôn ở trên; mũi tên rìa bản đồ chỉ tới đúng waypoint bạn chọn.
- **Navigation HUD trên game**: mũi tên lớn luôn chỉ **hướng tuyệt đối tới đích**
  với Bắc ở trên, chữ hướng đi BẮC–ĐÔNG–NAM–TÂY, lệnh rẽ dễ hiểu, tên đích,
  khoảng cách và trạng thái dữ liệu; tự ẩn khi Alt-Tab và bật/tắt bằng
  `Ctrl+Alt+H`.
- **Nhìn đêm trực tiếp trên màn hình**: nút **NHÌN ĐÊM** góc trên bên phải và
  `Ctrl+Alt+N`; chỉnh cường độ 0–100 trong Cài đặt. v1.7.4 dùng Windows
  Magnification để đọc pixel màn hình đang hiển thị và vẽ lại cục bộ với độ
  tương phản cao hơn; gamma không còn được áp dụng. Cửa sổ native bấm xuyên qua,
  tự dọn khi Alt-Tab/tắt/thoát và chỉ báo BẬT sau khi đọc ngược đúng cấu hình.
  Mức 70 là mặc định; tăng nếu còn tối, giảm nếu vùng sáng bị cháy.
- **Bản đồ lớn**: phóng to/thu nhỏ mượt, 12 lớp bật/tắt được (nước ngọt, nguồn
  nước, mỏ muối, vũng bùn, khu bảo tồn, vùng di cư, vùng tuần tra AI, khu thức
  ăn, động vật với biểu tượng riêng từng loài 🐗🦌🐢, tên vùng, địa điểm, và
  lớp **POI server** sống từ IslePilot), tên địa danh hiện trực tiếp trên bản
  đồ; danh sách lớp thu gọn được; nút xóa đường đi cho đỡ rối mắt giữa trận.
  Mở bản đồ bằng phím tắt là app tự về tab Bản đồ.
- **3 kiểu nền bản đồ**: ảnh chụp Vulnona (mặc định) hoặc bản vẽ tay
  [IsleMaps](https://www.islemaps.com/) sáng/tối — đổi trong Cài đặt, áp dụng
  cho cả bản đồ lớn lẫn minimap. Nền IsleMaps vẽ theo phiên bản game mới hơn,
  thấy cả quần đảo đông nam (Hell's Mouth).
- **Điểm đánh dấu (waypoint)**: bấm chuột phải để cắm, đổi tên/đổi màu, xóa,
  biểu tượng nhanh (💀 chỗ chết, 🏠 hang…); chọn **Dẫn đường tới điểm này** để
  bản đồ lớn, minimap và HUD cùng dẫn tới đúng đích đó.
- **Tìm kiếm & điều hướng**: ô tìm địa danh/waypoint, dán tọa độ để nhảy tới,
  đường thẳng và mũi tên từ vị trí hiện tại tới đích đã chọn; đoạn ước lượng
  hiện màu xanh nét đứt trên cả hai bản đồ; báo đã tới trong bán kính mặc định
  25 m và không lật mũi tên khi dự đoán vừa đi qua ghim. Đây là hướng thẳng tới
  ghim, **không phải** đường đi an toàn qua địa hình hay hệ thống navmesh.
- **Đường đã đi ổn định hơn**: tự ghi theo phiên, khôi phục phiên trước, bỏ
  mẫu nhảy bất khả thi và không ghi vị trí dự đoán vào lịch sử.
- **Khủng long của bạn**: growth, máu, đói, khát, thể lực, dinh dưỡng
  Carb/Đạm/Béo và Prime progress (có dịch tiếng Việt) từ hệ thống IslePilot;
  thanh chỉ số + bảng nhiệm vụ Prime gọn ngay dưới minimap. Đăng nhập Steam
  **một lần dùng cho mọi server IslePilot** — đổi server là dữ liệu tự đổi theo.
- **Garage (Gacha) với xem 3D**: mỗi dino đã park là một card có **model 3D
  xoay/phóng được, đúng màu skin** + growth + nút Park/Restore/Đổi tên/Bán;
  model tải một lần rồi cache, mở lại tức thì và offline được.
- **Phím tắt toàn cục** đổi được trong app, song ngữ Việt/Anh; riêng HUD có
  `Ctrl+Alt+H` và thanh chỉnh độ đậm trong Cài đặt.

## Cài đặt nhanh

1. Mở [Releases của fork](https://github.com/nguyenduytamgithub/theisle-overlay/releases)
   và tải file `TheIsle Overlay_*_x64-setup.exe` của bản được đánh dấu mới nhất.
2. Nếu đang chạy một bản Overlay khác, thoát nó từ khay hệ thống rồi chạy bộ
   cài. Dữ liệu settings và waypoint cũ được installer giữ lại.
3. Nếu Windows SmartScreen cảnh báo, chọn **More info → Run anyway**. Bộ cài
   chưa ký số; SHA-256 chính thức được ghi ngay trong trang Release.
4. Trong The Isle, chọn **Windowed** hoặc **Borderless Fullscreen**. Exclusive
   Fullscreen không cho cửa sổ overlay của Windows hiện đè lên game.
5. Mở app một lần. Lần đầu app tải dữ liệu bản đồ khoảng 3 MB.

Yêu cầu: **Windows 10/11 64-bit**. WebView2 (thường đã có sẵn trên Windows 11;
nếu thiếu, installer tự tải về).

Fork này cập nhật **thủ công**. Không bấm cập nhật lên upstream 2.x nếu bạn muốn
giữ Navigation HUD; hãy cài bản mới từ Releases của fork khi có thông báo.

## Dùng Navigation HUD và dẫn đường

1. Kết nối IslePilot theo hướng dẫn bên dưới. Server có live map sẽ tự cập nhật
   vị trí mỗi 5 giây; server không hỗ trợ thì vào game bấm `Tab` →
   **Asset Location** khi cần lấy mẫu mới.
2. Mở bản đồ lớn bằng `Ctrl+Alt+F`.
3. Bấm chuột phải tại nơi muốn tới để tạo waypoint, hoặc chọn waypoint đã lưu.
4. Trong menu waypoint, chọn **Dẫn đường tới điểm này**.
5. Quay lại game. Giữ Bắc ở phía trên để đối chiếu: mũi tên xanh lớn chỉ hướng
   tuyệt đối tới đích; dòng **HƯỚNG ĐI** và câu lệnh ĐI THẲNG/CHẾCH/RẼ giúp
   chỉnh đường. Minimap dùng cùng đích. Chọn **Dừng dẫn đường** khi xong.

| Phím | Tác dụng |
|---|---|
| `Ctrl+Alt+H` | Bật/tắt Navigation HUD |
| `Ctrl+Alt+N` | Bật/tắt Nhìn đêm |
| `Ctrl+Alt+M` | Bật/tắt minimap |
| `Ctrl+Alt+F` | Mở/ẩn bản đồ lớn |
| `Ctrl+Alt+R` | Tải lại giao diện khi minimap/HUD không vẽ |
| `Ctrl+Alt+C` | Bật/tắt click-through của minimap |
| `Ctrl+Alt+Up/Down` | Tăng/giảm độ đậm minimap |
| `Ctrl+Alt+Left/Right` | Tăng/giảm kích thước minimap |

Các phím đều đổi được trong **Cài đặt**. HUD tự ẩn khi game không ở foreground;
đây là hành vi bình thường, không phải ứng dụng bị tắt.

## Kết nối "Khủng long của bạn" (IslePilot)

Tab **Khủng long** đọc chỉ số dino của chính bạn (growth, máu, đói, khát, thể
lực, dinh dưỡng, Prime progress) từ hệ thống IslePilot. Có 2 cách kết nối:

**Cách 1 — Đăng nhập Steam qua IslePilot (khuyên dùng):** mở tab Khủng long →
bấm **Đăng nhập Steam** → đăng nhập trong cửa sổ islepilot.eu hiện ra, cửa sổ
tự đóng khi xong. Chỉ cần làm **một lần duy nhất** — không cần nhập link
server, dùng cho **mọi server IslePilot**, đổi server trong game là dữ liệu tự
đổi theo. Cách này còn mở thêm tab **Garage (Gacha)** và lớp **POI server**
trên bản đồ. Nếu cửa sổ không tự bắt được token, mở mục *"Hoặc dán token thủ
công"* và dán token (hoặc nguyên link `theisle-overlay://…`).

**Cách 2 — Cách cũ: nhập server + cookie** (chỉ khi cách 1 không hoạt động;
cookie lưu riêng từng server, đổi server phải làm lại). Mở mục **"Cách cũ"**
trong phần đăng nhập, nhập link server rồi bấm Đăng nhập Steam trong mục đó;
vẫn không được thì dán cookie thủ công:

1. Mở trang server trong trình duyệt và đăng nhập Steam ở đó. Bấm **F12**
   (hoặc chuột phải → **Inspect**) rồi chọn tab **Application** (Chrome) /
   **Storage** (Firefox).

   ![Mở DevTools và chọn tab Application](docs/guide-dino-1-devtools.png)

2. Chọn **Cookies** → domain của server → bấm cookie tên **`islepilot_player`**
   → copy toàn bộ **Value**.

   ![Copy giá trị cookie islepilot_player](docs/guide-dino-2-copy-cookie.jpg)

3. Trong app: dán vào ô cookie → bấm **Kiểm tra & lưu cookie**.

   ![Nhập link server, dán cookie và lưu trong app](docs/guide-dino-3-paste-app.jpg)

Server có **live map** thì app tự nhận và bật "lấy vị trí tự động" — khỏi cần
copy tọa độ thủ công; server tắt live map thì tùy chọn này tự khóa.

**Một số server dùng IslePilot** (tham khảo — mọi server chạy IslePilot đều
dùng được):

- https://mixi.islepilot.eu
- https://hoho.islepilot.eu
- https://sdvn.islepilot.eu
- https://sdvn2.islepilot.eu
- https://khunglong.islepilot.eu
- https://islepilot.eu/p/sbtcisland

## Nhẹ cỡ nào?

Ứng viên tích hợp v1.7.4 và bằng chứng cài/runtime được ghi sau khi build và
kiểm tra. Không dùng hash ứng viên để quảng bá là bản phát hành trước khi nghiệm
thu cảnh đêm thật và điều hướng trong game đạt.

| Hạng mục | Dung lượng |
|---|---|
| File cài đặt NSIS v1.7.4 candidate | Ghi sau khi build và băm SHA-256 |
| File chạy sau khi cài v1.7.4 candidate | Ghi sau khi build và băm SHA-256 |
| Dữ liệu bản đồ tải lần đầu | 2,9 MB (ảnh nền 2,6 MB + dữ liệu điểm 0,3 MB) |

HUD và bản đồ giới hạn cập nhật cục bộ khoảng 30 FPS, chạy thẳng 4 giây rồi
giảm dần tới mốc giữ 12 giây; không có mẫu mới thì hiện **CHỜ SERVER**;
không chạy bộ đọc game hoặc quét bộ nhớ nền. Mức RAM thực tế phụ thuộc WebView2,
số tab đang mở và model 3D đã tải, nên fork không công bố một con số RAM cố định.

## Lưu ý cần biết

1. **Chế độ hình ảnh của game**: overlay không thể hiện đè lên **Toàn màn hình
   độc quyền (Exclusive Fullscreen)** — đây là giới hạn của Windows với mọi
   overlay ngoài tiến trình. Hãy dùng **Cửa sổ** hoặc **Toàn màn hình không viền**.
   App tự đọc cấu hình game và cảnh báo nếu bạn đang để sai chế độ.
2. **Vị trí thật vẫn phụ thuộc server**: IslePilot live map xác nhận vị trí mặc
   định mỗi 5 giây. Giữa hai lần đó app hiển thị cục bộ mượt, giảm dự đoán sau
   4 giây và giữ hẳn sau 12 giây. Server tắt
   live map thì bạn phải bấm `Tab` → **Asset Location** để cập nhật thủ công.
3. **Mũi tên đích không dùng yaw server** nên không còn xoay lung tung khi nhân
   vật đổi góc nhìn. Dòng HƯỚNG ĐI ưu tiên hướng chuyển động đã xác nhận, chỉ
   dùng yaw ổn định làm dự phòng; dữ liệu quá cũ chuyển sang **CHỜ SERVER**.
4. **Nhìn đêm v1.7.4 cần Windowed/Borderless** để Windows Magnification ghép
   ảnh đã tăng tương phản lên game; Exclusive Fullscreen không hiện được overlay.
   Nếu hình quá sáng, giảm cường độ; nếu trạng thái kẹt, bấm `Ctrl+Alt+N` một lần
   để tắt sạch rồi bật lại. Tính năng xử lý pixel màn hình đang hiển thị ở máy
   này nhưng không mở tiến trình game, không đọc bộ nhớ game, không inject/hook,
   không giả lập input, không truy cập mạng và không lưu/gửi ảnh. Việc có được
   phép dùng hay không vẫn phụ thuộc luật của server.
5. **Không mở được hai bản cùng lúc** — phím tắt toàn cục mang tính độc quyền,
   hai bản chạy song song sẽ tranh nhau.
6. **Máy ít RAM**: ẩn bản đồ lớn bằng `Ctrl+Alt+F` khi vào game — app tự giảm
   bộ nhớ của cửa sổ ẩn. Bấm nút X thì app thu về **khay hệ thống** (icon cạnh
   đồng hồ) như Steam/Discord — chuột trái icon để mở lại, chuột phải → Thoát
   để tắt hẳn.
7. **Phím tắt bị ứng dụng khác chiếm**: app báo ngay khi khởi động, đổi lại trong
   tab Cài đặt.
8. **Tính năng "Khủng long của bạn"** hỗ trợ server dùng nền tảng IslePilot —
   xem mục [Kết nối "Khủng long của bạn"](#kết-nối-khủng-long-của-bạn-islepilot).
   Chế độ đăng nhập Steam (khuyên dùng) đọc qua API JSON ổn định; riêng cách cũ
   server + cookie phân tích HTML trang web của server nên **có thể hỏng khi
   IslePilot đổi giao diện** — app sẽ báo khi phát hiện server vừa cập nhật.
   Nếu phần này lỗi, các tính năng bản đồ **không bị ảnh hưởng**.
9. **Nên hỏi admin server** trước khi dùng thường xuyên — một số server có luật
   riêng về công cụ bên thứ ba. Tùy chọn lấy vị trí tự động chỉ bật khi app dò
   thấy server có live map; server tắt live map thì tùy chọn tự khóa, và lựa
   chọn tắt/bật thủ công của bạn luôn được tôn trọng.
10. **Token/cookie đăng nhập** được mã hóa bằng Windows DPAPI, chỉ giải được
   bằng tài khoản Windows của bạn trên chính máy đó.
11. **SmartScreen** có thể cảnh báo vì installer chưa ký số. Đối chiếu SHA-256
    trên trang Release và cập nhật fork bằng installer thủ công.

## An toàn với anti-cheat

Game chạy Easy Anti-Cheat cấp kernel. App này an toàn vì **không bao giờ đụng
vào tiến trình game**:

- Vị trí lấy qua **HTTPS từ IslePilot live map** khi server cho phép, hoặc từ
  **clipboard** sau khi bạn tự bấm `Tab` → **Asset Location**. Cả hai đều là
  dữ liệu server/game chủ động cung cấp ngoài tiến trình game.
- Phím tắt dùng `RegisterHotKey` (API hợp tác của Windows), **không phải**
  keyboard hook.
- Nhìn đêm dùng Windows Magnification để đọc các pixel **đã hiển thị trên màn
  hình** và vẽ lại cục bộ với độ tương phản cao hơn trong một bề mặt native bấm
  xuyên qua. Nó không mở tiến trình game, không đọc bộ nhớ, không inject/hook,
  không truy cập mạng và không lưu hoặc gửi hình ảnh.
- Chỉ số khủng long / Garage / model 3D lấy qua **HTTPS tới hệ thống IslePilot**
  (API islepilot.eu hoặc website của server) — cũng không liên quan gì tới tiến
  trình game.
- Không bao giờ: đọc bộ nhớ game, inject DLL, hook DirectX, giả lập phím,
  bắt gói mạng, tự chép tọa độ từ game theo timer, chia sẻ vị trí giữa người chơi.

CI có bước grep chặn mọi call site API cấm (`scripts/check-forbidden-apis.ps1`)
và test riêng `night_vision_safety` khóa adapter nhìn đêm vào đúng API màn hình.
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

# Sau mỗi lần map game cập nhật (chạy cho từng nền đã tải):
cargo run --bin verify_data --features devtools -- --source vulnona
cargo run --bin verify_data --features devtools -- --source islemaps-light
cargo run --bin verify_data --features devtools -- --source islemaps-dark
cargo test -p theisle-overlay --lib -- --ignored parse_real_cache
```

Lưu ý: `.cargo/config.toml` đặt `target-dir` ra ngoài thư mục OneDrive.

## Nguồn dữ liệu

Dữ liệu bản đồ **tải khi chạy lần đầu, không đóng gói sẵn** — đây là bản sao
cá nhân trên máy bạn, không phải bản phát hành lại.

- Basemap: [VulnonaMAP](https://vulnona.com/game/map/) (Coco.N) — ghép từ ảnh
  chụp trong game. Bản quyền hình ảnh: Afterthought LLC (The Isle).
- Nền IsleMaps (tùy chọn, chỉ tải khi bạn chọn trong Cài đặt) và điểm spawn
  động vật: [islemaps.com](https://www.islemaps.com/) (Pont & Emeara).
- POI: [myislemap.com](https://myislemap.com/), VulnonaMAP, hướng dẫn Steam
  của wiredredman.

Không liên kết với Afterthought LLC.

## Nguồn gốc, giấy phép và liên hệ

- Fork Navigation HUD: duy trì bởi
  [@nguyenduytamgithub](https://github.com/nguyenduytamgithub); báo lỗi của fork
  tại [GitHub Issues](https://github.com/nguyenduytamgithub/theisle-overlay/issues).
- Dự án và mã gốc v1.5.2: **Trần Quốc Toản** —
  [`toantranct/theisle-overlay`](https://github.com/toantranct/theisle-overlay).
- Upstream hiện không có file `LICENSE`. Fork này không tự gán một giấy phép
  mới; việc mã nguồn được xem công khai không thay thế điều khoản cấp phép.

Liên hệ và mục ủng hộ dưới đây thuộc về **tác giả upstream**, được giữ nguyên để
ghi công đúng nguồn:

- 📧 Email: toantranct1@gmail.com
- 💬 Facebook: https://www.facebook.com/satann247/
- 🐛 Upstream Issues: [GitHub Issues](https://github.com/toantranct/theisle-overlay/issues)

Nếu thấy nền tảng gốc hữu ích, bạn có thể mời tác giả upstream một ly cà phê:

<img src="docs/qr_donate.png" alt="VietQR — Techcombank 8866886767 TRAN QUOC TOAN" width="280">

**Techcombank · 8866886767 · TRAN QUOC TOAN**
