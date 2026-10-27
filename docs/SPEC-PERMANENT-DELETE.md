<!--
  SPEC-PERMANENT-DELETE.md — дельта к docs/SPEC-DOMOVOI.md: Корзина Домового остаётся списком сбора, но действие над
  ней — окончательное удаление собственным движком, как в DaisyDisk; шаг Корзины Finder, журнал, «Вернуть»,
  «Недавно перемещённые» и «Удалить окончательно» уходят. 4 фазы.

  ГРАНИЦА РЕШЕНИЯ (веер от 27.10.2026, выбран вариант A по рекомендации): движок удаления остаётся на
  NSFileManager.removeItemAtURL с детьми каталога в 4 потока (замер 2,6 с на 100 000 файлов), меняется только
  источник записей — элементы Корзины Домового вместо журнала; подтверждение — NSAlert с hasDestructiveAction.
  Отвергнуты: B — обратный отсчёт 5 с вместо подтверждения (противоречит разделу 5 документа и принципу «ничего без
  спроса», ошибочный клик на карточке «Проверьте сами» становится потерей данных); C — собственный обход на
  дескрипторах через unlinkat (нет гонок по путям, но заново решать семантику бандлов, ACL и флагов, перемерять
  скорость, новый код в двух крейтах; возможен позже за той же сигнатурой delete_run).
-->

# Baseline

- Предыдущая спека: `docs/SPEC-DOMOVOI.md`; все дельты из `docs/` применены. Эта дельта ОТМЕНЯЕТ
  `docs/SPEC-TRASH-PURGE.md` и `docs/SPEC-CLONE-TRASH-NAME.md`: их Knowledge больше не описывает продукт.
- Prior spec-hash: sha256:<placeholder — confirm>
- Проблема: удаление идёт в два шага (Корзина Finder, затем «Удалить окончательно» по журналу), место освобождается
  только после второго; журнал, «Вернуть», «Недавно перемещённые» и экран «Перемещено …» существуют ради первого шага.
  Реверс DaisyDisk 4.33.3: Collector → «Delete» → окончательное удаление с прогрессом, перед удалением отдельный
  проход перечитывает размер и хард-линки, отчёт содержит выжившие элементы.
- Уже есть и не меняется: список сбора `Tray` (`rules/src/tray.rs`: `add`, `remove`, `items`), замер
  `TrayMeasureJob` и `TrayModel::measured`, `Denylist` с причинами, `ShellGate` и `lends_session`, движок
  `purge()` с 4 потоками по детям и отменой между единицами работы (переименовывается, логика сохраняется),
  `CancelToken`, `DiskOverview::measure`, индексация `~/.Trash` и дельты FSEvents.
- Версия: фазы повышения версии нет — `cargo xtask release` (`crates/xtask/src/release.rs`) сам инкрементирует
  патч; `.github/workflows/ci.yml` версию не меняет.
- Typecheck-критерий проекта: `cargo clippy -p <crate> --all-targets -- -D warnings` (как в прежних спеках).
- Обоснование выбора — staged decision note (id после accept).

# Stack

- `docs/DOMOVOI.md`: принцип 3 — «Удаление только после подтверждения и только того, что собрано в Корзине Домового;
  удаление необратимо»; решение 5 — «Собственный движок удаления, `NSFileManager removeItem`, дети каталога в 4
  потока; Корзина Finder не используется»; раздел 4.4 «Корзина Домового» — список сбора с итогом, подтверждение,
  прогресс, отчёт; раздел 4.5 — копии дубликатов удаляются окончательно; раздел 4.8 экран 8 — «подтверждение перед
  удалением». `design/domovoi-design-book.html`, экран 8: текст состояний по Knowledge (без изменения токенов).
- `crates/xtask/src/check_sources.rs`: `STALE_STATEMENTS` пополняется парами «только в Корзину Finder» и «trashItem»
  для `docs/DOMOVOI.md`; `PURGE_MODULE` → `DELETE_MODULE` = `rules/src/delete_run.rs`; причина находки — «files are
  deleted only by the delete run».
- `crates/rules/src/delete_run.rs` (переименованный `trash_purge.rs`, единственный модуль удаления):
  `delete(subjects: Vec<DeleteSubject>, cancel, progress) -> DeleteReport`. План по каждому элементу:
  `symlink_metadata` физического пути — нет: `DeleteSkip::Missing`; `link_id` не совпал с индексом:
  `DeleteSkip::Replaced`; каталог — дети одного уровня. Выполнение как сейчас: дети в `DELETE_WORKERS` = 4 потока,
  затем корень; отмена между единицами — `interrupted` (корень не удаляется) и `untouched`; ошибка — `failed` с
  текстом `localizedDescription`. Отчёт: `deleted: Vec<DeleteSubject>`, `interrupted`, `untouched`, `skipped(reason)`,
  `failed(error)`, `freed_bytes` = сумма `measured_bytes` удалённых.
