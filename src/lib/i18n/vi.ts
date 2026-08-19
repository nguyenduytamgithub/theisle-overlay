// Toàn bộ chuỗi hiển thị tiếng Việt. Port từ strings_vi.py của bản gốc,
// thêm các khóa mới cho tab, danh sách waypoint, cài đặt và hướng dẫn.
// Không file UI nào được viết thẳng chuỗi hiển thị.

export const vi = {
  // --- chung ---
  "app.title": "Bản đồ The Isle",
  "app.minimap_title": "Bản đồ nhỏ",
  "app.fullmap_title": "Bản đồ Gateway",

  // --- tab ---
  "tab.map": "Bản đồ",
  "tab.settings": "Cài đặt",
  "tab.guide": "Hướng dẫn",

  // --- trạng thái vị trí ---
  "pos.none": "Chưa có vị trí",
  "pos.hint":
    "Trong game bấm Tab, rồi bấm chuột vào “Asset Location” ở góc trên bên phải để chép tọa độ.",
  "pos.off_map": "Ngoài bản đồ",

  // --- hướng ---
  "dir.N": "Bắc",
  "dir.NE": "Đông Bắc",
  "dir.E": "Đông",
  "dir.SE": "Đông Nam",
  "dir.S": "Nam",
  "dir.SW": "Tây Nam",
  "dir.W": "Tây",
  "dir.NW": "Tây Bắc",
  "heading.unknown": "Chưa rõ hướng",
  "heading.hint": "Chép tọa độ lần nữa sau khi di chuyển để biết hướng đi.",

  // --- layer POI ---
  "layer.water": "Nguồn nước",
  "layer.sanctuary": "Khu bảo tồn",
  "layer.migration": "Vùng di cư",
  "layer.saltlick": "Mỏ muối",
  "layer.mudwallow": "Vũng bùn",
  "layer.food": "Khu vực thức ăn",
  "layer.patrol": "Vùng tuần tra AI",
  "layer.region": "Tên vùng",
  "layer.landmark": "Địa điểm",
  "layers.title": "Lớp bản đồ",
  "layers.zone_labels": "Tên vùng khoanh",

  // --- waypoint ---
  "wp.title": "Điểm đánh dấu",
  "wp.new": "Điểm đánh dấu mới",
  "wp.add": "Thêm điểm",
  "wp.remove": "Xóa điểm",
  "wp.rename": "Đổi tên",
  "wp.name_prompt": "Tên điểm đánh dấu:",
  "wp.empty": "Chưa có điểm nào. Bấm chuột phải lên bản đồ để thêm.",
  "wp.distance": "{dir} · {dist}",
  "wp.here": "Vị trí của tôi",
  "wp.confirm_delete": "Xóa điểm “{name}”?",

  // --- vết đường ---
  "trail.title": "Đường đã đi",
  "trail.previous": "Đường đi phiên trước",

  // --- nút chung ---
  "btn.close": "Đóng",
  "btn.ok": "Đồng ý",
  "btn.cancel": "Hủy",
  "btn.save": "Lưu",

  // --- cảnh báo ---
  "warn.exclusive_fullscreen":
    "Game đang chạy chế độ Toàn màn hình. Bản đồ nhỏ sẽ không hiện đè lên được. " +
    "Hãy vào Cài đặt › Hình ảnh trong game và đổi sang “Cửa sổ” hoặc “Toàn màn hình không viền”.",
  "warn.hotkey_failed":
    "Không đăng ký được các phím tắt sau, vì ứng dụng khác đang giữ chúng:",
  "warn.no_data":
    "Chưa có dữ liệu bản đồ trên máy. Cần tải về một lần trước khi dùng.",

  // --- phím tắt (tên hành động) ---
  "hotkey.toggle_minimap": "Hiện/ẩn bản đồ nhỏ",
  "hotkey.toggle_fullmap": "Mở/đóng bản đồ lớn",
  "hotkey.toggle_click_through": "Bật/tắt chế độ bấm được",
  "hotkey.mark_here": "Đánh dấu vị trí hiện tại",
  "hotkey.opacity_up": "Bản đồ nhỏ đậm hơn",
  "hotkey.opacity_down": "Bản đồ nhỏ nhạt hơn",
  "hotkey.zoom_in": "Thu gần vùng nhìn",
  "hotkey.zoom_out": "Nhìn xa hơn",

  // --- cài đặt ---
  "settings.language": "Ngôn ngữ · Language",
  "settings.minimap": "Bản đồ nhỏ",
  "settings.visible": "Hiện bản đồ nhỏ",
  "settings.click_through": "Chuột bấm xuyên qua (không cản trở lúc chơi)",
  "settings.corner": "Góc neo theo cửa sổ game",
  "corner.top-left": "Trên trái",
  "corner.top-right": "Trên phải",
  "corner.bottom-left": "Dưới trái",
  "corner.bottom-right": "Dưới phải",
  "settings.size": "Kích thước",
  "settings.margin": "Cách mép",
  "settings.opacity": "Độ đậm",
  "settings.radius": "Bán kính vùng nhìn",
  "settings.hotkeys": "Phím tắt",
  "settings.hotkeys_hint":
    "Bấm vào ô phím rồi nhấn tổ hợp mới. Cần ít nhất một phím bổ trợ (Ctrl/Alt/Shift/Win).",
  "settings.press_keys": "Nhấn tổ hợp phím… (Esc để hủy)",
  "settings.hotkey_in_use": "Tổ hợp này đang bị ứng dụng khác giữ",
  "settings.hotkey_duplicate": "Trùng với một phím tắt khác trong ứng dụng",
  "settings.hotkey_invalid": "Tổ hợp không hợp lệ — cần ít nhất một phím bổ trợ",
  "settings.number_format": "Định dạng số tọa độ",
  "format.auto": "Tự động nhận biết",
  "format.us": "Kiểu Mỹ — 1,234.5",
  "format.eu": "Kiểu Châu Âu — 1.234,5",
  "settings.data": "Dữ liệu",
  "settings.open_trails": "Mở thư mục đường đi",
  "settings.redownload": "Tải lại dữ liệu bản đồ",

  // --- chạy lần đầu ---
  "firstrun.title": "Tải dữ liệu bản đồ",
  "firstrun.explain":
    "Ứng dụng cần tải ảnh bản đồ (~3 MB) và dữ liệu điểm về máy bạn một lần. " +
    "Dữ liệu được tải trực tiếp từ nguồn thay vì đóng gói sẵn — đây là bản sao cá nhân " +
    "trên máy bạn, không phải bản phát hành lại.",
  "firstrun.start": "Bắt đầu tải",
  "firstrun.downloading": "Đang tải…",
  "firstrun.done": "Xong! Đang mở bản đồ…",
  "firstrun.partial":
    "Đã tải được ảnh bản đồ nhưng dữ liệu điểm bị lỗi. Bạn vẫn dùng được bản đồ; " +
    "thử tải lại dữ liệu trong phần Cài đặt sau.",
  "firstrun.failed": "Tải thất bại. Kiểm tra kết nối mạng rồi thử lại.",
  "firstrun.retry": "Thử lại",
  "firstrun.continue": "Tiếp tục với bản đồ",

  // --- cập nhật ---
  "update.available": "Có bản cập nhật {version}",
  "update.install": "Cập nhật ngay",
  "update.installing": "Đang tải bản cập nhật…",
  "update.later": "Để sau",

  // --- ghi công ---
  "credits.title": "Nguồn dữ liệu",
  "credits.body":
    "Ảnh nền: VulnonaMAP (Coco.N) — ghép từ ảnh chụp trong game. " +
    "Hình ảnh thuộc bản quyền Afterthought LLC (The Isle). " +
    "Dữ liệu điểm: VulnonaMAP, myislemap.com, hướng dẫn Steam của wiredredman. " +
    "Ứng dụng này không liên kết với Afterthought LLC.",
} as const;

export type MsgKey = keyof typeof vi;
