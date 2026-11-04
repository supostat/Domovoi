<!--
  SPEC-APP-ICON.md — дельта к docs/SPEC-DOMOVOI.md: у приложения появляется своя иконка — «Восковая» розетка-оберег
  из design/icon; команда cargo xtask icon рисует из двух SVG набор PNG для iconset, CI сверяет его с исходниками,
  бандл собирает AppIcon.icns. 2 фазы.

  ГРАНИЦА РЕШЕНИЯ (веер от 04.11.2026, выбран вариант A по рекомендации): растеризация чистым Rust-крейтом resvg в
  xtask, сгенерированный артефакт в git и --check в CI — по образцу cargo xtask theme. Отвергнуты: B — rsvg-convert
  из Homebrew при каждой сборке бандла (внешняя зависимость сборки и релиза, CI бандл не собирает — поломка всплывёт
  на релизе, разные версии librsvg рисуют по-разному); C — один раз закоммиченный .icns (между SVG и .icns нет связи,
  правка рисунка — ручные неописанные шаги, расхождение никто не ловит). На ревью спеки уточнено: в git лежит набор
  PNG iconset, а .icns собирает системный iconutil при cargo xtask bundle — побайтовая сверка надёжна для PNG resvg,
  а повторяемость контейнера iconutil не проверена.
-->

# Baseline

- Предыдущая спека: `docs/SPEC-DOMOVOI.md`; все дельты из `docs/` применены, последняя — `docs/SPEC-TRAY-DRAG.md`
  (влита в `main`, `6b163cc`).
- Prior spec-hash: sha256:<placeholder — confirm>
- Проблема. У бандла нет своей иконки: в `crates/app/bundle/Info.plist` нет `CFBundleIconFile`, `place_resources`
  (`crates/xtask/src/bundle.rs`) кладёт в `Resources` только шрифты, лицензии, `catalog.json` и metallib — в Dock,
  Finder и на экране доступа (`applicationIconImage`, `crates/app/src/app_icon_drag.rs`) стоит заглушка macOS.
- Рисунок выбран пользователем и не меняется: «Восковая» розетка-оберег — `design/icon/domovoi-icon.svg` (полный) и
  `design/icon/domovoi-icon-small.svg` (упрощённый), холст 1024, скруглённый квадрат 824; файлы пока не в git.
- Уже есть и не меняется: сборка `cargo xtask bundle` (`bundle.rs`, `bundle_layout.rs`), проверка
  `cargo xtask check-bundle` (`check_bundle.rs`: обязательные файлы, ключи `Info.plist`, подписи), образец генератора с
  `--check` — `cargo xtask theme` (`crates/xtask/src/theme.rs`) и его шаг в `.github/workflows/ci.yml`.
- Вне этой спеки: формат Icon Composer (`.icon`, Liquid Glass macOS 26) — нужен `actool` Xcode 26, `Assets.car` и
  рисунок по слоям; поведение macOS 26 со старым `.icns` не проверено. Минимальная система — macOS 15, где `.icns` —
  штатный формат.
- Версия: фазы повышения версии нет — `cargo xtask release` сам инкрементирует патч; `.github/workflows/ci.yml`
  версию не меняет.
- Обоснование выбора — staged decision note (id после accept).

# Stack

- `Cargo.toml` (workspace): зависимость `resvg` — последняя версия crates.io на момент фазы, фиксируется
  `Cargo.lock`; подтягивает `usvg` и `tiny-skia`. Только в `crates/xtask`.
- `crates/xtask/src/icon_artwork.rs` (новый): `IconArtwork { Full, Small }`, путь исходника каждого рисунка.
- `crates/xtask/src/icon_slot.rs` (новый): `IconSlot { file_name: &'static str, pixels: u32, artwork: IconArtwork }`
  и таблица `ICON_SLOTS: [IconSlot; 10]`.
- `crates/xtask/src/icon.rs` (новый): `run(Mode)` — `Write` рисует набор в `assets/icon/AppIcon.iconset/`, `Check`
  рисует в памяти и сверяет с папкой; `render(artwork, pixels) -> Vec<u8>` (PNG); `check_iconset(directory)` —
  находки «нет файла», «отличается», «лишний файл».
- `crates/xtask/src/main.rs`: `icon` и `icon --check`, строка `USAGE`.
- `assets/icon/AppIcon.iconset/` (новый, сгенерированный): 10 PNG с именами `iconutil`.
- `crates/xtask/src/bundle_layout.rs`: `icon()` — `Resources/AppIcon.icns`; `bundle.rs`: `/usr/bin/iconutil --convert
  icns --output <icon()> assets/icon/AppIcon.iconset`.
- `crates/app/bundle/Info.plist`: `CFBundleIconFile` = `AppIcon`.
- `crates/xtask/src/check_bundle.rs`: обязательный файл `Resources/AppIcon.icns`, ключ `CFBundleIconFile` = `AppIcon`.
- `.github/workflows/ci.yml`: шаг `cargo xtask icon --check` рядом с `cargo xtask theme --check`.
- `README.md`, `docs/DOMOVOI.md`, `design/domovoi-design-book.html`: команда, источник иконки, образец рисунка.

# Conventions

- Один тип — один файл; имена без сокращений; комментарии не ссылаются на спеку и фазы (`cargo xtask check-comments`).
- Сгенерированное не правится руками: PNG набора пишет только `cargo xtask icon`, расхождение ловит `--check`.
- Геометрия и цвета SVG в `design/icon` не меняются.
- Typecheck-критерий — `cargo clippy -p xtask --all-targets -- -D warnings`.

# Knowledge

