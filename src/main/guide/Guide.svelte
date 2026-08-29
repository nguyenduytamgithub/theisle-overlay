<script lang="ts">
  // In-app usage guide, adapted from the original HUONG_DAN.md for the
  // installer-based app (no Python steps). Static prose per language; the
  // hotkey table reads the user's live bindings.
  import { onMount } from "svelte";
  import { getSettings, listenerBag, onSettingsChanged, type Settings } from "$lib/api";
  import { locale, t } from "$lib/i18n";
  import guideDino1 from "../../assets/guide-dino-1.jpg";
  import guideDino2 from "../../assets/guide-dino-2.jpg";
  import guideDino3 from "../../assets/guide-dino-3.jpg";

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
    "toggle_night_vision",
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

    <h2>Bản đồ lớn</h2>
    <ul>
      <li><strong>Lăn chuột</strong>: phóng to / thu nhỏ · <strong>kéo chuột trái</strong>: di chuyển</li>
      <li><strong>Bấm chuột phải</strong>: cắm điểm đánh dấu và đặt tên</li>
      <li><strong>Cột bên phải</strong>: bật/tắt từng lớp, trạng thái vị trí, danh sách điểm đánh dấu</li>
    </ul>

    <h2>Bản đồ nhỏ</h2>
    <p>
      Hình tròn, bám theo một góc cửa sổ game và <strong>chỉ hiện khi bạn đang trong game</strong>
      (Alt-Tab ra ngoài là tự ẩn, quay lại là hiện ngay).
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

    <h2>Nhìn đêm</h2>
    <p>
      Khi trời tối, bấm nút <strong>NHÌN ĐÊM</strong> ở góc trên bên phải hoặc nhấn
      <strong>Ctrl+Alt+N</strong>. Chế độ này chỉ chỉnh đường cong sáng của đúng màn hình
      đang chứa game; không đọc bộ nhớ, không chụp hình, không hook và không tự bấm phím.
    </p>
    <p>
      Alt-Tab, tắt chế độ, đổi màn hình hoặc thoát ứng dụng đều tự phục hồi màu gốc. Nếu
      màn hình đang bật HDR hoặc driver từ chối chỉnh gamma, nút sẽ báo
      <strong>KHÔNG HỖ TRỢ</strong> thay vì giả vờ đã bật. Có thể chỉnh cường độ và ẩn nút
      trong tab Cài đặt.
    </p>

    <h2>Khủng long của bạn (IslePilot)</h2>
    <p>
      Tab <strong>Khủng long</strong> đọc chỉ số dino của chính bạn (growth, máu, đói, khát,
      thể lực, dinh dưỡng, Prime progress) từ hệ thống IslePilot. Có <strong>2 cách</strong>
      kết nối:
    </p>
    <h3>Cách 1 — Đăng nhập Steam qua IslePilot (khuyên dùng)</h3>
    <ol>
      <li>Mở tab <strong>Khủng long</strong> → bấm nút <strong>Đăng nhập Steam</strong>.</li>
      <li>
        Cửa sổ đăng nhập của <strong>islepilot.eu</strong> hiện ra — đăng nhập Steam trong
        đó. Cửa sổ tự đóng khi xong.
      </li>
    </ol>
    <p>
      Chỉ cần làm <strong>1 lần duy nhất</strong> — không cần nhập link server, dùng được cho
      <strong>mọi server IslePilot</strong>: đổi server trong game là dữ liệu tự đổi theo.
      Cách này còn mở thêm <strong>Garage (Gacha)</strong> và lớp <strong>POI server</strong>
      trên bản đồ. Nếu cửa sổ không tự bắt được token, mở mục
      <em>"Hoặc dán token thủ công"</em> và dán token (hoặc nguyên link
      <strong>theisle-overlay://…</strong>) vào → <strong>Kiểm tra &amp; lưu token</strong>.
    </p>
    <h3>Cách 2 — Cách cũ: nhập server + cookie</h3>
    <p>
      Chỉ dùng khi cách 1 không hoạt động. Cookie lưu riêng cho từng server — đổi server là
      phải làm lại.
    </p>
    <ol>
      <li>
        Mở mục <strong>"Cách cũ: nhập server + cookie"</strong> trong phần đăng nhập, nhập
        link server (vd https://mixi.islepilot.eu) → bấm <strong>Đăng nhập Steam</strong>
        trong mục đó.
      </li>
      <li>
        Nếu vẫn không được, dán cookie thủ công: mở trang server trong trình duyệt và đăng
        nhập Steam ở đó → bấm <strong>F12</strong> (hoặc chuột phải → Inspect) → tab
        <strong>Application</strong> (Chrome) / <strong>Storage</strong> (Firefox) →
        <strong>Cookies</strong> → chọn domain server → bấm cookie tên
        <strong>islepilot_player</strong> → copy toàn bộ <strong>Value</strong> → dán vào ô
        trong app → <strong>Kiểm tra &amp; lưu cookie</strong>.
        <img src={guideDino1} alt="Chuột phải → Inspect hoặc F12, chọn tab Application" loading="lazy" />
        <img src={guideDino2} alt="Cookies → islepilot_player → copy toàn bộ Value" loading="lazy" />
        <img src={guideDino3} alt="Nhập link server, dán cookie vào app và lưu" loading="lazy" />
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

    <h2>Full map</h2>
    <ul>
      <li><strong>Mouse wheel</strong>: zoom · <strong>left-drag</strong>: pan</li>
      <li><strong>Right-click</strong>: drop a named waypoint</li>
      <li><strong>Right panel</strong>: layer toggles, position status, waypoint list</li>
    </ul>

    <h2>Minimap</h2>
    <p>
      Circular, pinned to a corner of the game window and
      <strong>only shown while you are in the game</strong> (it hides when you Alt-Tab away
      and returns the moment the game is focused again). <strong>N</strong> (amber) =
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

    <h2>Night Vision</h2>
    <p>
      In dark scenes, click the <strong>NIGHT VISION</strong> button in the top-right or
      press <strong>Ctrl+Alt+N</strong>. Windows Magnification locally redraws displayed
      screen pixels with a verified contrast transform. It does not read game memory,
      inject or hook code, synthesize input, or send images over the network.
    </p>
    <p>
      Use <strong>Windowed</strong> or <strong>Borderless Fullscreen</strong>. Alt-Tab,
      switching it off, or quitting removes the native magnifier. If Windows rejects the
      requested transform or cleanup cannot be verified, the button truthfully reports
      <strong>UNAVAILABLE</strong>. Strength and button visibility are adjustable in Settings.
    </p>

    <h2>Your Dino (IslePilot)</h2>
    <p>
      The <strong>Dino</strong> tab reads your own dino's stats (growth, health, hunger,
      thirst, stamina, nutrition, Prime progress) from the IslePilot system. There are
      <strong>2 ways</strong> to connect:
    </p>
    <h3>Method 1 — Steam login via IslePilot (recommended)</h3>
    <ol>
      <li>Open the <strong>Dino</strong> tab → click <strong>Steam login</strong>.</li>
      <li>
        An <strong>islepilot.eu</strong> sign-in window opens — sign in with Steam there.
        It closes itself when done.
      </li>
    </ol>
    <p>
      Do this <strong>once</strong> — no server link needed, and it works on
      <strong>every IslePilot server</strong>: switch servers in game and the data follows.
      This login also unlocks the <strong>Garage (Gacha)</strong> and the
      <strong>server POI</strong> map layer. If the window fails to catch the token, open
      <em>"Or paste the token manually"</em> and paste the token (or the whole
      <strong>theisle-overlay://…</strong> link) → <strong>Verify &amp; save token</strong>.
    </p>
    <h3>Method 2 — Legacy: server link + cookie</h3>
    <p>
      Only when method 1 does not work. The cookie is stored per server — switching servers
      means doing it again.
    </p>
    <ol>
      <li>
        Open the <strong>"Legacy: server URL + cookie"</strong> section of the login card,
        enter the server link (e.g. https://mixi.islepilot.eu) → click
        <strong>Steam login</strong> inside that section.
      </li>
      <li>
        If that still fails, paste the cookie manually: open the server page in your
        browser and sign in with Steam there → press <strong>F12</strong> (or right-click →
        Inspect) → <strong>Application</strong> tab (Chrome) / <strong>Storage</strong>
        (Firefox) → <strong>Cookies</strong> → pick the server's domain → click the
        <strong>islepilot_player</strong> cookie → copy the whole <strong>Value</strong> →
        paste it into the app → <strong>Verify &amp; save cookie</strong>.
        <img src={guideDino1} alt="Right-click → Inspect or F12, open the Application tab" loading="lazy" />
        <img src={guideDino2} alt="Cookies → islepilot_player → copy the whole Value" loading="lazy" />
        <img src={guideDino3} alt="Enter the server link, paste the cookie and save" loading="lazy" />
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
  .guide :global(h3) {
    color: var(--color-text);
    font-size: 0.95rem;
    font-weight: 600;
    margin: 0.9rem 0 0.3rem;
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
  .guide :global(img) {
    max-width: 100%;
    border: 1px solid var(--color-border);
    border-radius: 6px;
    margin: 0.5rem 0;
    display: block;
  }
</style>
