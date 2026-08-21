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
  "tab.dino": "Khủng long",
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
  "settings.require_game": "Chỉ hiện khi game đang chạy",
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

  // --- khủng long của bạn (IslePilot) ---
  "dino.title": "Khủng long của bạn",
  "dino.explain":
    "Đọc thông tin khủng long của chính bạn từ trang quản lý IslePilot của server " +
    "(growth, máu, đói, khát, Prime progress). Chỉ là kết nối HTTPS tới website của server " +
    "— không đụng gì tới game, an toàn với anti-cheat.",
  "dino.server": "Server",
  "dino.login": "Đăng nhập Steam",
  "dino.login_wait": "Đang chờ bạn đăng nhập trong cửa sổ vừa mở…",
  "dino.login_failed": "Đăng nhập không thành công. Thử lại.",
  "dino.logged_in": "Đã đăng nhập",
  "dino.logout": "Đăng xuất",
  "dino.auth_expired": "Phiên đăng nhập đã hết hạn — hãy đăng nhập lại.",
  "dino.supported_servers":
    "Hiện tại chỉ hỗ trợ server thuộc xxx.islepilot.eu (ví dụ mixi, sdvn2…).",
  "dino.manual_cookie": "Dán cookie đăng nhập",
  "dino.manual_cookie_hint":
    "Mở trang server trong trình duyệt và đăng nhập Steam. Bấm F12 → tab Application " +
    "(Chrome) hoặc Storage (Firefox) → Cookies → chọn domain server → tìm cookie tên " +
    "islepilot_player rồi copy phần Value dán vào đây. Giữ bí mật chuỗi này như mật khẩu.",
  "dino.cancel_login": "Hủy đăng nhập",
  "dino.manual_cookie_save": "Kiểm tra & lưu cookie",
  "dino.manual_cookie_checking": "Đang kiểm tra cookie…",
  "dino.manual_cookie_bad":
    "Cookie không hợp lệ hoặc phiên chưa đăng nhập — kiểm tra lại chuỗi đã dán.",
  "dino.server_settings": "Cài đặt server",
  "dino.live_map_yes": "Server có live map — vị trí sẽ tự cập nhật",
  "dino.live_map_checking": "Đang kiểm tra live map của server…",
  "dino.enabled": "Theo dõi thông tin khủng long",
  "dino.interval": "Tần suất cập nhật",
  "dino.overlay_panel": "Hiện thanh chỉ số dưới bản đồ nhỏ",
  "dino.use_map_position":
    "Lấy vị trí tự động từ live map của server (thay cho copy tọa độ thủ công)",
  "dino.rules_note":
    "⚠ Nên hỏi admin server trước khi dùng thường xuyên — một số server có luật riêng về " +
    "công cụ bên thứ ba. Dữ liệu hiển thị chỉ là của chính bạn, do panel của server cung cấp.",
  "dino.growth": "Trưởng thành",
  "dino.health": "Máu",
  "dino.hunger": "Đói",
  "dino.thirst": "Khát",
  "dino.prime": "Prime progress",
  "dino.online": "Online",
  "dino.offline": "Offline",
  "dino.updated": "Cập nhật lúc {time}",
  "dino.no_data": "Chưa có dữ liệu — bật theo dõi và chờ lần cập nhật đầu.",
  "dino.fetch_error": "Lỗi kết nối tới panel:",
  "dino.layout_changed":
    "IslePilot vừa cập nhật phiên bản mới — nếu số liệu trông sai, giao diện của họ có thể " +
    "đã đổi và app cần cập nhật theo.",
  "dino.map_disabled": "Server này tắt live map.",
  "dino.crashed":
    "Phần Khủng long gặp lỗi và đã được cách ly — bản đồ và các tính năng khác không bị ảnh hưởng.",
  "map.crashed":
    "Bản đồ gặp lỗi hiển thị. Bấm Thử lại, hoặc nhấn F5 để tải lại toàn bộ ứng dụng.",
  "btn.retry": "Thử lại",

  // --- cập nhật ---
  "update.available": "Có bản cập nhật {version}",
  "update.install": "Cập nhật ngay",
  "update.installing": "Đang tải bản cập nhật…",
  "update.later": "Để sau",

  // --- footer + donate ---
  "footer.developed_by": "Được phát triển bởi",
  "footer.donate": "Ủng hộ",
  "footer.reload_hint": "Nếu ứng dụng bị lỗi, nhấn F5 để tải lại",
  "donate.title": "Ủng hộ tác giả",
  "donate.hint": "Quét mã VietQR bằng app ngân hàng, hoặc chuyển khoản thủ công:",
  "donate.copy_stk": "Copy số tài khoản",
  "donate.copied": "Đã copy!",
  "donate.thanks": "Cảm ơn bạn đã ủng hộ! ❤",

  // --- ghi công ---
  "credits.title": "Nguồn dữ liệu",
  "credits.body":
    "Ảnh nền: VulnonaMAP (Coco.N) — ghép từ ảnh chụp trong game. " +
    "Hình ảnh thuộc bản quyền Afterthought LLC (The Isle). " +
    "Dữ liệu điểm: VulnonaMAP, myislemap.com, hướng dẫn Steam của wiredredman. " +
    "Ứng dụng này không liên kết với Afterthought LLC.",
} as const;

export type MsgKey = keyof typeof vi;