- `crates/rules/src/delete_subject.rs` (вместо `trash_subject.rs`): `path`, `physical`, `link_id`, `measured_bytes`,
  `rule_id`. `crates/rules/src/delete_skip.rs`, `delete_report.rs`, `delete_progress.rs` — переименования
  `purge_*`. `crates/rules/src/delete_failure.rs` (вместо `trash_failure.rs`): `Denied(DenyReason)`, `Missing`,
  `Replaced`, `ReadOnly`, `NoPermission`, `Other(String)` — по коду `NSError` `removeItem`.
- `crates/rules/src/tray.rs`: `move_to_trash` → `take_for_delete(index, denylist, sizes: &[Reclaimable])
  -> (Vec<DeleteSubject>, Vec<DeleteOutcome>)`: вердикт Denylist по каждому узлу в момент взятия, отказы —
  `Denied`; `open_trash` удаляется.
- Удаляются: `rules/src/trash_journal.rs`, `trash_item.rs`, `trash_subject.rs`, `trash_failure.rs`,
  `trash_purge.rs`, экспорт `TrashJournal`, `JournalEntry`, `RestoreError`, `TrashOutcome` из `rules/src/lib.rs`.
- `crates/dups/src/clone_plan.rs`: старое тело при замене клоном — через `delete_run::delete` одним элементом, не
  через Корзину; `clone_journal.rs` хранит только журнал замен. Прочие вызовы `trash_item` вне Корзины Домового
  (модели: `RemoveModel`, `TrashOrphan`) переводятся на тот же движок — список даёт компилятор после удаления
  `trash_item.rs`.
- `crates/app/src/delete_job.rs` (переименованный `purge_job.rs`): поток, прогресс, отмена, итог.
- `crates/app/src/shell_action.rs`: `ConfirmTray` → `DeleteTray`, `StopPurge` → `StopDelete`, `TrashOrphan` →
  `DeleteOrphan`; удаляются `OpenTrash`, `Restore`, `PurgeTrash`. `needs_session`: `DeleteTray` да, `StopDelete`
  нет.
- `crates/app/src/tray_model.rs`: убираются `journal`, `purging`, `purged`, `outcome` и `RECENT_LIMIT`; добавляются
  `deleting: Option<DeleteStatus>`, `deleted: Option<DeleteOutcome>`. Удаляются `journal_row.rs`,
  `purge_outcome.rs`, `purge_status.rs`, `tray_outcome.rs`; появляются `delete_status.rs`, `delete_outcome.rs`.
- `crates/app/src/tray_variant.rs`: `Measured`, `Measuring`, `Deleting`; `shell_fixture.rs`,
  `tests/fixtures/shell_state.json`: вариант `08-deleting`.
- `crates/app/src/tray_screen.rs`, `tray_sheet.rs`, `shell_controller.rs`: кнопки «Удалить» и «Отмена»; `NSAlert`
  с `hasDestructiveAction`; во время удаления — «Удаляю: N из M» и «Остановить»; итог — «Удалено: N объектов,
  освобождено X» и список «Не удалено:» с причиной, кнопка «Закрыть»; после итога `DiskOverview::measure`.
  Блок «Недавно перемещённые» на экране рекомендаций удаляется.
- `crates/app/src/strings.rs`: удаляются `WILL_MOVE`, `TRAY_NOTE`, `MOVE_TO_TRASH`, `MOVED_TO_TRASH`,
  `FREED_AFTER_EMPTYING`, `OPEN_TRASH`, `RECENTLY_MOVED`, `RESTORE`, `NOT_MOVED`, `PURGE*`, `RESTORE_*`,
  `TRASH_NO_TRASH`; добавляются `DELETE` = «Удалить», `DELETE_CONFIRM` = «Удалить N объектов?»,
  `DELETE_CONFIRM_HINT` = «Вернуть их будет нельзя.», `DELETING` = «Удаляю:», `DELETED` = «Удалено:», `FREED` =
  «освобождено», `NOT_DELETED` = «Не удалено:»; `ACTION_TRASH` → `ACTION_DELETE` = «Удалить»; `DUPLICATES_NOTE` —
  «По умолчанию копии удаляются…»; `TRAY_NOTE` заменяется на «Удаление необратимо: файлы не попадают в Корзину.»
- `crates/app/src/end_to_end.rs`, `scenario_step.rs`: шаги `TrashRecommendations` → `DeleteRecommendations`,
  удаляются `RestoreLast` и `PurgeTrash`; `ReplaceWithClone` идёт через новый движок.
