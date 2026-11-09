<!--
  FIX-CLONES-IN-BACKGROUND.md — дельта к docs/SPEC-DOMOVOI.md: «Заменить клоном» держит главный поток —
  применение к большим копиям вешает окно на минуты без прогресса и остановки, режим клонов тормозит. 1 фаза.

  ДИАГНОЗ (/mneme:fix от 09.11.2026, гипотеза 1): apply_duplicates в обработчике кнопки по очереди зовёт
  replace_with_clone — prepare с обходом открытых файлов всех процессов (~50 мс) и побайтное сравнение копии с
  клоном (растёт с размером, CancelToken::default()); care_models при каждой перестройке зовёт clone_plan на каждую
  копию. Правка — CloneJob в потоке с прогрессом по байтам и остановкой, оценка копий без обхода процессов.
-->

# Baseline

- Предыдущая спека: `docs/SPEC-DOMOVOI.md`; все дельты из `docs/` применены, последняя —
  `docs/FIX-DUPLICATE-PREVIEW.md` (`929acb9` в `main`).
- Prior spec-hash: sha256:<placeholder — confirm>
- Проблема. В «Дубликатах» выбран режим «Заменить клоном» и применён к нескольким копиям видео по ~10 ГБ: окно
  на минуты зависает с крутящимся курсором, без прогресса и кнопки остановки. Переключение в режим клонов и
  обновления экрана в нём подтормаживают. Ожидается замена в фоне с прогрессом и возможностью остановить.
- Уже есть и не меняется: порядок замены в `ClonePlan::execute` (клон во временное имя, метаданные, проверка
  перед обменом, `RENAME_SWAP` под `NSFileCoordinator`, удаление старого тела), предусловия `prepare` и их
  `CloneRefusal`, `CloneJournal` и его `sweep` при `Session::open`, удаление копий через лоток.
- Версия: фазы повышения версии нет — `cargo xtask release` сам инкрементирует патч.

# Stack

- `crates/rules/src/content_comparison.rs`: `ContentComparison::of(first, second, size, cancel, compared)` —
  `compared: &dyn Fn(u64)` получает число сравненных байт после каждого блока; `delete_run.rs` и тест правил
  передают пустой отклик.
- `crates/dups/src/clone_plan.rs`: `ClonePlan::assess(index, keep, copy, denylist, cloud_folders)` — все
  предусловия `prepare`, кроме `other_processes_holding`; `prepare` проходит через `assess` и добавляет обход
  процессов и план. `execute(self, clone_journal, cancel, compared)` передаёт токен и отклик в сравнение;
  `ContentComparison::Stopped` даёт `CloneReplaceError::Stopped`, журнал убирает временный клон, копия не тронута.
- `crates/dups/src/clone_replace_error.rs`: вариант `Stopped`; `crates/app/src/clone_refusal_text.rs` — его текст.
- `crates/app/src/session.rs`: `Session::clone_assessment(keep, copy)` вместо `clone_plan` для оценки;
  `Session::replace_with_clones(pairs, progress, cancel) -> ClonesOutcome` вместо `replace_with_clone`: копии по
  очереди, токен проверяется между копиями, прогресс — `CloneProgress { done, total }` в байтах сравнения всех пар,
  пересканирование заменённых путей одним вызовом в конце.
- `crates/app/src/clone_progress.rs`, `clones_outcome.rs` (`freed`, `failures: Vec<(PathBuf, CloneReplaceError)>`,
  `stopped`), `clone_job.rs`: `pub struct CloneJob` по образцу `ApplyJob` и `DuplicateJob` — `start(session,
  pairs)`, `absorb_progress`, `done`/`total`, `stop`, `is_finished`, `finish -> (Session, ClonesOutcome)`.
- `crates/app/src/duplicates_state.rs`, `duplicates_screen.rs`: `DuplicatesState::Cloning { done, total }` —
  блок прогресса с заголовком `CLONING`, счётом размеров через `format_size` и кнопкой «Остановить», которая шлёт
  `ShellAction::StopClones`; кнопка поиска остаётся на `toggleDuplicateSearch:`.
- `crates/app/src/shell_state.rs`: поле `cloning: Option<CloneJob>`; `care_models` оценивает копии через
  `clone_assessment`.
- `crates/app/src/shell_controller.rs`, `shell_action.rs`: `apply_duplicates` в режиме клонов забирает сессию,
  запускает `CloneJob`, ставит `Cloning` и тикает таймером, как поиск; по окончании возвращает сессию,
  `refresh` и сообщение: `CLONED`, `NOT_CLONED` с отказами, `CLONES_STOPPED` при остановке; `StopClones`
  и `stop_background_work` останавливают задачу.
- `crates/app/src/strings.rs`: `CLONING`, `CLONES_STOPPED`.
- `crates/app/src/end_to_end.rs`: шаг `replace_with_clone` и оценка идут через `replace_with_clones` с одной
  парой и `clone_assessment`.
- `crates/app/tests/clone_replacement_caller.rs` (написан диагнозом), `smoke_rules_dups.rs`,
  `crates/dups/tests/clone_replace_image.rs`, `crates/rules/tests/duplicate_copy_delete.rs` — под новые подписи.
- `docs/DOMOVOI.md`, 4.2 п. 2: замена идёт в фоне с полосой и «Остановить», остановка не трогает текущую копию;
  «не открыта» проверяется при замене, отказ — в итоговом сообщении.

# Conventions

- Тяжёлая работа с сессией — задача в потоке, сессия уходит в неё и возвращается в `finish` (`ApplyJob`,
  `DuplicateJob`); главный поток только тикает и рисует прогресс.