- **Слоты iconset.** Имена и пиксели, которых ждёт `iconutil`: `icon_16x16.png` 16, `icon_16x16@2x.png` 32,
  `icon_32x32.png` 32, `icon_32x32@2x.png` 64, `icon_128x128.png` 128, `icon_128x128@2x.png` 256, `icon_256x256.png`
  256, `icon_256x256@2x.png` 512, `icon_512x512.png` 512, `icon_512x512@2x.png` 1024. Слот в 32 px и меньше берёт
  упрощённый рисунок (первые три), остальные — полный: так решено при выборе рисунка.
- **Рендер.** SVG без текста, поэтому база шрифтов не нужна: `usvg` разбирает файл, `resvg` рисует в `tiny_skia`
  `Pixmap` размером со слот с масштабом `pixels / 1024`, PNG — `Pixmap::encode_png`. Точные имена API сверить с
  документацией крейта (context7) на версии из `Cargo.lock`.
- **Сверка.** `--check` сравнивает байты PNG: одна версия `resvg` из `Cargo.lock` и одна архитектура (arm64 у
  разработчика и у раннеров macOS в CI) дают одинаковые байты. Повторяемость `.icns` от `iconutil` не проверена —
  поэтому в git лежит набор PNG, а `.icns` собирается при `cargo xtask bundle`.
- **Правила `xtask icon` и случаи самопроверки.** Правила: (1) пишет ровно 10 PNG с именами `iconutil`; (2) слоты не
  больше 32 px — упрощённый рисунок, остальные — полный; (3) PNG ровно размера слота; (4) вне скруглённого квадрата
  прозрачно, в центре непрозрачно; (5) `--check` молчит, когда папка совпадает с рисунком; (6) `--check` называет
  отличающийся PNG; (7) `--check` называет недостающий PNG; (8) `--check` называет лишний файл в папке.
  Случаи: (1) → таблица слотов сверяется с перечнем имён и пикселей выше; (2) → рисунок каждого слота сверяется с
  правилом 32 px; (3) → PNG слотов 16 и 1024 декодируются и имеют эти размеры; (4) → угловой пиксель слота 1024 с
  альфой 0, центральный — 255; (5) → запись во временную папку и сверка без находок; (6) → один байт PNG во временной
  копии изменён — находка «отличается» с именем файла; (7) → один PNG удалён — находка «нет файла»; (8) → в папку
  добавлен посторонний файл — находка «лишний файл».
- **Сборка.** `iconutil` требует папку с суффиксом `.iconset` и точные имена слотов; `.icns` кладётся в `Resources`
  до подписи, имя без расширения уходит в `CFBundleIconFile`.
- **Предел проверки.** Вид иконки в Dock, Finder и на экране доступа автоматически не проверяется — это приёмка
  человеком на собранном бандле на границе фазы `wire`. Если Dock показывает старую заглушку, это кэш LaunchServices:
  `touch target/bundle/DOMOVOI.app` и перезапуск приложения.

# Gameplan

<!-- 2 фазы цепочкой; wire — бандл с иконкой и проверка на настоящем бандле. -->

### Phase 1: icon-assets — набор PNG иконки из SVG

- [ ] Добавить в git `design/icon/domovoi-icon.svg` и `design/icon/domovoi-icon-small.svg` без изменений.
- [ ] `resvg` в зависимости workspace и `crates/xtask`.
- [ ] `icon_artwork.rs`, `icon_slot.rs`, `icon.rs` по Stack и Knowledge; `icon` и `icon --check` в `main.rs`.
- [ ] Модульные тесты в `icon.rs` с именами, начинающимися на `icon_`, — все восемь случаев самопроверки из Knowledge.
- [ ] Сгенерировать `assets/icon/AppIcon.iconset/` командой `cargo xtask icon`.
- [ ] Шаг `cargo xtask icon --check` в `.github/workflows/ci.yml`; строки команды в таблице xtask `README.md`.

**Done when (EXECUTABLE):**
```
cargo test -p xtask icon
```
- Восемь случаев самопроверки `xtask icon` (exit 0).
```
cargo xtask icon --check
```
- Набор PNG в git совпадает с рисунком SVG (exit 0).
```
grep -q xtask.icon .github/workflows/ci.yml
```
- CI сверяет набор с исходниками (exit 0).
```
cargo clippy -p xtask --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).

### Phase 2: wire — иконка в собранном бандле (потребляет фазу 1: набор assets/icon/AppIcon.iconset)

- [ ] `bundle_layout.rs`: `icon()`; `bundle.rs`: `iconutil` собирает `Resources/AppIcon.icns` из набора до подписи.
- [ ] `crates/app/bundle/Info.plist`: `CFBundleIconFile` = `AppIcon`.
- [ ] `check_bundle.rs`: обязательный `Resources/AppIcon.icns` и ключ `CFBundleIconFile`.
- [ ] `docs/DOMOVOI.md`: одна строка — иконка, её источник в `design/icon` и генератор `cargo xtask icon`;
  `design/domovoi-design-book.html`: блок «Иконка» с обоими рисунками и правилом 32 px.

**Done when (EXECUTABLE):**
```timeout=900s
cargo xtask bundle --profile adhoc
```
- Бандл собирается с иконкой из настоящего набора фазы 1 (exit 0).
```
cargo xtask check-bundle target/bundle/DOMOVOI.app
```
- В бандле есть `AppIcon.icns`, `Info.plist` ссылается на него, подписи целы (exit 0).
```
cargo xtask icon --check
```
- Набор не разошёлся с рисунком (exit 0).
```
cargo xtask check-sources
```
- Документ и книга не расходятся с принятыми решениями (exit 0).
```
cargo clippy -p xtask --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).
