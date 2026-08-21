# Changelog

Mọi thay đổi đáng chú ý của TheIsle Overlay được ghi tại đây, theo định dạng
[Keep a Changelog](https://keepachangelog.com/vi/1.1.0/) và đánh số phiên bản
[SemVer](https://semver.org/lang/vi/). Mã trong ngoặc là commit tương ứng.

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