- Один тип — один файл; комментарии не ссылаются на спеку и фазы (`cargo xtask check-comments`).
- Typecheck-критерий — `cargo clippy -p domovoi-rules -p domovoi-dups -p domovoi-app --all-targets -- -D warnings`;
  формат — `cargo fmt --all -- --check`; перед сдачей шага прогнать clippy-критерий самому (заметка 881723d0).

# Knowledge

- **Красный прогон.** `cargo test -p domovoi-app --test clone_replacement_caller` — 0 passed, 2 failed:
    switching_to_clone_mode_does_not_hold_the_caller (clone_replacement_caller.rs:80:5):
      assessing 8 copies for clones held the caller for 463.056208ms
    replacing_a_large_copy_with_a_clone_does_not_hold_the_caller (clone_replacement_caller.rs:62:5):
      replacing a 1073741824-byte copy held the caller for 418.448ms
- **Замер.** 64 / 256 / 1024 МиБ: `pids_with_open` ≈ 50 мс при любом размере (≈680 процессов),
  `clone_plan` ≈ 50 мс, `replace_with_clone` 95 / 150 / 470 мс; `ContentComparison::of` на 1 ГиБ — 180–370 мс
  при тёплом кэше. Для копии в 10 ГБ сравнение читает 2×10 ГБ — секунды и десятки секунд на копию.
- **Причина (гипотеза 1, подтверждена).** `apply_duplicates` (`shell_controller.rs:1041-1053`) в обработчике
  кнопки зовёт `replace_with_clone` на каждую отмеченную копию; `execute` сравнивает копию с клоном с
  `CancelToken::default()` (`clone_plan.rs:206`) — остановить нечем, прогресса нет. `care_models`
  (`shell_state.rs:331-332`) на каждой перестройке в режиме клонов зовёт `clone_plan` на каждую копию, а
  `prepare` обходит открытые файлы всех процессов (`clone_plan.rs:127` → `pids_with_open`).
- **Отвергнуто.** Гипотеза 2 — ожидание `NSFileCoordinator` (`clone_plan.rs:215-234`): в замере presenter'ов не
  было, время объясняет сравнение; прогресса и остановки она не даёт. Вариант «один обход процессов на все копии
  и кэш отказов» оставляет применение на главном потоке. Кэш отказов между перестройками не нужен: оценка без
  обхода процессов ≈ 0–4 мс на копию (`clone_plan` минус `pids_with_open` в замере).
- **Поведение.** Отказ «открыта в другой программе» больше не виден в режиме клонов заранее: `prepare` в задаче
  находит его при замене, строка уходит в `NOT_CLONED`. Выход во время задачи: временный клон убирает
  `CloneJournal::sweep` при следующем `Session::open` (`session.rs:71`).
- **Регрессия.** `clone_replacement_caller.rs`: первый тест переписывается на `CloneJob` — `start` возвращает
  меньше чем за `CALLER_BUDGET`, прогресс растёт, `stop` после первого прогресса даёт `stopped`, копия на том же
  иноде, временных `.domovoi-tmp-` в папке нет; второй тест зовёт `clone_assessment`, утверждение прежнее.
- **Проверка мутацией.** По одной на прогон: вернуть `CancelToken::default()` в сравнение `execute` —
  краснеет тест остановки; вернуть `other_processes_holding` в `assess` — краснеет тест режима клонов; выполнить
  замену в `CloneJob::start` до возврата — краснеет тест бюджета.

# Gameplan

<!-- 1 фаза: замена клоном в фоне с прогрессом и остановкой, оценка копий без обхода процессов. -->

### Phase 1: clones-in-background — «Заменить клоном» идёт в фоне с прогрессом и остановкой

- [ ] `ContentComparison::of` с откликом сравненных байт; `ClonePlan::assess`, `execute` с токеном и откликом,
  `CloneReplaceError::Stopped` и его текст.
- [ ] `Session::clone_assessment` и `replace_with_clones`; `CloneProgress`, `ClonesOutcome`, `CloneJob`.
- [ ] `DuplicatesState::Cloning`, блок прогресса и `StopClones`; контроллер запускает задачу, тикает, итог в
  сообщении; `stop_background_work` её останавливает; строки `CLONING`, `CLONES_STOPPED`.
- [ ] `end_to_end.rs`, `smoke_rules_dups.rs`, `clone_replace_image.rs`, `duplicate_copy_delete.rs` под новые
  подписи; абзац в 4.2 `docs/DOMOVOI.md`.
- [ ] Переписать `crates/app/tests/clone_replacement_caller.rs` по Knowledge и оставить регрессией; мутации по
  Knowledge, по одной на прогон.

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-app --test clone_replacement_caller
```
- Задача возвращает управление сразу, отдаёт прогресс и останавливается, не трогая копию; оценка 8 копий укладывается в бюджет (exit 0).
```
cargo test -p domovoi-dups --test clone_replace_image
```
- Предусловия и замена на образе APFS не изменились (exit 0).
```
cargo test -p domovoi-app --test smoke_rules_dups
```
- Оценка и замена через сессию на образе (exit 0).
```
cargo test -p domovoi-rules --test duplicate_copy_delete
```
- Сравнение копий при удалении не изменилось (exit 0).
```timeout=900s
cargo xtask bundle --profile adhoc
```
- Бандл собирается (exit 0).
```timeout=900s
cargo test -p domovoi-app --test wire_main_path
```
- Главный путь бандла с шагом replace_with_clone (exit 0).
```timeout=900s
cargo clippy -p domovoi-rules -p domovoi-dups -p domovoi-app --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).
```
cargo fmt --all -- --check
```
- Формат совпадает с rustfmt (exit 0).
