<!--
  FIX-DUPLICATE-PREVIEW.md — дельта к docs/SPEC-DOMOVOI.md: пробел на строке дубликата из домашней папки не открывает
  Quick Look. 1 фаза.

  ДИАГНОЗ (/mneme:fix от 09.11.2026, гипотеза 1): PreviewTargets::from_duplicates строит путь из MemberModel.path —
  строки для показа с «~», которую NSURL::fileURLWithPath не разворачивает; правка — путь из индекса по узлу
  участника, как у списка уровня.
-->

# Baseline

- Предыдущая спека: `docs/SPEC-DOMOVOI.md`; все дельты из `docs/` применены, последняя —
  `docs/FIX-DUPLICATE-SEARCH-ERROR.md` (`8800642` в `main`).
- Prior spec-hash: sha256:<placeholder — confirm>
- Проблема. В «Дубликатах» выделена строка копии из домашней папки (`~/Downloads/…`), пробел — Quick Look не
  показывает файл. Пробел на строке уровня главного экрана открывает тот же файл. Ожидается предпросмотр файла из
  списка дубликатов.
- Уже есть и не меняется: `MemberModel.path` — строка для показа с «~» (подпись строки, подсказка, перенос отметок в
  `shell_state.rs`); выбор целей по ключам строк (`DuplicateKey`: заголовок группы не цель, участник 0 —
  оставляемый, n — `copies[n-1]`); `PreviewSource`, панель и цепочка респондеров; список уровня.
- Версия: фазы повышения версии нет — `cargo xtask release` сам инкрементирует патч.

# Stack

- `crates/app/src/preview_targets.rs`: `PreviewTargets::from_duplicates(groups, index, keys)` — путь участника
  `index.path(node)` по `MemberModel::node`, как `from_level` берёт путь по `NodeOrigin::Indexed`; участник без узла
  (модель из фикстуры) целью не становится.
- `crates/app/src/shell_controller.rs`, `preview_targets`: передаёт индекс сессии в `from_duplicates`.
- `crates/app/tests/preview_targets.rs`: тест `duplicate_targets_come_from_member_rows_only` строит участников с
  узлами реального индекса и ждёт `index.path(node)`; правило «заголовок не цель, участники — цели» остаётся.
- `crates/app/tests/duplicate_preview.rs` (написан диагнозом): регрессия — копии в папке внутри домашней дают
  Quick Look файлы на диске.

# Conventions

- Путь для системы (Quick Look, Finder) берётся из индекса по узлу; строка с «~» — только для показа.
- Комментарии не ссылаются на спеку и фазы (`cargo xtask check-comments`).
- Typecheck-критерий — `cargo clippy -p domovoi-app --all-targets -- -D warnings`; формат —
  `cargo fmt --all -- --check`.

# Knowledge

- **Красный прогон.** `cargo test -p domovoi-app --test duplicate_preview` — 0 passed, 1 failed:
    a_member_in_the_home_folder_previews_its_file_on_disk (duplicate_preview.rs:43:5):
      Quick Look gets the files on disk, not the shown paths
      left: ["~/domovoi-tree-yF1DXV/Documents/report.pdf", "~/domovoi-tree-yF1DXV/Downloads/report copy.pdf"]
      right: ["/Users/ingvar/domovoi-tree-yF1DXV/Documents/report.pdf", "/Users/ingvar/domovoi-tree-yF1DXV/Downloads/report copy.pdf"]
- **Причина (гипотеза 1, подтверждена).** `DuplicatesModel::found` пишет в `MemberModel.path`
  `tilde(&index.path(…))` (`duplicates_model.rs:38`, `:55`); `from_duplicates` делает из неё
  `PathBuf::from(&member.path)` (`preview_targets.rs:37`); `NSURL::fileURLWithPath` (`preview_source.rs:56`) «~» не
  разворачивает и считает путь относительным к рабочему каталогу процесса — файла там нет. Список уровня берёт
  `index.path(id)` (`preview_targets.rs:20`), поэтому там предпросмотр работает. Других потребителей `member.path`
  как пути на диске нет: «Показать в Finder» берёт путь по узлу (`shell_controller.rs:1455-1460`).
- **Почему старый тест зелёный.** `duplicate_targets_come_from_member_rows_only` (`tests/preview_targets.rs:91-128`)
  задаёт участникам абсолютные пути `/Users/igor/…`, которых `found` для файлов из домашней папки не выдаёт.
- **Отвергнуто.** Гипотеза 2 — панель не получает управление в окне «Дубликаты»: красный прогон показывает неверные
  пути независимо от панели, цепочка респондеров та же, что у списка уровня (`ShellRootView`). Рассинхрон ключа
  строки и участника — ключи `0.0`/`0.1` дали ровно два нужных файла. Вариант «абсолютный путь в `MemberModel`,
  «~» при отрисовке» ломает единый приём моделей: `recommendations_model`, `tray_model`, `chat_model` хранят строки
  для показа.
- **Тест в домашней папке.** `tilde` читает `std::env::home_dir()`, поэтому репро создаёт `~/domovoi-tree-XXXXXX`
  через `TempTree::under` и удаляет её в Drop (после красного прогона папки нет).
- **Не входит.** Шаг `--e2e` для предпросмотра дубликатов: шаг `preview_targets` проверяет только список уровня, а
  панель Quick Look по пробелу — приёмка человеком (заметка c11b5728). `docs/DOMOVOI.md` уже обещает Quick Look в
  списке дубликатов — текст не меняется.
- **Проверка мутацией.** Вернуть `PathBuf::from(&member.path)` в `from_duplicates` — `duplicate_preview` краснеет.

# Gameplan

<!-- 1 фаза: предпросмотр дубликата берёт путь на диске из индекса. -->

### Phase 1: duplicate-preview — Quick Look в «Дубликатах» открывает файлы из домашней папки

- [ ] `PreviewTargets::from_duplicates(groups, index, keys)` с путём `index.path(node)` по узлу участника; вызов в
  `ShellController::preview_targets` передаёт индекс сессии.
- [ ] Переписать `duplicate_targets_come_from_member_rows_only` на участников с узлами реального индекса.
- [ ] Оставить `crates/app/tests/duplicate_preview.rs` регрессионным тестом: вызов принимает индекс, утверждения не
  меняются; мутация по Knowledge.

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-app --test duplicate_preview
```
- Копии в домашней папке дают Quick Look файлы на диске (exit 0).
```
cargo test -p domovoi-app --test preview_targets
```
- Цели списка уровня и строк дубликатов: заголовок не цель, участники — пути индекса (exit 0).
```
cargo clippy -p domovoi-app --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).
```
cargo fmt --all -- --check
```
- Формат совпадает с rustfmt (exit 0).
