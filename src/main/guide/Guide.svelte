<script lang="ts">
  // In-app usage guide, adapted from the original HUONG_DAN.md for the
  // installer-based app (no Python steps). Static prose per language; the
  // hotkey table reads the user's live bindings.
  import { onMount } from "svelte";
  import { getSettings, listenerBag, onSettingsChanged, type Settings } from "$lib/api";
  import { locale, t } from "$lib/i18n";

  let settings = $state<Settings | null>(null);

  onMount(() => {
    const bag = listenerBag();
    (async () => {
      settings = await getSettings();
      await bag.add(onSettingsChanged((s) => (settings = s)));
    })();
    return () => bag.dispose();
  });

  const ACTIONS = [
    "toggle_minimap",
    "toggle_fullmap",
    "toggle_click_through",
    "mark_here",
    "opacity_up",
    "opacity_down",
    "zoom_in",
    "zoom_out",
    "reload_ui",
  ] as const;
</script>

<div class="guide mx-auto max-w-2xl overflow-y-auto p-6 text-sm leading-relaxed">
  {#if $locale === "vi"}
    <h1>Cách dùng — điều quan trọng nhất</h1>
    <p>
      Ứng dụng <strong>không tự biết bạn đang ở đâu</strong>. Bạn phải tự báo cho nó,
      mỗi lần một cái:
    </p>
    <ol>
      <li>
        Chạy game ở chế độ <strong>Cửa sổ</strong> hoặc
        <strong>Toàn màn hình không viền</strong> (Cài đặt › Hình ảnh trong game).
      </li>
      <li>Trong game bấm <strong>Tab</strong> để mở màn hình nhân vật.</li>
      <li>
        Bấm chuột vào chữ <strong>"Asset Location"</strong> ở góc trên bên phải.
      </li>
      <li>Vị trí của bạn hiện lên bản đồ ngay lập tức.</li>
    </ol>
    <p>
      Cứ đến chỗ đáng nhớ thì bấm chép một lần. Các lần chép sẽ được nối thành đường đi.
    </p>

    <h2>Vì sao không tự động được?</h2>
    <p>
      Muốn bản đồ tự bám theo bạn liên tục thì phần mềm phải <strong>đọc bộ nhớ game</strong>
      hoặc <strong>tự bấm phím thay bạn</strong>. Cả hai đều là thứ mà Easy Anti-Cheat được
      thiết kế để phát hiện, và hậu quả là <strong>khóa tài khoản</strong>.
    </p>
    <p>
      Ứng dụng này chỉ đọc clipboard của Windows — thứ mà chính game tự nguyện đưa ra.
      Nó không đụng gì vào tiến trình game. Đó là lý do nó an toàn, và cũng là lý do nó
      không thể "live" theo đúng nghĩa. Đây là đánh đổi có chủ ý.
    </p>

    <h2>Hướng hoạt động thế nào</h2>
    <p>
      Ứng dụng tự tính hướng đi, nhưng cần <strong>hai lần chép tọa độ</strong> cách nhau
      ít nhất <strong>20 mét</strong>. Mũi tên chỉ <strong>hướng bạn vừa đi</strong>, không phải hướng
      mặt bạn đang quay về — đứng yên xoay người thì nó không biết được. Mẫu cũ quá
      <strong>10 phút</strong> thì hướng hết hạn và hiện lại <em>"Chưa rõ hướng"</em> để tránh chỉ sai.
    </p>

    <h2>Bản đồ lớn</h2>
    <ul>
      <li><strong>Lăn chuột</strong>: phóng to / thu nhỏ · <strong>kéo chuột trái</strong>: di chuyển</li>
      <li><strong>Bấm chuột phải</strong>: cắm điểm đánh dấu và đặt tên</li>
      <li><strong>Cột bên phải</strong>: bật/tắt từng lớp, trạng thái vị trí, danh sách điểm đánh dấu</li>
    </ul>

    <h2>Bản đồ nhỏ</h2>
    <p>
      Hình tròn, bám theo một góc cửa sổ game và <strong>chỉ hiện khi game đang chạy</strong>.
      <strong>B</strong> (màu vàng) = Bắc · <strong>Đ</strong> = Đông · <strong>N</strong> = Nam ·
      <strong>T</strong> = Tây. Bản đồ <strong>không xoay</strong> — hướng Bắc luôn ở trên.
      Tam giác/đĩa <strong>màu vàng viền đen-trắng</strong> là bạn, luôn giữ nguyên độ đậm dù
      bản đồ mờ tới đâu.
    </p>
    <p>
      Mặc định <strong>chuột bấm xuyên qua</strong> — bấm vào chỗ đó thì game nhận, không cản
      trở lúc chơi. Khi mở bản đồ lớn, bản đồ nhỏ tự ẩn và tự hiện lại lúc đóng.
    </p>
    <p>
      <strong>Lưu ý:</strong> lớp phủ không thể hiện đè lên chế độ
      <strong>Toàn màn hình độc quyền</strong>. Đây là giới hạn của Windows. Dùng Cửa sổ hoặc
      Toàn màn hình không viền.
    </p>

    <h2>Khủng long của bạn (IslePilot)</h2>
    <p>
      Tab <strong>Khủng long</strong> đọc chỉ số dino của chính bạn (growth, máu, đói, khát,
      Prime progress) từ trang quản lý IslePilot của server. Có 2 cách kết nối — chọn một:
    </p>
    <ol>
      <li>
        <strong>Đăng nhập Steam (nhanh nhất):</strong> nhập link server → bấm
        <strong>Đăng nhập Steam</strong> → đăng nhập trong cửa sổ hiện ra. Xong.
      </li>
      <li>
        <strong>Dán cookie thủ công</strong> (khi cách 1 không được): mở trang server trong
        trình duyệt và đăng nhập Steam ở đó → bấm <strong>F12</strong> (hoặc chuột phải →
        Inspect) → tab <strong>Application</strong> (Chrome) / <strong>Storage</strong>
        (Firefox) → <strong>Cookies</strong> → chọn domain server → bấm cookie tên
        <strong>islepilot_player</strong> → copy toàn bộ <strong>Value</strong> → dán vào ô
        trong app → <strong>Kiểm tra &amp; lưu cookie</strong>. Giữ bí mật chuỗi này như mật
        khẩu.
      </li>
    </ol>
    <p>
      Server có <strong>live map</strong> thì app tự nhận và bật "lấy vị trí tự động" — khỏi
      cần copy tọa độ thủ công; server tắt live map thì tùy chọn này tự khóa.
    </p>
    <p><strong>Một số server dùng IslePilot</strong> (tham khảo — mọi server chạy IslePilot đều dùng được):</p>
    <ul>
      <li>https://mixi.islepilot.eu</li>
      <li>https://hoho.islepilot.eu</li>
      <li>https://sdvn.islepilot.eu</li>
      <li>https://sdvn2.islepilot.eu</li>
      <li>https://khunglong.islepilot.eu</li>
      <li>https://islepilot.eu/p/sbtcisland</li>
    </ul>
  {:else}
    <h1>How to use — the important part</h1>
    <p>
      The app <strong>does not know where you are by itself</strong>. You tell it, one
      copy at a time:
    </p>
    <ol>
      <li>
        Run the game in <strong>Windowed</strong> or
        <strong>Borderless Fullscreen</strong> mode (in-game Settings › Video).
      </li>
      <li>In game, press <strong>Tab</strong> to open the character screen.</li>
      <li>Click <strong>"Asset Location"</strong> in the top-right corner.</li>
      <li>Your position appears on the map instantly.</li>
    </ol>
    <p>
      Copy whenever you reach somewhere worth remembering — the copies chain into your
      travelled path.
    </p>

    <h2>Why can't it be automatic?</h2>
    <p>
      Making the map follow you continuously would require
      <strong>reading game memory</strong> or <strong>pressing keys for you</strong> — both
      exactly what Easy Anti-Cheat is designed to detect, with an
      <strong>account ban</strong> as the consequence.
    </p>
    <p>
      This app only reads the Windows clipboard — content the game itself hands over. It
      never touches the game process. That is why it is safe, and also why it cannot be
      truly "live". This is a deliberate trade-off.
    </p>

    <h2>How heading works</h2>
    <p>
      Heading is computed automatically but needs <strong>two coordinate copies</strong> at
      least <strong>20 metres</strong> apart. The arrow shows
      <strong>where you just travelled</strong>, not where you are facing — turning on the spot
      is invisible to it. Samples older than <strong>10 minutes</strong> expire and the readout
      returns to <em>"Heading unknown"</em> rather than point wrong.
    </p>

    <h2>Full map</h2>
    <ul>
      <li><strong>Mouse wheel</strong>: zoom · <strong>left-drag</strong>: pan</li>
      <li><strong>Right-click</strong>: drop a named waypoint</li>
      <li><strong>Right panel</strong>: layer toggles, position status, waypoint list</li>
    </ul>

    <h2>Minimap</h2>
    <p>
      Circular, pinned to a corner of the game window and
      <strong>only shown while the game is running</strong>. <strong>N</strong> (amber) =
      North · <strong>E</strong> · <strong>S</strong> · <strong>W</strong>. The map
      <strong>never rotates</strong> — north stays up. The
      <strong>yellow dart/disc</strong> with the dark-and-white outline is you, always fully
      opaque however faded the map is.
    </p>
    <p>
      <strong>Click-through</strong> is on by default — clicks land in the game, never
      blocking play. While the full map is open the minimap hides itself and returns when
      the full map closes.
    </p>
    <p>
      <strong>Note:</strong> no overlay can draw over
      <strong>exclusive Fullscreen</strong> — a Windows limitation. Use Windowed or
      Borderless Fullscreen.
    </p>

    <h2>Your Dino (IslePilot)</h2>
    <p>
      The <strong>Dino</strong> tab reads your own dino's stats (growth, health, hunger,
      thirst, Prime progress) from the server's IslePilot panel. Two ways to connect — pick
      one:
    </p>
    <ol>
      <li>
        <strong>Steam login (fastest):</strong> enter the server link → click
        <strong>Steam login</strong> → sign in in the window that opens. Done.
      </li>
      <li>
        <strong>Paste the cookie manually</strong> (when method 1 fails): open the server
        page in your browser and sign in with Steam there → press <strong>F12</strong> (or
        right-click → Inspect) → <strong>Application</strong> tab (Chrome) /
        <strong>Storage</strong> (Firefox) → <strong>Cookies</strong> → pick the server's
        domain → click the <strong>islepilot_player</strong> cookie → copy the whole
        <strong>Value</strong> → paste it into the app →
        <strong>Verify &amp; save cookie</strong>. Treat this string like a password.
      </li>
    </ol>
    <p>
      If the server runs a <strong>live map</strong>, the app detects it and enables
      automatic position — no manual coordinate copying needed; when the server has the
      live map disabled the option locks itself off.
    </p>
    <p><strong>Some servers using IslePilot</strong> (examples — any IslePilot-powered server works):</p>
    <ul>
      <li>https://mixi.islepilot.eu</li>
      <li>https://hoho.islepilot.eu</li>
      <li>https://sdvn.islepilot.eu</li>
      <li>https://sdvn2.islepilot.eu</li>
      <li>https://khunglong.islepilot.eu</li>
      <li>https://islepilot.eu/p/sbtcisland</li>
    </ul>
  {/if}

  <h2>{$t("settings.hotkeys")}</h2>
  {#if settings}
    <table class="w-full border-collapse">
      <tbody>
        {#each ACTIONS as action (action)}
          <tr class="border-b" style="border-color: var(--color-border)">
            <td class="py-1 pr-4 font-mono text-xs">
              {(settings.hotkeys as Record<string, string>)[action] ?? "—"}
            </td>
            <td>{$t(`hotkey.${action}` as never)}</td>
          </tr>
        {/each}
      </tbody>
    </table>
  {/if}

  <h2>{$t("credits.title")}</h2>
  <p style="color: var(--color-muted)">{$t("credits.body")}</p>
</div>

<style>
  .guide :global(h1) {
    color: var(--color-accent);
    font-size: 1.15rem;
    font-weight: 600;
    margin: 0 0 0.5rem;
  }
  .guide :global(h2) {
    color: var(--color-accent);
    font-size: 1rem;
    font-weight: 600;
    margin: 1.25rem 0 0.4rem;
  }
  .guide :global(p),
  .guide :global(ul),
  .guide :global(ol) {
    margin: 0 0 0.6rem;
  }
  .guide :global(ol),
  .guide :global(ul) {
    padding-left: 1.4rem;
  }
  .guide :global(ol) {
    list-style: decimal;
  }
  .guide :global(ul) {
    list-style: disc;
  }
</style>
