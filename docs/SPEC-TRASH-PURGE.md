<!--
  SPEC-TRASH-PURGE.md — дельта к docs/SPEC-DOMOVOI.md: «Удалить окончательно» то, что Домовой перенёс в Корзину, —
  по журналу, своим кодом, в фоне, в 4 потока, с прогрессом и подтверждением. 3 фазы.

  ГРАНИЦА РЕШЕНИЯ (веер от 20.10.2026, выбран вариант A по рекомендации): одна кнопка «Удалить окончательно» на
  экране «Перемещено …» и у заголовка «Недавно перемещённые», удаляется всё перенесённое разом, отмена между
  единицами работы. Отвергнуты: B — «Удалить» у каждой строки плюс «всё разом» (две кнопки в строке узкого листа,
  подтверждение на строку — интерфейс ради редкого случая); C — «Удалить сразу» на экране подтверждения
  (разрушительная кнопка рядом с главной, нет паузы на итог переноса; добавляется позже на том же движке).
-->

# Baseline

- Предыдущая спека: `docs/SPEC-DOMOVOI.md`; дельты `SPEC-CRUMBS-WIDTH` применены.
- Prior spec-hash: sha256:<placeholder — confirm>
- Проблема: освободить место можно только очисткой Корзины в Finder, а на сотнях тысяч мелких файлов она идёт
  минутами. Замер на 100 000 файлов (APFS-образ, release): перенос в Корзину через `Session` 0,05 с; переиндексация
  перенесённого 3,9 с в фоне; удаление `NSFileManager.removeItem` 4,6 с в один поток и 2,6 с в 4 потока по детям
  каталога, `rm -rf` 11 с, 8 × `rm -rf` 3,9 с.
- Уже есть и не меняется: перенос (`TrashJournal::trash`, сверка `link_id`, имя по правилу Finder), «Вернуть»
  (`TrashJournal::restore`), лист Корзины Домового с «Недавно перемещённые», `ShellGate`, гейт удаления
  `cargo xtask check-sources` для всего кода, кроме нового модуля.
- Версия: фазы повышения версии нет — `cargo xtask release` (`crates/xtask/src/release.rs`) сам инкрементирует
  патч; `.github/workflows/ci.yml` версию не меняет.
- Обоснование выбора — staged decision note (id после accept).

# Stack

- `crates/rules/src/trash_purge.rs` (новый, единственный модуль, где разрешено удаление):
  `purge(entries, cancel, progress) -> PurgeReport` — записи по одной; для каждой сверка `link_id` пути в Корзине;
  каталог — дети одного уровня удаляются в `PURGE_WORKERS` = 4 потока `NSFileManager.removeItemAtURL`, затем сам
  каталог; файл — одним вызовом. Прогресс — единицы работы (дети и файлы): сделано / всего. Отчёт — удалённые,
  нетронутые (отмена), пропущенные (нет в Корзине, подменены), ошибки по записи.
- `crates/rules/src/trash_journal.rs`: `take_for_purge()` забирает все записи и сохраняет журнал;
  `return_untouched(entries)` возвращает нетронутые и сохраняет.
- `crates/xtask/src/check_sources.rs`: `PURGE_MODULE` = `rules/src/trash_purge.rs` пропускается так же, как сам
  файл проверки; поиск вызовов вынесен в чистую `removal_findings(path, content)` с тестами в `#[cfg(test)]`.
- `crates/app/src/purge_job.rs` (новый): поток, канал прогресса, отмена, итог — по образцу `TrayMeasureJob`.
- `crates/app/src/shell_action.rs`: `PurgeTrash` (входит в `needs_session` — журнал в Session), `StopPurge`.
- `crates/app/src/shell_controller.rs`: `PurgeTrash` — `NSAlert` с подтверждением, затем `take_for_purge` и
  `PurgeJob`; тик прогресса обновляет лист; итог — `return_untouched`, новый `DiskOverview::measure`, лист с итогом.
