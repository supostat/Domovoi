<!--
  FIX-BROKEN-CLONE-JOURNAL.md — дельта к docs/SPEC-DOMOVOI.md: неразборчивый журнал клонов в папке состояния
  закрывает диск — Домовой показывает окно ошибки и завершается. 1 фаза.

  ДИАГНОЗ (/mneme:fix от 09.11.2026, гипотеза 1 из трёх): CloneJournal::open превращает любую ошибку разбора в
  InvalidData, Session::open передаёт её дальше, и открытие кончается fail; правка — неразборчивый журнал
  откладывается рядом, как битый кэш скана сбрасывается, и сессия открывается с пустым журналом.
-->

# Baseline

- Предыдущая спека: `docs/SPEC-DOMOVOI.md`; все дельты из `docs/` применены, последняя —
  `docs/FIX-CLONE-IN-SHARED-FOLDER.md` (в рабочем дереве, ещё не в коммите).
- Prior spec-hash: sha256:<placeholder — confirm>
- Проблема. Журнал клонов (`~/Library/Application Support/com.domovoi.app/clone-journal`) не читается — Домовой
  при запуске не открывает сессию и показывает только окно ошибки.
- Уже есть и не меняется: запись журнала (`begin`, `settle`, атомарная запись); уборка временных файлов при открытии
  (`sweep`) и её ошибки; формат записи `PendingReplacement`; сброс битого кэша скана.
- Не входит: поиск временных `.domovoi-tmp-<uuid>`, оставшихся от записей неразборчивого журнала; версия формата
  журнала.
- Версия: фазы повышения версии нет — `cargo xtask release` сам инкрементирует патч.

# Stack

- `crates/dups/src/clone_journal_error.rs` (новый): `CloneJournalError { Io(io::Error), Corrupt(String) }` с `Display`
  и `From<io::Error>` — по образцу `CacheError`.
- `crates/dups/src/clone_journal.rs`: `CloneJournal::open` возвращает `Result<Self, CloneJournalError>`, ошибка разбора
  — `Corrupt(reason)`; `CloneJournal::set_aside(path) -> io::Result<Self>` переименовывает файл в
  `clone-journal.broken` (прежний такой файл заменяется) и возвращает пустой журнал по тому же пути.
- `crates/dups/src/lib.rs`: экспорт `CloneJournalError`.
- `crates/app/src/session.rs`: `Session::open` на `Corrupt` пишет причину в системный лог (категория `session`) и
  зовёт `set_aside`; `Io` по-прежнему закрывает открытие.
- `crates/app/tests/broken_clone_journal.rs` (написан диагнозом): регрессия.
- `docs/DOMOVOI.md`, раздел 3, п. 2: незаконченные замены записываются в журнал в папке состояния и убираются при
  следующем запуске; неразборчивый журнал откладывается рядом как `clone-journal.broken`, сессия открывается с пустым
  журналом, временные файлы из его записей остаются на диске.

# Conventions

- Правило «битый журнал не закрывает диск» решается в `Session::open` по варианту ошибки, как `CacheError::Corrupt`
  в `open_cached`; ошибки ввода-вывода не проглатываются.
- Один тип — один файл; комментарии не ссылаются на спеку и фазы (`cargo xtask check-comments`).
- Typecheck-критерий — `cargo clippy -p domovoi-dups -p domovoi-app --all-targets -- -D warnings`; формат —
  `cargo fmt --all -- --check`.

# Knowledge

- **Красный прогон.** `cargo test -p domovoi-app --test broken_clone_journal` — 0 passed, 1 failed:
    panicked at crates/app/tests/broken_clone_journal.rs:22:37:
    the journal "a journal cut short by a crash" does not keep the disk closed: unaligned pointer: ptr 0x0000000c76c2a376 unaligned for alignment 4
  Пустой файл журнала (0 байт) в первой версии репро падал так же:
    InvalidData, error: "subtree pointer overran range: ptr 0x0000000000000001 size 8 in range 0x0000000000000001..0x0000000000000001"
- **Причина.** `CloneJournal::open` превращает ошибку разбора в `InvalidData` (`clone_journal.rs:34-35`),
  `Session::open` передаёт её через `?` (`session.rs:72`), открытие уходит в `fail` (`shell_controller.rs:303`) —
  окно ошибки и завершение программы (`shell_controller.rs:1708-1714`).
- **Образец.** Битый кэш скана уже не закрывает диск: `CacheError::Corrupt` в `open_cached` даёт
  `CacheOpening::Reset` и полный скан (`session.rs:135`); `CacheError` — `crates/index/src/cache.rs:58-62`.
- **Почему отложить, а не перезаписать.** Уборка при открытии сразу сохраняет журнал (`session.rs:73` →
  `clone_journal.rs:97`), и байты неразборчивого журнала без переименования пропали бы при первом же открытии.
- **Источник порчи не установлен.** Запись журнала атомарна: временный файл, `sync_all`, `rename`
  (`crates/apfs-sys/src/atomic_file.rs:13-16`); заголовка версии у журнала нет, а формат записи уже менялся
  (`910a81c` убрал поля `flags` и `clone_id`). Правило покрывает любой источник.
- **Цена.** Временные `.domovoi-tmp-<uuid>` из потерянных записей остаются на диске — до одной копии на прерванную
  замену; данные пользователя не теряются: под путём копии лежит либо исходная копия, либо готовый клон.
- **Отвергнуто.** Гипотеза 2 — заголовок версии журнала: закрывает только смену формата, но не порчу и не пустой
  файл. Гипотеза 3 — не закрывать диск и при ошибках `sweep` (`session.rs:73`): шире заявленного бага и не
  воспроизведена.
- **Тест.** Диск и папка состояния — `TempTree`, два журнала (мусор и пустой файл); `Session::open` и скан проходят,
  `clone-journal.broken` хранит исходные байты, журнал снова открывается пустым.
- **Проверка мутацией.** Одна мутация на прогон: `Corrupt` в `Session::open` снова закрывает открытие —
  `broken_clone_journal` краснеет на открытии сессии; пустой журнал без переименования — краснеет на «the broken
  journal is set aside».

# Gameplan

<!-- 1 фаза: неразборчивый журнал клонов откладывается рядом, сессия открывается. -->

### Phase 1: broken-clone-journal — битый журнал клонов не закрывает диск

- [ ] `CloneJournalError` (`Io` / `Corrupt`) в своём файле; `CloneJournal::open` отдаёт `Corrupt` при неразборчивом
  содержимом; `CloneJournal::set_aside` переименовывает файл в `clone-journal.broken` и возвращает пустой журнал.
- [ ] `Session::open`: на `Corrupt` — строка в системный лог и `set_aside`; `Io` по-прежнему закрывает открытие.
- [ ] Строка о журнале в разделе 3, п. 2 `docs/DOMOVOI.md`.
- [ ] Оставить `crates/app/tests/broken_clone_journal.rs` регрессионным тестом; мутации по Knowledge, по одной на
  прогон.

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-app --test broken_clone_journal
```
- Битый и пустой журнал откладываются рядом, сессия открывается и сканирует диск (exit 0).
```
cargo test -p domovoi-dups --test clone_replace_image
```
- Замена клоном по-прежнему ведёт журнал и убирает временные файлы (exit 0).
```
cargo clippy -p domovoi-dups -p domovoi-app --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).
```
cargo fmt --all -- --check
```
- Формат совпадает с rustfmt (exit 0).