- Тесты: `crates/rules/tests/delete_run.rs` (из `trash_purge.rs`), `crates/rules/tests/tray_delete.rs` (из
  `tray_trash.rs`), `crates/app/tests/delete_job.rs` (из `purge_job.rs`); меняются `denylist.rs`, `session_gate.rs`,
  `snapshot_care.rs`, `voice.rs`, `smoke_tray_measure.rs`, `wire_main_path.rs`.

# Conventions

- Удалять файлы может только `rules/src/delete_run.rs`; гейт `check-sources` пропускает ровно этот файл, тесты и
  `agent/src/private_socket.rs`.
- Перед удалением каждого элемента: вердикт Denylist в момент взятия из Корзины Домового, совпадение `link_id`
  физического пути с индексом в момент плана. Любое расхождение — пропуск с причиной, не удаление.
- Необратимое действие — только после подтверждения `NSAlert` с `hasDestructiveAction`. Обратного отсчёта нет.
- Голос по разделу 5: «Удалено: N объектов, освобождено X», без восклицаний.
- Один коммит на фазу; `cargo xtask check-comments` и `check-tokens` зелёные в каждой фазе.

# Knowledge

- Безопасность: удаляется только физический путь элемента, чей `link_id` совпадает с узлом индекса в момент плана.
  Окно между планом и `removeItem` — миллисекунды; гонка по пути закрывается вариантом C позже при необходимости.
- Корзина Finder не трогается: `Denylist` по-прежнему закрывает `~/.Trash` (константа `TRASH`), узлы Корзины Finder
  в Корзину Домового не попадают.
- Отмена останавливает раздачу работы перед следующей единицей: элемент, удаление которого началось, попадает в
  `interrupted` (корень на диске, часть детей удалена); нетронутые — `untouched`. Лист показывает оба списка.
- Read-only том: `trashItem` отвечал кодом 3328 (заметка памяти); `removeItem` на read-only томе даёт `EROFS` в
  `NSCocoaErrorDomain` 513 — `DeleteFailure::ReadOnly`, строка `TRASH_READ_ONLY` остаётся. Denylist проверяет W_OK
  родителя только на записываемых томах — без изменений.
- Размер: `measured_bytes` берётся из замера листа (`Reclaimable.bytes`), `freed_bytes` — сумма по удалённым.
  Честное «свободно» после удаления даёт `DiskOverview::measure`, не сумма.
- Индекс: удаление приходит событиями тома как дельта на удаление — отдельного кода нет (как у SPEC-TRASH-PURGE).
- Замена клоном: старое тело раньше уходило в Корзину с именем по правилу Finder (SPEC-CLONE-TRASH-NAME); теперь
  удаляется одним вызовом `delete` после `rename_swap`, правило имени не нужно.
- Session: `DeleteJob` Session не берёт — работает по снимку субъектов; лист открыт, Session дома.
- Граница гейтов: тесты доказывают движок на APFS-образе (удаление, сверку `link_id`, пропуски, отмену), гейт
  `check-sources`, `DeleteJob` на настоящем движке, лист во всех вариантах, сквозной путь через бандл. Не доказывает
  гейт: `NSAlert` и маршрут кнопок — приёмка человеком: собрать рекомендации, «Удалить» → подтверждение → «Удаляю:
  N из M» → «Удалено: N объектов, освобождено X», «свободно» выросло, Корзина Finder без этих элементов.

# Gameplan

### Phase 1: delete-docs — принципы, решения и гейт документов

<!-- deps: [] -->

- [ ] `docs/DOMOVOI.md`: принцип 3, решение 5, разделы 4.4, 4.5, 4.8 по Stack; упоминания Корзины Finder убраны.
- [ ] `design/domovoi-design-book.html`, экран 8: состояния «список», «подтверждение», «удаление», «итог» по
  Knowledge; токены не трогаются.
- [ ] `crates/xtask/src/check_sources.rs`: `STALE_STATEMENTS` пополняется парами «только в Корзину Finder» и
  «trashItem» для `docs/DOMOVOI.md`; тест в `#[cfg(test)]`.

