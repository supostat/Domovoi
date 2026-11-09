<!--
  FIX-DUPLICATE-SEARCH-ERROR.md — дельта к docs/SPEC-DOMOVOI.md: поиск дубликатов, закончившийся ошибкой
  ввода-вывода, закрывает Домового целиком. 1 фаза.

  ДИАГНОЗ (/mneme:fix от 09.11.2026, гипотезы 1 и 2 вместе, ветка тома без APFS проверена на образе HFS+): одна копия,
  у которой не читаются метаданные, или том без PRIVATESIZE обрывает весь поиск через DuplicateGroup::of_members, а
  контроллер отдаёт любую ошибку поиска в fail, который закрывает приложение; правка — оценка для тома без
  PRIVATESIZE, нечитаемая копия уходит в пропущенные, ошибка поиска показывается на вкладке.
-->

# Baseline

- Предыдущая спека: `docs/SPEC-DOMOVOI.md`; все дельты из `docs/` применены, последняя —
  `docs/FIX-CLONED-COPIES.md` (`94add0f` в `main`).
- Prior spec-hash: sha256:<placeholder — confirm>
- Проблема. Поиск дубликатов закончился ошибкой ввода-вывода: Домовой показал окно «Домовой не смог продолжить» и
  закрылся. Ожидается сообщение об ошибке на вкладке «Дубликаты» и возможность работать дальше.
- Уже есть и не меняется: файлы, которые не читаются при хэшировании, уходят в пропущенные (`SkipReason`); отмена
  поиска; выбор оставляемого и правило «копию блокирует общее с оставляемым»; остальные вызовы `fail`.
- Версия: фазы повышения версии нет — `cargo xtask release` сам инкрементирует патч.

# Stack

- `crates/index/src/reclaimable.rs`, `Index::reclaimable`: ошибка `Unsupported` (файловая система не вернула
  PRIVATESIZE) даёт для этого файла оценку `estimate_file`, как ENOENT даёт ноль.
- `crates/dups/src/duplicate_copy.rs`: поле `reclaimable: Reclaimable` — выигрыш копии, посчитанный при сборке группы.
- `crates/dups/src/duplicate_group.rs`, `of_members`: копия, у которой `index.reclaimable` вернул ошибку, уходит из
  группы и возвращается как `SkippedFile` с `SkipReason::of(&error)`; группа без копий вне пакетов не создаётся;
  `of_members` больше не возвращает `io::Result`.
- `crates/dups/src/duplicate_search.rs`: пропущенные при сборке групп копии — в `Duplicates::skipped`.
- `crates/app/src/session.rs`: пересборка групп после изменений на диске добавляет такие копии в пропущенные.
- `crates/app/src/duplicates_model.rs`: `found` берёт выигрыш копии из `DuplicateCopy::reclaimable` и не читает
  метаданные на каждой перестройке; `found` больше не возвращает `io::Result`.
- `crates/app/src/duplicates_state.rs`, `screen_key.rs`, `duplicates_screen.rs`, `strings.rs`: состояние
  `DuplicatesState::Failed { reason }` — пустое состояние вкладки с текстом ошибки, переключатель поиска выключен и
  начинает новый поиск.
- `crates/app/src/shell_state.rs`: `ShellState::finish_duplicate_search(session, outcome, summary)` возвращает сессию,
  запоминает причину неудачи (`SearchError::Io`) и сбрасывает её при новом поиске; `rebuild_data` строит `Failed`.
- `crates/app/src/shell_controller.rs`: конец поиска зовёт `finish_duplicate_search` и `refresh`, а не `fail`.
- `crates/dups/tests/unreadable_copy.rs` (написан диагнозом): регрессия на группе и на поиске по образу HFS+.
- `docs/DOMOVOI.md`, 4.5: неудачный поиск показывает причину на вкладке и запускается заново; копия с нечитаемыми
  метаданными пропускается; на томе без APFS выигрыш копии — оценка по занятому размеру.

# Conventions

- Ошибка поиска — состояние вкладки, а не фатальная ошибка приложения; правило «что с ней делать» живёт в
  `ShellState`, контроллер только вызывает его.
- Выигрыш копии считается в одном месте — при сборке группы.
- Комментарии не ссылаются на спеку и фазы (`cargo xtask check-comments`).
- Typecheck-критерий — `cargo clippy -p domovoi-index -p domovoi-dups -p domovoi-app --all-targets -- -D warnings`;
  формат — `cargo fmt --all -- --check`.

# Knowledge

