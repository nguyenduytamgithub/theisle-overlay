# Changelog

Mọi thay đổi đáng chú ý của TheIsle Overlay được ghi tại đây, theo định dạng
[Keep a Changelog](https://keepachangelog.com/vi/1.1.0/) và đánh số phiên bản
[SemVer](https://semver.org/lang/vi/). Mã trong ngoặc là commit tương ứng.

## [1.4.0] — 2026-08-23

### Thêm

- **Đăng nhập Steam 1 lần cho mọi server IslePilot** (khuyên dùng): đăng nhập
  qua islepilot.eu duy nhất một lần, token dùng chung cho mọi server — hết cảnh
  nhập link server + copy cookie mỗi lần đổi server. Token lưu mã hóa DPAPI;
  redirect được bắt ngay trong cửa sổ đăng nhập (không đăng ký protocol hệ
  thống, không đụng app overlay gốc nếu có cài); có ô dán token thủ công làm
  lối thoát. Chế độ mới đọc API JSON thay vì scrape HTML: thêm **thể lực, dinh
  dưỡng Carb/Đạm/Béo, tên server đang chơi, giới tính** trong tab Khủng long.
  Cách cũ nhập server + cookie giữ nguyên làm dự phòng, người dùng cũ không
  phải làm lại gì. (`b8cff31`)
- **Tab Garage (Gacha)** — cần đăng nhập token: mỗi dino đã park là một card
  gồm **model 3D xoay/phóng được, đúng màu skin đã park** + tên/loài/growth +
  nút Park/Restore/Đổi tên/Bán (Bán chỉ hiện khi server bật; có hộp xác nhận).
  Model + texture tải từ CDN công khai của IslePilot (21 loài), cache trên đĩa
  — lần đầu mỗi loài tải vài MB có hiện tiến trình, các lần sau mở tức thì và
  offline được. Danh sách tự làm mới mỗi 10 phút khi tab đang mở (có dòng
  trạng thái), server không hỗ trợ garage thì báo rõ thay vì nút chết.
  (`63b4caf`, `2044c5f`)
- **Lớp bản đồ "POI server (IslePilot)"**: vẽ POI sống do admin server đặt
  (Sanctuaries, Migration/Patrol Zones…) lên bản đồ lớn, màu theo server, tự
  làm mới ~15 giây; cần đăng nhập token, thiếu quyền (link Discord/server tắt
  live map) thì hiện lý do trong bảng lớp. (`5bbb840`)
- **Thanh Thể lực trên minimap**: dải chỉ số dưới đĩa thêm hàng ⚡ khi có dữ
  liệu (chế độ token); cửa sổ overlay tự cao thêm đúng một hàng. (`f60d567`)
- **Icon cho thanh tab + tab Ủng hộ riêng**: 6 tab đều có icon; QR VietQR
  chuyển từ popup Footer thành tab Ủng hộ cạnh Hướng dẫn. (`69dbf51`)
- **Protocol `theisle-overlay://`**: bấm link `theisle-overlay://?sid=..&token=..`
  từ bất kỳ đâu là mở app và đăng nhập luôn — cố ý không dùng scheme
  `isle-overlay://` để không tranh với app gốc. (`aa9aa8e`)

### Sửa

- **Minimap "tự bỏ tích" rồi bật lại không hiện, phải mở lại app** — hai lỗi
  thực địa: (1) Windows tự lặp hotkey khi giữ tổ hợp làm toggle đảo ngược tức
  thì → thêm debounce 350 ms cho các phím bật/tắt (phím chỉnh độ đậm/zoom vẫn
  lặp như chủ đích); (2) cửa sổ minimap chết (WebView2 crash) thì supervisor
  trước đây lặp vô hạn không làm gì — giờ tự phát hiện và dựng lại trong ~5
  giây. (`6265364`)

### Thay đổi

- **Hướng dẫn kết nối IslePilot viết lại**: 2 cách rõ ràng — Đăng nhập Steam
  qua IslePilot (khuyên dùng) và cách cũ server + cookie (dự phòng); bỏ mục
  giải thích hướng đi và câu "giữ bí mật chuỗi như mật khẩu". (`f7d7818`)
- Tab Khủng long và Garage được giữ sống sau lần mở đầu (chuyển tab không còn
  khựng); model 3D chỉ dựng lại khi đổi loài/màu, tạm ngừng render khi khuất
  màn hình. (`2044c5f`)

## [1.3.0] — 2026-08-22

### Thêm

- **Bảng nhiệm vụ Prime trên overlay**: panel mới dưới thanh chỉ số của bản đồ
  nhỏ, liệt kê 10 nhiệm vụ Prime kèm ✓/○ và bộ đếm "Prime 2/10"; nhiệm vụ xong
  tô xanh, dòng dài tự cắt "…". Bật/tắt bằng checkbox trong tab Khủng long hoặc
  **hotkey Ctrl+Alt+Q** (đổi được trong Cài đặt); cửa sổ overlay tự co giãn
  theo số nhiệm vụ, mất mạng tạm thời không làm panel co giật. (`ec5da8a`)
- **Dịch nhiệm vụ sang tiếng Việt**: từ điển dịch tay cho toàn bộ pool nhiệm
  vụ đã biết + mẫu theo số ("Visit 3 Patrol zones" → "Ghé 3 khu Tuần tra");
  câu lạ dịch qua API miễn phí MyMemory **đúng một lần** rồi lưu vĩnh viễn tại
  `%LOCALAPPDATA%\TheIsleOverlay\quest_translations.json` (hết quota tự nghỉ
  6 giờ và hiện tiếng Anh; UI tiếng Anh bỏ qua API hoàn toàn). Tab Khủng long
  hiện câu tiếng Việt, rê chuột thấy câu gốc tiếng Anh. (`ec5da8a`)

### Thay đổi

- **Vị trí từ IslePilot chính xác hơn**: đọc thẳng JSON markers API của panel
  (`/api/p/{slug}/map/markers` — đúng nguồn panel tự dùng, tọa độ UE cm chuẩn
  xác, không sợ panel đổi giao diện), tự nhận marker của bạn qua steamId trong
  cookie phiên; trang HTML `/map` giữ làm nguồn dự phòng và để dò khả năng
  live map. (`ec5da8a`)

## [1.2.0] — 2026-08-22

### Thêm

- **Mũi tên dẫn đường waypoint trên minimap**: mũi tên ở rìa đĩa chỉ hướng +
  khoảng cách tới waypoint gần nhất khi nó nằm ngoài vùng nhìn; waypoint trong
  vùng nhìn hiện thành chấm (viền trắng, khác chấm POI viền đen). Có công tắc
  riêng trong Cài đặt › Bản đồ nhỏ.
- **Chế độ bám vị trí + mũi tên mép** trên bản đồ lớn: kéo bản đồ đi nơi khác
  là tạm ngừng tự căn giữa, mũi tên ở mép màn hình chỉ về phía bạn — bấm mũi
  tên hoặc nút "Về vị trí của tôi" để quay lại và bám tiếp.
- **Ô tìm kiếm địa danh** trên bảng phải tab Bản đồ: gõ tên vùng/địa
  điểm/hồ nước/waypoint → nhảy tới kèm hiệu ứng nhấp nháy đánh dấu.
- **Dán tọa độ → nhảy tới**: dán chuỗi tọa độ (bạn bè nhắn qua chat) vào ô tìm
  kiếm — parse bằng đúng bộ đọc tọa độ của clipboard (thuần thao tác tay).
- **Màu + biểu tượng cho waypoint**: nút tròn màu cạnh mỗi waypoint (bấm để
  đổi qua 7 màu, đồng bộ cả minimap); hộp đặt tên có sẵn nút biểu tượng nhanh
  💀 🏠 💧 ⚠️ 🍖 — waypoint mang biểu tượng thì **hiện thẳng biểu tượng đó
  trên cả hai bản đồ** thay cho chấm tròn, và nhãn mũi tên dẫn đường cũng kèm
  biểu tượng ("💧 850 m").

- **Lớp "Động vật"**: ~340 điểm spawn động vật AI (Boar, Bunny, Chicken, Crab,
  Deer, Frog, Goat, Teno, Turtle) từ dữ liệu cộng đồng của islemaps.com — bật
  trong bảng lớp của bản đồ lớn, hiện trên cả minimap, dùng được với mọi kiểu
  nền. **Mỗi loài một biểu tượng riêng** (🐗 🐰 🐔 🦀 🦌 🐸 🐐 🦕 🐢) để nhận
  ra ngay không cần rê chuột. Nguồn tải runtime và fail-soft như mọi nguồn
  khác: trang đổi cấu trúc thì lớp tạm vắng, không ảnh hưởng gì còn lại
  (POIS_VERSION 3).
- **Lớp "Nước ngọt"**: lớp phủ tô đúng các sông/hồ uống được (từ islemaps.com),
  căn chỉnh chính xác trên CẢ ba kiểu nền nhờ quy đổi khung tọa độ phía Rust;
  hiện trên cả bản đồ lớn lẫn minimap, bật/tắt trong bảng lớp.
- **Nút "Xóa đường đi"** trong bảng bên phải tab Bản đồ: xóa vết phiên hiện
  tại + ẩn vết phiên trước trên CẢ HAI cửa sổ cho đỡ rối mắt giữa trận; file
  lịch sử trên đĩa vẫn giữ nguyên (có ghi mốc ngắt).
- **Toggle "Hiện đường đi trên bản đồ nhỏ"** trong Cài đặt › Bản đồ nhỏ — tắt
  là minimap sạch vết, bản đồ lớn vẫn hiện đủ.
- **Lựa chọn nền bản đồ** trong Cài đặt: Vulnona (mặc định) / IsleMaps sáng /
  IsleMaps tối — nền vẽ tay từ [islemaps.com](https://www.islemaps.com/) (Pont
  & Emeara), áp dụng đồng thời cho bản đồ lớn lẫn minimap. Bản IsleMaps vẽ theo
  phiên bản game mới hơn nên thấy cả quần đảo đông nam (Hell's Mouth) mà ảnh
  Vulnona 0.21.7 cắt mất. Ảnh chỉ tải khi bạn chọn lần đầu (~6,4 / 4,5 MB, có
  kiểm tra toàn vẹn kích thước), sau đó dùng offline; nút "Tải lại dữ liệu"
  refresh có điều kiện qua ETag. Waypoint/trail giữ nguyên vì mọi tọa độ lưu
  bằng cm gốc của game; mỗi nền có calibration riêng nhúng sẵn kèm bộ test
  anchor, và `verify_data --source` đối chiếu điểm POI với ảnh nền cho cả 3
  nguồn.

### Thay đổi

- Hình học bản đồ (kích thước ảnh, khung zoom) giờ lấy động từ Rust
  (`get_map_info`) thay vì hằng số 7800×7817 phía frontend; khung zoom neo theo
  tỉ lệ mặt đất nên mức phóng to/thu nhỏ thực tế giữ nguyên trên mọi nền.
- Minimap nạp ảnh IsleMaps có thu nhỏ lúc decode (bitmap thường trú ~6 MB thay
  vì ~25 MB) và giải phóng bitmap cũ ngay khi đổi nền.

## [1.1.1] — 2026-08-21

### Sửa

- **Minimap ẩn khi Alt-Tab ra ngoài game**: game chạy borderless vẫn "visible"
  phía sau các app khác nên gate theo sự-tồn-tại khiến minimap lơ lửng đè lên
  Chrome/desktop — giờ gate theo cửa sổ foreground, có debounce ~0,5 giây chống
  nhấp nháy, quay lại game là hiện ngay. (`c45ecf8`)
- **Cài mới xong minimap không hiện trong game**: quy tắc "ẩn khi bản đồ lớn
  đang mở" kiểm tra WS_VISIBLE, mà cửa sổ chính nằm SAU game vẫn tính là
  visible → chặn nhầm minimap tới khi người dùng tự ẩn cửa sổ chính. Giờ chỉ
  chặn khi cửa sổ chính thực sự ở foreground. (`4409e87`)

### Thay đổi

- Bản đồ lần đầu mở chỉ bật lớp **Tên vùng** — các lớp POI khác tắt sẵn cho
  sạch, bật lại một chạm trong bảng lớp; lựa chọn đã lưu của người dùng cũ
  không bị ảnh hưởng. (`6f06035`)

## [1.1.0] — 2026-08-21

### Thêm

- **Icon khay hệ thống (system tray)** với menu Hiện cửa sổ / Thoát (song ngữ, đổi theo ngôn ngữ app). Nút X giờ thu app về khay như Steam/Discord thay vì hủy cửa sổ; chuột trái icon để mở lại. (`ccdb70c`)
- **Minimap chỉ hiện khi game đang chạy** — cài đặt mới "Chỉ hiện khi game đang chạy" (mặc định bật). Game thu nhỏ là minimap ẩn trong ~0,25 giây, tắt game là ẩn trong ~2,5 giây, mở game lại là tự hiện đúng góc đã neo. (`ccdb70c`)
- **Tam giác vàng đánh dấu vị trí của bạn** trên cả minimap lẫn bản đồ lớn — viền kép đen-trắng, xoay theo hướng di chuyển; khi chưa rõ hướng hiện đĩa vàng. Không thể nhầm với waypoint hay chấm POI nữa. (`518992d`)
- **Hotkey cứu hộ Ctrl+Alt+R**: tải lại giao diện cả hai cửa sổ — là phím tắt toàn cục nên hoạt động kể cả khi UI không nhận click; vị trí/trail tự khôi phục sau reload. (`cc0eb13`)
- **Footer** hiện phiên bản app + gợi ý "Nhấn F5 hoặc Ctrl+Alt+R để tải lại" ở góc trái dưới. (`518992d`, `cc0eb13`)
- **Tab Khủng long**: khu cài đặt server + cookie tự thu gọn sau khi đăng nhập (nút ⚙ để mở lại — hết cảnh phải cuộn mới thấy chỉ số). App tự dò server có live map hay không: có thì mặc định bật "lấy vị trí tự động" (vẫn tắt được, và lựa chọn tay của bạn luôn được tôn trọng), không có thì tự tắt và khóa ô tích, kèm dòng trạng thái ngay dưới. (`990dae9`)
- Lệnh `get_current_position`: mở lại cửa sổ hoặc F5 là vị trí + trail hiện ngay, không phải chờ lần copy tọa độ kế tiếp. (`ccdb70c`, `518992d`)
- Ghi log lỗi giao diện toàn cục vào file log (`%LOCALAPPDATA%\TheIsleOverlay\logs`) và log mọi lần ẩn/hiện cửa sổ — báo lỗi thực địa giờ tự chỉ đích danh nguyên nhân. (`518992d`, `462c67a`)
- Hướng dẫn kết nối tab Khủng long từng bước (Steam login / dán cookie, kèm ảnh minh họa) trong tab Hướng dẫn của app và cả hai README, cùng danh sách server IslePilot tham khảo. (`5e40555`)

### Sửa

- **Hotkey "chết hẳn" phải End Task**: message queue của thread hotkey giờ được tạo trước khi công bố thread id (WM_QUIT từng bị nuốt khiến thread mồ côi giữ toàn bộ phím); dừng thread cũ có chờ (join) trước khi đăng ký lại nên đổi phím không còn làm mất hết hotkey; đăng ký có retry; hành động chạy trên worker riêng nên vòng bơm message không bao giờ bị chặn. (`ccdb70c`)
- **Đơ tab / UI không nhận click**: nhiều lớp — watchdog tự phát hiện và hồi webview bị treo (`ccdb70c`), cú hích `NotifyParentWindowPositionChanged` tái đồng bộ luồng chuột sau mỗi lần hiện (`462c67a`), và loại bỏ tận gốc ở mục Thay đổi bên dưới (`a999133`).
- **Minimap nuốt click của chính app**: đĩa minimap (luôn-trên-cùng) đè lên cửa sổ chính sẽ nuốt click vùng nó che khi tắt click-xuyên → minimap giờ tự ẩn khi bản đồ lớn đang mở và tự hiện lại khi đóng. (`cc0eb13`)
- **Poller IslePilot chết vĩnh viễn** khi phiên hết hạn hoặc site đổi giao diện (hai trường hợp không phân biệt được): giờ cảnh báo một lần, poll chậm dần (backoff lũy tiến, trần 5 phút) và tự hồi khi đọc được trở lại. (`ccdb70c`)
- Mở app từ icon khay từng hiện trang cũ do thiếu bước đồng bộ. (`462c67a`)
- Chuyển tab nhanh làm rò rỉ listener sự kiện; F5 giờ giữ nguyên tab đang mở; lỗi Leaflet được cách ly khỏi thanh tab (có nút Thử lại). (`518992d`)
- Sample tọa độ đầu tiên sau khi khởi động từng bị mất; minimap giờ luôn được giám sát kể cả khi webview khởi tạo lỗi (fallback 5 giây). (`ccdb70c`)
- Cookie hợp lệ nhưng **chưa có dino trên server** từng bị từ chối oan là "cookie
  không hợp lệ" (trang /me chỉ ghi "No dino" nên không có chỉ số để parse) — cả 3
  đường dán cookie / đăng nhập Steam / cảnh báo hết-phiên của poller giờ xác thực
  bằng dấu hiệu phiên đăng nhập thật của panel, không phụ thuộc chỉ số dino. Link
  server thừa dấu `/` cuối cũng được chuẩn hóa. (`16c26a1`)
- Sửa lỗi biên dịch CI: trùng module test, chữ ký `IsSuspended`. (`bf7e5e2`)

### Thay đổi

- **Gỡ hoàn toàn cơ chế đóng băng webview (TrySuspend)** — thao tác bất đồng bộ bên trong WebView2 này là gốc của mọi biến thể "cửa sổ hiện mà click chết" (3 sự cố thực địa một ngày). Thay bằng gợi ý dọn cache đồng bộ (`MemoryUsageTargetLevel` LOW khi ẩn / NORMAL khi hiện); sự kiện broadcast tới cả cửa sổ ẩn nên hiện lại là đúng ngay. Đánh đổi: app ẩn/ngồi khay nặng thêm ~80 MB — đổi lấy độ tin cậy tuyệt đối giữa trận. Watchdog giữ lại làm lính canh. (`a999133`, đảo ngược `4a2f3c7`)
- Mọi mutex dùng khóa chống-poisoning (`lock_safe`) — một panic lẻ ở thread nào đó không còn kéo sập clipboard, supervisor và hotkey cùng lúc. (`ccdb70c`)
- Kiểm tra ẩn/hiện cửa sổ qua registry HWND (`IsWindowVisible`/`IsIconic` — đọc tức thời) thay cho getter chặn-luồng của tauri; luồng bơm hotkey không còn phụ thuộc main loop. (`ccdb70c`)
- Nâng phiên bản 1.1.0. (`6357f40`)

## [1.0.1] — 2026-08-19

### Sửa

- Spam Ctrl+Alt+F nhanh không còn làm treo cửa sổ (thêm độ trễ ổn định + token hủy cho cơ chế đóng băng webview). (`2cb6f44`)

### Tài liệu

- Thêm ảnh chụp trong game và toàn bản đồ vào README; thêm mục liên hệ/ủng hộ. (`a6ea77e`, `2cb6f44`)

## [1.0.0] — 2026-08-19

Bản viết lại toàn bộ bằng Tauri (Rust + WebView2) từ app PySide6 gốc — giữ nguyên định dạng cài đặt/waypoint/trail nên dữ liệu cũ dùng lại được ngay. (`ffb2126`)

### Thêm

- Nhãn tên vùng/địa danh trên bản đồ và các lớp bật/tắt mới. (`1b40416`)
- Tab "Khủng long của bạn": đọc chỉ số dino (growth, máu, đói, khát, Prime) từ panel IslePilot của server, đăng nhập Steam qua webview hoặc dán cookie. (`9ea7a90`)
- Footer ghi công tác giả với liên kết GitHub/Facebook và popup ủng hộ VietQR. (`19e4dd2`)
- README song ngữ Việt/Anh. (`859f061`)

### Sửa

- Dữ liệu tải lần đầu tới thẳng minimap không cần khởi động lại; các tab dùng được ngay trong lúc tải. (`3f1bff7`)
- Ctrl+Alt+F khôi phục được bản đồ lớn từ trạng thái thu nhỏ. (`f8988fe`)

### Hiệu năng

- Đóng băng cửa sổ ẩn để giải phóng RAM renderer. (`4a2f3c7` — *đã gỡ ở 1.1.0 vì gây lỗi treo, xem phần Thay đổi của 1.1.0*)