**Done when (EXECUTABLE):**
```
grep -q необратимо docs/DOMOVOI.md
```
- Принцип 3 переписан (exit 0).
```
cargo xtask check-sources
```
- Документ и книга без устаревших утверждений о Корзине Finder (exit 0).
```
cargo test -p xtask -- check_sources
```
- Новые устаревшие утверждения ловятся (exit 0).
```
cargo clippy -p xtask --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).

### Phase 2: delete-engine — движок по элементам Корзины Домового и гейт кода

<!-- deps: [delete-docs] — order-only: гейт документов должен быть зелёным до смены причины в гейте кода. -->

- [ ] `rules/src/delete_run.rs`, `delete_subject.rs`, `delete_failure.rs`, `delete_skip.rs`, `delete_report.rs`,
  `delete_progress.rs`, `Tray::take_for_delete` по Stack; удаление `trash_journal.rs`, `trash_item.rs`,
  `trash_subject.rs`, `trash_failure.rs`, `trash_purge.rs` и их экспортов.
- [ ] `dups/src/clone_plan.rs`: старое тело — через `delete`; вызовы `trash_item` в других крейтах — по компилятору.
- [ ] `xtask/src/check_sources.rs`: `DELETE_MODULE`, причина находки, тест в `#[cfg(test)]`.
- [ ] `crates/rules/tests/delete_run.rs` на APFS-образе (`ImageVolume`): каталог из сотен файлов и файл удаляются,
  прогресс дошёл до «всего», `freed_bytes` равен сумме замера; элемент с подменённым `link_id` — в пропущенные;
  исчезнувший — в пропущенные; отмена после первого элемента — второй цел и в `untouched`.
- [ ] `crates/rules/tests/tray_delete.rs`: `take_for_delete` отдаёт отказы Denylist отдельно от субъектов; список
  после взятия пуст.

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-rules --test delete_run
```
- Удаление, сверка `link_id`, пропуски и отмена на образе (exit 0).
```
cargo test -p domovoi-rules --test tray_delete
```
- Взятие из списка сбора с вердиктом Denylist (exit 0).
```
cargo test -p domovoi-rules --test denylist
```
- Denylist не изменил поведения (exit 0).
```
cargo test -p xtask -- check_sources
```
- Гейт пропускает только `delete_run.rs` (exit 0).
```
cargo xtask check-sources
```
- Весь код проходит гейт (exit 0).
```
cargo clippy -p domovoi-rules -p domovoi-dups -p xtask --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).

### Phase 3: delete-sheet — лист, подтверждение, прогресс и итог

<!-- deps: [delete-engine] — потребление: DeleteJob и лист вызывают настоящий delete и take_for_delete. -->

- [ ] `DeleteJob`, `ShellAction` (`DeleteTray`, `StopDelete`, `DeleteOrphan`, удаление `OpenTrash`, `Restore`,
  `PurgeTrash`), `TrayModel`, `TrayVariant`, `tray_screen.rs`, `shell_controller.rs`, строки, вариант снимка
  `08-deleting`, удаление «Недавно перемещённые», перемер `DiskOverview` по Stack и Knowledge.
- [ ] `crates/app/tests/delete_job.rs` на APFS-образе: настоящий `Tray::take_for_delete` и `DeleteJob` — прогресс
  растёт до «всего», итог перечисляет удалённое, пути исчезли.
- [ ] `session_gate.rs`: `DeleteTray` ждёт Session, `StopDelete` — нет; `snapshot_care.rs`: вариант `08-deleting`;
  `voice.rs`: новые строки; `smoke_tray_measure.rs`: замер без журнала.

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-app --test delete_job
```
- REAL-DEP: `DeleteJob` на настоящем движке и списке сбора (exit 0).
```
cargo test -p domovoi-app --test session_gate
```
- Удаление ждёт Session, остановка доступна всегда (exit 0).
```
cargo test -p domovoi-app --test snapshot_care
```
- Лист собирается во всех вариантах, включая `08-deleting` (exit 0).
```
cargo test -p domovoi-app --test voice
```
- Новые строки проходят правила голоса (exit 0).
```
cargo test -p domovoi-app --test smoke_tray_measure
```
- Замер листа без журнала (exit 0).
```
cargo clippy -p domovoi-app --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).

### Phase 4: wire — сквозной путь через бандл

<!-- deps: [delete-engine, delete-sheet] — потребление: сценарий собирает настоящие Tray, delete_run, DeleteJob и лист. -->

- [ ] `end_to_end.rs`, `scenario_step.rs`: шаг `DeleteRecommendations { level }` через настоящий `DeleteJob`,
  `ReplaceWithClone` через новый движок; удаление шагов `RestoreLast`, `PurgeTrash`.
- [ ] `wire_main_path.rs`: сценарий скан → рекомендации → удаление уровня «Безопасно» → путей нет на диске →
  «свободно» выросло → замена клоном удаляет старое тело.
- [ ] Приёмка человеком по Knowledge (граница гейтов).

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-app --test wire_main_path
```
- Главный путь на настоящих модулях через бандл (exit 0).
```
cargo xtask check-comments
```
- Комментарии не ссылаются на отменённые спеки (exit 0).
```
cargo clippy --workspace --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).