- `crates/app/src/tray_model.rs`, `tray_screen.rs`: состояние удаления (прогресс / итог); кнопки и строки по Knowledge.
- `crates/app/src/strings.rs`: `PURGE` = «Удалить окончательно», `PURGE_CONFIRM` = «Удалить окончательно
  перенесённое Домовым?», `PURGE_CONFIRM_HINT` = «Вернуть его будет нельзя.», `PURGE_ACCEPT` = «Удалить»,
  `PURGING` = «Удаляю: N из M», `PURGED` = «Удалено окончательно».
- `crates/app/src/tray_variant.rs`, `shell_fixture.rs`, `tests/fixtures/shell_state.json`: вариант снимка
  `Tray(Purging)` (`08-purging`) с прогрессом из фикстуры.
- `crates/app/src/end_to_end.rs`, `scenario_step.rs`: шаг `purge_trash` через настоящий `PurgeJob`.
- Тесты: новые `crates/rules/tests/trash_purge.rs`, `crates/app/tests/purge_job.rs`; меняются `session_gate.rs`,
  `wire_main_path.rs`.

# Conventions

- Удалять файлы может только `trash_purge.rs`, и только то, что лежит в Корзине по записи журнала и совпадает по
  `link_id`. Гейт пропускает ровно этот файл.
- Журнал забывает набор до первого удаления: «Вернуть» никогда не восстанавливает наполовину удалённое.
- Необратимое действие — только после подтверждения.

# Knowledge

- Безопасность: удаляется только `trashed` записи, у которого `link_id` совпадает с журналом. Пути нет — запись
  пропускается (Корзину уже очистили в Finder). `link_id` другой — не удаляется, запись пропускается (элемент
  подменён, вернуть его тоже нельзя, как `RestoreError::Replaced`). Записи всех томов обрабатываются одинаково;
  на деле это `~/.Trash`: лист принимает только узлы просканированного тома.
- Журнал: `take_for_purge` забирает весь набор и сохраняет журнал до первого удаления. Отмена останавливает
  раздачу работы перед следующей единицей; запись, удаление которой началось, не возвращается (остаток лежит в
  Корзине для Finder), нетронутые — `return_untouched`. Падение посреди удаления оставляет нетронутые записи без
  «Вернуть», но в Корзине — безопасная деградация.
- Потоки: многопоточность внутри записи (дети каталога), иначе одна большая папка шла бы в один поток. 4 потока —
  по замеру (2,6 с против 4,6 с на 100 000 файлов).
- Лист: на экране «Перемещено …» строка «Место освободится после очистки Корзины» остаётся, кнопки — «Удалить
  окончательно», «Открыть Корзину», «Закрыть»; у заголовка «Недавно перемещённые» — «Удалить окончательно». Во
  время удаления — строка «Удаляю: N из M» и «Остановить» вместо кнопок; итог — «Удалено окончательно: N»,
  «свободно» перемеряется.
- Подтверждение: `NSAlert`, «Удалить окончательно перенесённое Домовым?» / «Вернуть его будет нельзя.», кнопка
  «Удалить» с `hasDestructiveAction`, «Отмена» — по Esc.
- Session: задача удаления Session не берёт — работает по снимку записей; лист открыт, Session дома
  (`lends_session` не нарушается). `PurgeTrash` за гейтом `needs_session`: без Session журнала нет.
- Индекс: `~/.Trash` индексируется, удаление приходит событиями тома как лёгкая дельта на удаление — отдельного
  кода нет.
- Граница гейтов: тесты доказывают движок на APFS-образе (удаление, сверку `link_id`, пропуски, отмену с
  возвратом нетронутых), гейт `check-sources` (модуль разрешён, остальное ловится), `PurgeJob` с настоящим
  движком, состояние листа на снимке, удаление через бандл. Не доказывает гейт: `NSAlert` и маршрут кнопок
  листа — приёмка человеком: перенести большой выбор, «Удалить окончательно» → подтверждение → «Удаляю: N из M»
  → «Удалено окончательно», «свободно» выросло, Корзина в Finder без этих элементов.

# Gameplan

### Phase 1: purge-engine — удаление по журналу и исключение в гейте

<!-- deps: [] -->

