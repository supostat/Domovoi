<!--
  SPEC-DELETE-PROGRESS.md — дельта к docs/SPEC-DOMOVOI.md: удаление идёт вглубь через дескрипторы и считает файлы,
  поэтому полоса и счёт в листе «К удалению» движутся и внутри одной огромной папки, а «Остановить» срабатывает между
  файлами. 3 фазы.

  ГРАНИЦА РЕШЕНИЯ (веер от 28.10.2026, выбран вариант A по рекомендации): каждый ребёнок элемента удаляется обходом
  на дескрипторах (BulkListing + unlinkat), счёт — по файлам с итогом из индекса (IndexNode.files), полоса под
  строкой; это отложенный вариант C из SPEC-PERMANENT-DELETE за той же сигнатурой delete(). Отвергнуты: B — план на
  два уровня с removeItem (мало кода, но крупность счёта зависит от формы дерева, папка с объёмом на третьем уровне
  снова застынет, «Остановить» ждёт конца папки второго уровня); C — полоса по свободному месту тома (движок не
  меняется, но снимки Time Machine держат блоки — полоса стоит на нуле; чужие записи сбивают число; «Остановить»
  внутри папки по-прежнему не работает).
-->

# Baseline

- Предыдущая спека: `docs/SPEC-DOMOVOI.md`; все дельты из `docs/` применены, последняя — `docs/SPEC-ANSWER-TARGETS.md`
  (влита в `main`, `d32cc32`). Движок удаления описан в `docs/SPEC-PERMANENT-DELETE.md`; там же вариант C (обход на
  дескрипторах) отложен «за той же сигнатурой delete_run» — эта дельта его реализует.
- Prior spec-hash: sha256:<placeholder — confirm>
- Проблема. `delete_run.rs` считает единицей работы ребёнка верхнего уровня (`Planned::work` = дети + 1) и удаляет
  каждого одним `removeItemAtURL`. У `~/Library/pnpm/store/v3` 6 детей, почти весь объём — в `files` (256 папок,
  сотни тысяч файлов): лист стоит на «Удаляю: 5 из 7 элементов» минутами, полосы нет, а «Остановить» не действует
  до конца `files` — отмена проверяется только между детьми.
- Уже есть и не меняется: сигнатура `delete(subjects, cancel, progress) -> DeleteReport`, `DeleteReport` (`deleted`,
  `interrupted`, `untouched`, `failed`, `freed_bytes` — сумма `measured_bytes` удалённых), сверка `link_id` при
  плане, `DELETE_WORKERS = 4` на детей верхнего уровня, `DeleteFailure` и `from_io`, `Tray::take_for_delete`,
  `DeleteJob` с `keep_furthest`, такт `DELETE_TICK = 0.1` и пересборка листа на каждом такте, кнопка «Остановить»,
  `DeleteOutcome` и строка итога.
- Версия: фазы повышения версии нет — `cargo xtask release` (`crates/xtask/src/release.rs`) сам инкрементирует патч;
  `.github/workflows/ci.yml` версию не меняет.
- Typecheck-критерий проекта: `cargo clippy -p <crate> --all-targets -- -D warnings`.
- Обоснование выбора — staged decision note (id после accept).

# Stack

- `crates/apfs-sys/src/unlink_at.rs` (новый): `pub fn unlink_at(directory: BorrowedFd<'_>, name: &OsStr, flags:
  libc::c_int) -> io::Result<()>` над `unlinkat`; экспорт из `lib.rs` рядом с `open_at`.
- `crates/rules/src/delete_run.rs`:
  - план элемента — как сейчас (`symlink_metadata`, `link_id`), дети — из `BulkListing` дескриптора элемента;
  - ребёнок-каталог удаляется обходом в глубину: `open_at(.., O_RDONLY | O_DIRECTORY | O_NOFOLLOW)` → `BulkListing`
    → `unlink_at(fd, name, 0)` для файлов и ссылок, каталог — `unlink_at(parent, name, AT_REMOVEDIR)` после
    содержимого; в точки монтирования (`DIR_MNTSTATUS_MNTPOINT | TRIGGER`) и firmlink движок не заходит;
  - отмена проверяется перед каждым файлом; `Tally::step` — на каждый удалённый не-каталог;
  - `removeItemAtURL` уходит; `DeleteFailure::from_cocoa` и фичи `NSFileManager`/`NSURL` у `objc2-foundation`
    уходят, если у них не остаётся вызовов.
- `crates/rules/src/delete_progress.rs`: `DeleteProgress { files_done, files_total, subjects_done }`;
  `files_total` = сумма `indexed_files` спланированных элементов, но не меньше `files_done`.