- **Красный прогон.** `cargo test -p domovoi-dups --test unreadable_copy` — 0 passed, 2 failed:
    a_copy_whose_metadata_cannot_be_read_does_not_fail_its_group (unreadable_copy.rs:73:10):
      one unreadable copy does not fail the group: Os { code: 13, kind: PermissionDenied, message: "Permission denied" }
    copies_on_a_volume_without_private_size_are_found_with_their_allocated_size (unreadable_copy.rs:151:10):
      a volume without PRIVATESIZE does not fail the search: Io(Custom { kind: Unsupported, error: "the file system did not return the private size" })
- **Закрытие приложения.** Конец поиска: `Err(error) => self.fail(&error.to_string())` (`shell_controller.rs:1006-1009`);
  `fail` показывает окно «Домовой не смог продолжить» и зовёт `terminate(None)` (`shell_controller.rs:1669-1675`),
  хотя сессия уже возвращена в состояние (`shell_controller.rs:1002`). У экрана нет состояния ошибки: только `Idle`,
  `Searching`, `Found` (`duplicates_state.rs:8-20`).
- **Обрыв поиска.** Ошибки чтения при хэшировании уходят в `SkipReason` (`content_digest.rs:56-92`), а сбор групп
  идёт через `of_members(...).map_err(SearchError::Io)?` (`duplicate_search.rs:74`) → `index.reclaimable`, который
  прощает только ENOENT (`reclaimable.rs:163-171`). Перестройка экрана снова зовёт `index.reclaimable` на каждую копию
  (`duplicates_model.rs:37-41`), и ошибка там уходит в `refresh` → `fail` (`shell_controller.rs:481-485`).
- **Том без APFS.** Проверено на образе HFS+ (`hdiutil create -fs HFS+`): `private_size` отдаёт `Unsupported` «the
  file system did not return the private size» (`identity.rs:39-41`, `path_query.rs:49-54`), и поиск по двум
  одинаковым файлам на таком томе обрывается всегда. На томе без клонов оценка `estimate_file` — занятый размер.
- **Образ в тесте.** Тест создаёт и подключает образ сам (`hdiutil create`, `attach -nobrowse -mountpoint`, `detach
  -force` в Drop); testkit умеет только APFS. Подключение образов бывает нестабильным при параллельных прогонах с
  другими образами (заметка 46dd48ef) — красный с панической строкой на attach сверять с ней.
- **Не входит.** Остальные вызовы `fail`, закрывающие приложение при восстановимых ошибках (запись CSV —
  `shell_controller.rs:1635`, замер листа — `:1143` и другие), — отдельная проверка.
- **Проверка мутацией.** Одна мутация на прогон: вернуть `?` на ошибке `index.reclaimable` в `of_members` —
  `unreadable_copy` краснеет; убрать оценку при `Unsupported` — краснеет тест на образе HFS+; вернуть `fail` в конце
  поиска — краснеет тест `ShellState` о неудачном поиске.

# Gameplan

<!-- 1 фаза: ошибка поиска не закрывает приложение, одна копия и том без APFS не обрывают поиск. -->

### Phase 1: duplicate-search-error — ошибка поиска остаётся на вкладке «Дубликаты»

- [ ] `Index::reclaimable` с оценкой при `Unsupported`.
- [ ] `DuplicateCopy::reclaimable`, пропуск копии с ошибкой в `of_members`, пропущенные в `find_duplicates` и в
  пересборке групп `Session`.
- [ ] `DuplicatesModel::found` без чтения метаданных; `DuplicatesState::Failed`, вид и ключ экрана, строки.
- [ ] `ShellState::finish_duplicate_search` и тест в `shell_state::tests`: неудачный поиск возвращает сессию и даёт
  `Failed` с причиной, новый поиск сбрасывает её; контроллер зовёт его вместо `fail`.
- [ ] Строки в 4.5 `docs/DOMOVOI.md`.
- [ ] Оставить `crates/dups/tests/unreadable_copy.rs` регрессионным тестом; мутации по Knowledge, по одной на прогон.

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-dups --test unreadable_copy
```
- Нечитаемая копия не обрывает группу, поиск на томе HFS+ находит пару с выигрышем по занятому размеру (exit 0).
```
cargo test -p domovoi-app --lib shell_state::tests
```
- Неудачный поиск возвращает сессию и показывает причину на вкладке (exit 0).
```
cargo test -p domovoi-dups --test pipeline
```
- Группы и выигрыш обычного поиска не изменились (exit 0).
```
cargo test -p domovoi-dups --test clones_among_copies
```
- Клоны среди копий по-прежнему можно удалить (exit 0).
```
cargo test -p domovoi-dups --test smoke_rules
```
- Удаление копий через лоток не изменилось (exit 0).
```
cargo clippy -p domovoi-index -p domovoi-dups -p domovoi-app --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).
```
cargo fmt --all -- --check
```
- Формат совпадает с rustfmt (exit 0).