- [ ] `trash_purge.rs`, `TrashJournal::take_for_purge` / `return_untouched`, исключение и `removal_findings` в
  `check_sources.rs` по Stack и Knowledge.
- [ ] Тест `crates/rules/tests/trash_purge.rs` на APFS-образе (`ImageVolume`, как в `tray_trash`): каталог из
  сотен файлов и файл переносятся `TrashJournal::trash`, `purge` удаляет оба, журнал пуст, прогресс дошёл до
  «всего»; запись, чей путь в Корзине подменён другим элементом, не удаляется и попадает в пропущенные; запись
  без пути — в пропущенные; отмена после первой записи (по событию прогресса) — вторая запись цела на диске и
  возвращается `return_untouched`.
- [ ] `check_sources.rs`, `#[cfg(test)]`: удаление в `rules/src/trash_purge.rs` разрешено, то же в другом файле —
  находка.

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-rules --test trash_purge
```
- Удаление по журналу со сверкой `link_id` и отменой (exit 0).
```
cargo test -p xtask -- check_sources
```
- Гейт пропускает только модуль удаления (exit 0).
```
cargo xtask check-sources
```
- Весь код проходит гейт (exit 0).
```
cargo clippy -p domovoi-rules -p xtask --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).

### Phase 2: purge-sheet — кнопка, подтверждение и прогресс в листе

<!-- deps: [purge-engine] — потребление: PurgeJob вызывает настоящий purge и журнал. -->

- [ ] `PurgeJob`, `ShellAction::PurgeTrash` / `StopPurge`, подтверждение, состояние удаления в `TrayModel`,
  кнопки и строки листа, строки, вариант снимка `08-purging`, перемер `DiskOverview` по Stack и Knowledge.
- [ ] Тест `crates/app/tests/purge_job.rs` на APFS-образе: настоящий `TrashJournal` и `PurgeJob` — прогресс
  растёт до «всего», итог перечисляет удалённое, пути исчезли.
- [ ] `session_gate.rs`: `PurgeTrash` в таблице действий, которым нужна Session, `StopPurge` — нет.

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-app --test purge_job
```
- REAL-DEP: PurgeJob на настоящем движке удаления (exit 0).
```
cargo test -p domovoi-app --test session_gate
```
- Удаление ждёт Session, остановка доступна всегда (exit 0).
```
cargo test -p domovoi-app --test snapshot_care
```
- Лист Корзины собирается во всех вариантах, включая 08-purging (exit 0).
```
cargo test -p domovoi-app --test voice
```
- Новые строки проходят правила голоса (exit 0).
```
cargo clippy -p domovoi-app --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).

### Phase 3: wire — удаление на главном пути бандла

<!-- deps: [purge-engine, purge-sheet] — потребление ВСЕХ code-фаз: wire-терминал по WIRE-PHASE-RULE. -->

- [ ] `ScenarioStep::PurgeTrash` («purge_trash»): `EndToEnd` забирает набор журнала и гонит настоящий
  `PurgeJob` до итога; отчёт `{ "step": "purge_trash", "purged", "skipped", "journal_left", "paths_left" }`.
- [ ] `crates/app/tests/wire_main_path.rs`: после `replace_with_clone` — `purge_trash`: `purged > 0`,
  `journal_left == 0`, `paths_left == 0`; следующие шаги (включая снимки) проходят как прежде.
- [ ] Прогрев гейта: `cargo xtask bundle --profile adhoc` выполняется задачей до гейта.
- [ ] `docs/SPEC-DOMOVOI.md`: [UI] — «Удалить окончательно» в листе Корзины Домового; Knowledge — удаление только
  по журналу со сверкой `link_id`, единственный модуль удаления в гейте.

**Done when (EXECUTABLE):**
```
cargo xtask bundle --profile adhoc
```
- Бандл для e2e собран (exit 0).
```
cargo test -p domovoi-app --test wire_main_path
```
- Перенесённое удаляется настоящим PurgeJob из бандла, журнал пуст (exit 0).
```
cargo xtask check-sources
```
- Согласованность документов и гейт удаления (exit 0).
```
cargo clippy --workspace --all-targets -- -D warnings
```
- Весь workspace чист (exit 0).