- `crates/rules/src/delete_subject.rs`: поле `indexed_files: u64`; `Tray::take_for_delete` берёт его из
  `index.node(item.node).files`.
- `crates/xtask/src/check_sources.rs`: `REMOVAL_CALLS` пополняется `unlinkat(` и `unlink_at(`; разрешённые модули —
  `DELETE_MODULE` и `apfs-sys/src/unlink_at.rs`.
- `crates/app/src/progress_bar.rs` (новый): `PROGRESS_HEIGHT` и `progress_bar(marker, width, share, palette)` —
  вынос из `model_card_view.rs::progress` (дорожка `line_soft`, заливка `wax`); карточка модели и лист — его
  пользователи.
- `crates/app/src/delete_status.rs`: `DeleteStatus { files_done, files_total }`; `delete_run.rs` и `delete_job.rs`
  приложения переходят на новые поля.
- `crates/app/src/tray_model.rs`: `deleting_line` — «Удаляю: 120 000 из 412 000 файлов»; `deleting_share() ->
  Option<f64>` = `files_done / files_total`, `0.0` при нулевом итоге. `strings.rs`: `DELETE_UNITS` — «файла / файлов
  / файлов» (родительный после «из»).
- `crates/app/src/tray_screen.rs`: полоса во всю ширину колонки между строкой «Удаляю» и «Остановить».
- `crates/app/tests/fixtures/shell_state.json`: `tray_deleting` с новыми именами полей.
- `crates/app/src/end_to_end.rs`: `delete_recommendations` пишет `files_done`, `files_total`, `deleting_line` и
  `deleting_share` последнего состояния через настоящую `TrayModel::with_deleting`.
- `docs/DOMOVOI.md`: решение 5 и раздел «К удалению» — обход на дескрипторах, счёт по файлам, полоса;
  `design/domovoi-design-book.html` экран 08 — подпись состояния «удаление с полосой и счётом файлов».

# Conventions

- Удаляет файлы только `delete_run.rs`; `unlink_at` — единственная обёртка, и `check-sources` держит обе границы.
- Путь читается один раз — при плане; внутри элемента всё идёт от дескриптора родителя с `O_NOFOLLOW`: ссылка
  удаляется как ссылка, цель не трогается.
- Флаги и ACL не снимаются: `uchg`, `schg`, запрет ACL дают `EPERM`/`EACCES` → `DeleteFailure::NoPermission`, как у
  `removeItem`.
- Сбой одного файла не останавливает обход соседей; каталог, где что-то осталось, не удаляется, элемент — в
  `failed` с причиной первого сбоя. Файл, исчезнувший во время обхода (`ENOENT`), — не сбой.
- Полоса — токены темы (`line_soft`, `wax`), без литералов цвета; голос строки — без восклицаний.

# Knowledge

- Итог берётся из индекса, а не из второго обхода: `IndexNode.files` — число не-каталогов поддерева, он уже лежит
  рядом с `link_id`, который читает `take_for_delete`. Если на диске файлов больше, чем знает индекс, `files_total`
  дорастает до `files_done` — полоса не уходит за 100 %, строка не показывает «130 из 120».
- Каталоги не считаются: шаг — удалённый не-каталог. Пустые папки удаляются без шага; при нулевом итоге полоса
  пустая.
- Огромный ребёнок по-прежнему удаляется одним потоком (как и одним `removeItem` сейчас): скорость не растёт, но и
  не падает. Замер прежнего движка — 2,6 с на 100 000 файлов (`SPEC-PERMANENT-DELETE`).
- Причины прежнего отказа: бандл — обычный каталог для `unlinkat`; ACL и флаги — та же ошибка, что у `removeItem`
  (см. Conventions); скорость — приёмка человеком; второй крейт — одна обёртка в `apfs-sys`.
- Глубина держит по дескриптору на уровень у каждого из 4 работников; `EMFILE` — сбой элемента с причиной, не паника.
- Отмена посреди элемента оставляет его в `interrupted`; освобождённое внутри прерванного элемента в
  `freed_bytes` не входит — как и сейчас.
- Тест `delete_run` фиксирует прежний итог `FOLDER_FILES + 2` (дети, папка, файл) — он становится `FOLDER_FILES +
  1` (файлы папки и файл); так же меняется `app/tests/delete_job.rs`. Литерал `DeleteSubject` в
  `rules/tests/delete_run.rs` и `tray_delete.rs` получает `indexed_files`.
