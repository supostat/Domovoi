<!--
  FIX-DIAGRAM-BACKDROP.md — дельта к docs/SPEC-DOMOVOI.md: кадр диаграммы в живом окне заливается surface, а панель
  под ним — bg, и вокруг кольца виден квадрат. 1 фаза.

  ДИАГНОЗ (/mneme:fix от 07.11.2026, выбрана гипотеза 1 по рекомендации): DiagramPainter заливает кадр palette.surface,
  панели, где стоит диаграмма, залиты tokens.bg; снимки подменяли заливку на bg и квадрат не показывали. Отвергнута
  гипотеза 2 — перекрасить панели в surface по книге: меняет все экраны и проверку окна в theme_pixels.
-->

# Baseline

- Предыдущая спека: `docs/SPEC-DOMOVOI.md`; все дельты из `docs/` применены, последняя — `docs/FIX-PURGE-DRIFT.md`
  (`d1f3519` в `main`).
- Prior spec-hash: sha256:<placeholder — confirm>
- Проблема. На экранах 02 «Сканирование» и 03 «Главное окно» живого окна область диаграммы (MTKView) залита
  `surface`, панель вокруг — `bg`: вокруг кольца виден квадрат (тёмная тема `#1C2130` на `#11141D`, светлая
  `#FFFFFF` на `#ECEEF3`). Так с коммитов `e417e24` и `96d8267`.
- Уже есть и не меняется: панели окна залиты `bg` (`crates/app/src/window_frame.rs`, проверка окна в
  `crates/app/tests/theme_pixels.rs`); центральный диск — `surface_2`; слой диаграммы помечен sRGB.
- Вне этой спеки: цвет панели в книге (`.win` — `--surface`) расходится с приложением (`bg`) — решение обо всех
  экранах, не о диаграмме.
- Версия: фазы повышения версии нет — `cargo xtask release` сам инкрементирует патч.

# Stack

- `crates/render/src/diagram_painter.rs`, `empty_scene`: `clear` — `palette.bg` вместо `palette.surface`.
- `crates/app/src/shell_snapshot.rs`, `write_snapshot`: подмена `scene.clear = tokens.bg.premultiplied()`
  удаляется — снимок и живой вид заливают кадр одной функцией.
- `crates/render/tests/golden/*.png`: восемь эталонов пересобираются, меняется только фон кадра.
- `crates/app/tests/diagram_backdrop.rs` (написан диагнозом): остаётся регрессионным тестом.

# Conventions

- Один способ: заливку кадра решает painter, хост её не переопределяет; параметра «фон диаграммы» у `DiagramState`
  нет — все хосты диаграммы стоят на `bg`, параметр с одним значением — лишняя вариативность.
- Комментарии не ссылаются на спеку и фазы (`cargo xtask check-comments`).
- Typecheck-критерий — `cargo clippy -p domovoi-app -p domovoi-render --all-targets -- -D warnings`; формат —
  `cargo fmt --all -- --check` (заметка 7bbc2699).

# Knowledge

- **Красный прогон.** `cargo test -p domovoi-app --test diagram_backdrop`:
    panicked at crates/app/tests/diagram_backdrop.rs:33:9:
    assertion `left == right` failed: dark: the scanning diagram's frame shows the pane background around the ring
      left: [0.10980392, 0.12941177, 0.1882353, 1.0]
     right: [0.06666667, 0.078431375, 0.11372549, 1.0]
- **Симптом.** Скриншот экрана сканирования бандла, тёмная тема, профиль дисплея MN272U → sRGB: панель #181a24,
  квадрат #252739, центральный диск #2f3144 — все три сдвинуты профилем одинаково, разница тонов — разница токенов
  bg, surface, surface_2.
- **Причина (гипотеза 1, выбрана).** Живой вид строит кадр через `DiagramState::scene` без изменений
  (`diagram_drawer.rs:46`), painter заливает его `palette.surface` (`diagram_painter.rs:240`); панель под
  диаграммой — `tokens.bg` (`window_frame.rs:51`). Снимки подменяли заливку на `bg` (`shell_snapshot.rs:81`),
  поэтому снимки 02 и 03 и `theme_pixels` квадрата не показывали.
- **Гипотеза 2 (отвергнута).** Перекрасить панели в `surface`, как `.win` в книге (`domovoi-design-book.html:251`):
  меняет цвет всех экранов (`access_screen.rs:63`, `purge_screen.rs:37`, `tray_screen.rs:54`) и проверку окна в
  `theme_pixels`.
- **Цветовое пространство не причина.** Заметка f3493482: цель BGRA8Unorm со слоем sRGB, цвета theme пишутся как
  есть; color_parity сходится ±1.
- **Эталоны рендера.** `golden_offscreen` рисует сцены painter'а с его заливкой; локально сравнение с допуском 24 на
  канал — разница фона до 19 пройдёт незамеченной; ночной CI сравнивает точно (`DOMOVOI_GOLDEN_EXACT` в `ci.yml`),
  поэтому эталоны пересобираются в фазе, а гейт сравнивает точно. Тест текста
  `atlas_text_matches_core_text_line_drawing` задаёт заливку сам и не меняется.
- **Предел проверки.** Регрессия проверяет заливку сцены, которую получает живой вид, а не пиксели MTKView на экране;
  вид окна бандла остаётся приёмкой человеком.

# Gameplan

<!-- 1 фаза; гейт — регрессионный тест заливки, точные эталоны рендера, цвет окна на снимке. -->

### Phase 1: diagram-backdrop — кадр диаграммы залит фоном панели

- [ ] `diagram_painter.rs`, `empty_scene`: заливка кадра — `palette.bg`.
- [ ] `shell_snapshot.rs`, `write_snapshot`: удалить подмену `scene.clear`.
- [ ] Пересобрать эталоны `crates/render/tests/golden` с `DOMOVOI_GOLDEN_UPDATE=1` и прочитать один тёмный и один
  светлый снимок: изменился только фон кадра.
- [ ] Оставить `crates/app/tests/diagram_backdrop.rs` регрессионным тестом.

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-app --test diagram_backdrop
```
- Кадр живой диаграммы на экранах 02 и 03 залит фоном панели в обеих темах (exit 0).
```
env DOMOVOI_GOLDEN_EXACT=1 cargo test -p domovoi-render --test golden_offscreen
```
- Эталоны рендера совпадают с кадром побайтно (exit 0).
```
cargo test -p domovoi-app --test theme_pixels
```
- Окно, боковая панель и акцентная кнопка снимка — прежние токены (exit 0).
```
cargo clippy -p domovoi-app -p domovoi-render --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).
```
cargo fmt --all -- --check
```
- Формат совпадает с rustfmt (exit 0).
