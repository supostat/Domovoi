<!--
  FIX-BUNDLE-FOLDER-TIME.md — дельта к docs/SPEC-DOMOVOI.md: cargo xtask bundle поверх уже собранного бандла
  оставляет папке DOMOVOI.app прежнее время изменения, и система держит прежнюю иконку. 1 фаза.

  ДИАГНОЗ (/mneme:fix от 04.11.2026, выбрана гипотеза 1 по рекомендации): сборка не сдвигает время папки .app, а
  LaunchServices по нему решает, менялся ли бандл. Отвергнута гипотеза 2 — перерегистрация через lsregister -f в конце
  сборки: запись LaunchServices несла CFBundleIconFile = AppIcon ещё до ручного вмешательства, устаревшей была
  картинка, а время папки такой вызов не сдвигает.
-->

# Baseline

- Предыдущая спека: `docs/SPEC-DOMOVOI.md`; все дельты из `docs/` применены, последняя — `docs/SPEC-APP-ICON.md`
  (влита в `main`, `7941a25`).
- Prior spec-hash: sha256:<placeholder — confirm>
- Проблема. `assemble` (`crates/xtask/src/bundle.rs:51-62`) пишет файлы в уже существующие папки бандла
  (`copy_into` через `fs::copy`, `iconutil`, `codesign`) и не трогает корень `target/bundle/DOMOVOI.app`, поэтому
  время изменения папки остаётся от первой сборки. После пересборки с новой иконкой Dock, экран доступа
  (`applicationIconImage`) и список «Полный доступ к диску» показывали заглушку macOS, пока папке вручную не сделали
  `touch` и не перезапустили приложение.
- Уже есть и не меняется: порядок сборки (исполняемые, `Info.plist`, ресурсы с `AppIcon.icns`, Sparkle, подпись),
  `cargo xtask check-bundle`, набор `assets/icon/AppIcon.iconset`.
- Вне этой спеки: перерегистрация через `lsregister -f` из сборки (отвергнутая гипотеза); строка о ручном `touch` в
  Knowledge `docs/SPEC-APP-ICON.md` остаётся историей той спеки.
- Версия: фазы повышения версии нет — `cargo xtask release` сам инкрементирует патч.

# Stack

- `crates/xtask/src/bundle.rs`: последний шаг `assemble`, после `sign`, — `mark_bundle_changed(&layout.app)`: открыть
  папку `.app` через `fs::File::open` и выставить ей время изменения «сейчас» через
  `File::set_modified(SystemTime::now())`.
- `crates/app/tests/bundle_folder_time.rs` (новый, написан диагнозом): читает `target/bundle/DOMOVOI.app` и требует,
  чтобы папка была не старше `Contents/Info.plist`, `Contents/Resources/AppIcon.icns` и `Contents/MacOS/domovoi`.

# Conventions

- Один тип — один файл; имена без сокращений; комментарии не ссылаются на спеку и фазы (`cargo xtask check-comments`).
- Время папки сдвигается после подписи: подпись переписывает исполняемый файл, и папка должна быть не старше его.
- Typecheck-критерий — `cargo clippy -p xtask --all-targets -- -D warnings`.

# Knowledge

- **Красный прогон.** Тест был зелёным после ручного `touch` в 09:40; `cargo xtask bundle --profile adhoc` поверх
  существующего бандла оставил папку на 09:40 и записал файлы в 09:50:
    09:40:20 target/bundle/DOMOVOI.app
    09:50:09 target/bundle/DOMOVOI.app/Contents/Info.plist
    09:50:09 target/bundle/DOMOVOI.app/Contents/Resources/AppIcon.icns
    09:50:10 target/bundle/DOMOVOI.app/Contents/MacOS/domovoi
    panicked at crates/app/tests/bundle_folder_time.rs:29:9:
    …/target/bundle/DOMOVOI.app is older than …/DOMOVOI.app/Contents/Info.plist
    test result: FAILED. 0 passed; 1 failed
- **Запись LaunchServices.** `lsregister -dump` после пересборки: `mod date` бандла равен времени папки, хотя
  содержимое уже новее — время папки не сдвинулось вслед за файлами:
    mod date:                   2026-11-04 09:40 (POSIX 1793785220)
    reg date:                   2026-11-04 09:41 (POSIX 1793785280)
    icons:                      Contents/Resources/AppIcon.icns
- **Подтверждённая гипотеза.** До первого ручного `touch` папка держала 05:59 при `AppIcon.icns` от 09:13, и Dock
  показывал заглушку, хотя запись LaunchServices уже несла `CFBundleIconFile = AppIcon` — `Info.plist` система
  перечитала, устаревшей осталась картинка. После `touch`, `lsregister -f` и перезапуска Dock показал розетку; эти два
  изменения сделаны вместе и не разведены — вывод, что решает время папки, не проверен отдельным прогоном.
- **Отвергнутая гипотеза.** Вызов `lsregister -f` в конце сборки: регистрация при запуске и так проходила, время папки
  он не сдвигает, и регрессионный тест на нём остаётся красным.
- **Подпись.** Сдвиг времени папки после подписи не должен ломать `codesign --verify --deep --strict` — это проверяет
  критерий `cargo xtask check-bundle`; если подпись ломается, это находка фазы, а не повод переносить сдвиг до подписи.
- **Предел проверки.** Тест доказывает, что время папки движется при каждой сборке; что Dock и Настройки после этого
  берут новую картинку — наблюдение диагноза, автоматически не проверяется.

# Gameplan

<!-- 1 фаза; гейт — красный тест диагноза на пересобранном поверх бандле. -->

### Phase 1: bundle-folder-time — сборка сдвигает время папки DOMOVOI.app

- [ ] `bundle.rs`: `mark_bundle_changed` последним шагом `assemble`, после `sign`, по Stack.
- [ ] Оставить `crates/app/tests/bundle_folder_time.rs` регрессионным тестом без ослабления проверок.

**Done when (EXECUTABLE):**
```timeout=900s
cargo xtask bundle --profile adhoc
```
- Бандл пересобирается поверх существующего (exit 0).
```
cargo test -p domovoi-app --test bundle_folder_time
```
- Папка DOMOVOI.app не старше файлов, которые записала сборка (exit 0).
```
cargo xtask check-bundle target/bundle/DOMOVOI.app
```
- Подписи и обязательные файлы целы после сдвига времени (exit 0).
```
cargo clippy -p xtask --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).
```
cargo clippy -p domovoi-app --test bundle_folder_time -- -D warnings
```
- Регрессионный тест чист по clippy (exit 0).