- Предел проверки: точки монтирования и firmlink внутри элемента в тесте не создать — правило «не заходить» видно из
  кода и Knowledge. Скорость на большом хранилище и «Остановить» посреди `v3/files` — приёмка человеком на границе
  wire (бандл пересобрать: `cargo xtask bundle --profile adhoc`).

# Gameplan

### Phase 1: deep-delete — удаление вглубь и счёт по файлам

- [ ] `apfs-sys/src/unlink_at.rs`, экспорт; `check_sources.rs`: `unlinkat(`, `unlink_at(` и разрешённые модули.
- [ ] `delete_run.rs`: дети из `BulkListing`, обход в глубину на дескрипторах, точки монтирования и firmlink не
  обходятся, отмена перед каждым файлом, шаг на не-каталог; `removeItem` и ставшее ненужным уходят.
- [ ] `DeleteProgress { files_done, files_total, subjects_done }`, `DeleteSubject.indexed_files`,
  `Tray::take_for_delete`; переход вызовов в `app/src/delete_job.rs`, `app/src/delete_run.rs` и тестах.
- [ ] `rules/tests/delete_run.rs`: итог по файлам; шаги растут внутри одного ребёнка (`subjects_done == 0` — не
  меньше числа его файлов); вложенные каталоги удаляются целиком; отмена посреди большой папки — элемент в
  `interrupted`, часть файлов на месте; ссылка на папку вне элемента — цель цела; том только для чтения —
  `ReadOnlyVolume`. `rules/tests/tray_delete.rs`: `indexed_files` из индекса. `app/tests/delete_job.rs`: итог
  `FOLDER_FILES + 1`.

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-rules --test delete_run
```
- Обход вглубь, счёт внутри папки, отмена между файлами, ссылки не уводят наружу (exit 0).
```
cargo test -p domovoi-rules --test tray_delete
```
- Число файлов элемента из индекса (exit 0).
```
cargo test -p domovoi-app --test delete_job
```
- Задача удаления на настоящем `Tray` отдаёт растущий счёт до итога (exit 0).
```
cargo xtask check-sources
```
- Удаляет только модуль удаления, `unlink_at` нигде больше (exit 0).
```
cargo clippy -p domovoi-apfs-sys -p domovoi-rules -p domovoi-app --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).

### Phase 2: tray-bar — полоса и счёт файлов в листе

<!-- deps: [deep-delete] — потребление: DeleteProgress.files_done/files_total через DeleteJob в DeleteStatus. -->

- [ ] `progress_bar.rs` (вынос из `model_card_view.rs`), карточка модели на нём.
- [ ] `DeleteStatus { files_done, files_total }`, `deleting_line`, `deleting_share`, `DELETE_UNITS`; фикстура
  `shell_state.json`.
- [ ] `tray_screen.rs`: полоса между строкой и «Остановить».
- [ ] `app/tests/tray_deleting.rs` (новый): строка и доля для 1 200 из 3 400, доля 1.0 на итоге, 0.0 при нулевом
  итоге; `app/tests/voice.rs`: «Удаляю: 1 200 из 3 400 файлов».

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-app --test tray_deleting
```
- Строка и доля полосы (exit 0).
```
cargo test -p domovoi-app --test voice
```
- Голос строки удаления (exit 0).
```
cargo test -p domovoi-app --test snapshot_care
```
- Лист 08 в состоянии удаления с полосой держит размеры в обеих темах (exit 0).
```
cargo test -p domovoi-app --test delete_job
```
- Настоящий движок фазы 1 под задачей приложения (exit 0).
```
cargo clippy -p domovoi-app --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).

### Phase 3: wire — счёт удаления на главном пути бандла

<!-- deps: [deep-delete, tray-bar] — потребление: files_done/files_total из DeleteJob (фаза 1) и TrayModel::with_deleting, deleting_line, deleting_share (фаза 2) в отчёте сценария. -->

- [ ] `end_to_end.rs::delete_recommendations`: `files_done`, `files_total`, `deleting_line`, `deleting_share`
  последнего состояния.
- [ ] `app/tests/wire_main_path.rs`: `files_done == files_total > 0`, `deleting_line` кончается итогом и словом
  «файлов» или «файла», `deleting_share == 1.0`.
- [ ] `docs/DOMOVOI.md` (решение 5, «К удалению»), книга экран 08 — подпись.

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-app --test wire_main_path
```
- Главный путь бандла удаляет с итогом по файлам и строкой листа (exit 0).
```
cargo xtask check-comments
```
- Комментарии без ссылок на книгу, фазы и спеки (exit 0).
```
cargo xtask check-sources
```
- Книга и документ без устаревших формулировок, удаление только в модуле удаления (exit 0).
```
cargo clippy -p domovoi-app --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).
