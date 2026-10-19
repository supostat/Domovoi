<!--
  SPEC-CLONE-TRASH-NAME.md — дельта к docs/SPEC-DOMOVOI.md: имя старого тела копии в Корзине после
  «Заменить клоном». 1 фаза.

  ГРАНИЦА РЕШЕНИЯ (веер от 18.10.2026, выбран вариант A по рекомендации): переименование внутри
  Корзины сразу после trashItem. Отвергнуты: B — сначала Корзина, затем клон на место без swap
  (теряется атомарность подмены и координация ForReplacing); C — скрытая папка с исходным именем
  внутри (пустая папка уходит только через Корзину — один скрытый мусор вместо другого).
-->

# Baseline

- Предыдущая спека: `docs/SPEC-DOMOVOI.md` (19 фаз, прогон завершён).
- Prior spec-hash: sha256:7735e83db0d5a6174daa3691483d0fc4d33378c546b86b905aabce22c05d7316
- Уже есть и не меняется: `ClonePlan::execute` (клон оригинала в `.domovoi-tmp-<UUID>` рядом с копией,
  `rename_swap` внутри `NSFileCoordinator` с `ForReplacing`, старое тело — в Корзину через
  `TrashJournal::trash` с исходным путём копии), `CloneJournal::settle`/`sweep`, `TrashJournal::restore`
  (сверка LINKID, `free_name` по правилу Finder), Корзина Домового (`Tray`) — у её элементов имя в
  Корзине совпадает с исходным.
- Проблема: `trashItemAtURL` сохраняет имя элемента, поэтому старое тело лежит в Корзине как
  `.domovoi-tmp-<UUID>` — скрытое и неузнаваемое в Finder.
- Версия: фазы повышения версии нет — `cargo xtask release` сам инкрементирует патч
  `[workspace.package].version`; CI версию не меняет.
- Обоснование выбора — staged decision note (id после accept).

# Stack

- `crates/apfs-sys/src/directory_operations.rs`: `rename_exclusive(from_directory, from_name,
  to_directory, to_name)` — `renameatx_np` с `RENAME_EXCL` (`EEXIST`, если имя занято), рядом с
  `rename_swap`.
- `crates/rules/src/trash_journal.rs`: в `TrashJournal::trash` после `trash_item` — если имя в Корзине
  отличается от имени `subject.original`, переименование в той же папке Корзины в исходное имя или в
  «имя N.расш» по правилу Finder; в журнал пишется итоговый путь.
- Тесты: `crates/apfs-sys/tests/platform.rs`, `crates/rules/tests/tray_trash.rs`,
  `crates/dups/tests/clone_replace_image.rs`.

# Conventions

- Файлы уходят только через Корзину; переименование внутри Корзины — rename, а не удаление (гейт
  `check-sources` держится).
- `rules` остаётся `#![forbid(unsafe_code)]`: системный вызов живёт в `apfs-sys`.
- Чужой файл в Корзине никогда не перезаписывается: только `RENAME_EXCL`, при `EEXIST` — следующий
  номер.
- Тесты с Корзиной идут на APFS-образе (`.Trashes/<uid>` образа), не в Корзине пользователя.

# Knowledge

- `trashItemAtURL:resultingItemURL:` переносит элемент внутри тома (rename) и возвращает итоговый URL;
  имя меняется только при совпадении. Исходное имя возвращается переименованием в той же папке
  Корзины: LINKID и FILEID сохраняются, возврат по журналу (`restore` сверяет LINKID) работает как
  прежде.
- Правило имён — то же, что у `free_name` при возврате: «имя 2.расш», «имя 3.расш»…; переименование
  exclusive, поэтому гонка с Finder или другим процессом не перезапишет файл, а возьмёт следующий
  номер.
- Переименование улучшает отображение и ничего не откатывает: если оно не удалось по иной причине,
  чем `EEXIST`, журнал хранит фактический путь со скрытым именем, замена клоном считается
  выполненной.
- Окно сбоя: падение между `trashItem` и переименованием оставляет скрытое имя — как раньше, но
  только в этом окне.
- Лишний клон неудавшейся замены (`CloneJournal::sweep`, LINKID не совпал) уходит в Корзину через
  `trash_item` без журнала и остаётся со скрытым именем: это служебный клон оригинала, не копия
  пользователя.

# Gameplan

### Phase 1: trash-name — исходное имя старого тела копии в Корзине

- [ ] `apfs-sys`: `rename_exclusive` (`renameatx_np` + `RENAME_EXCL`) и тест в `tests/platform.rs`:
  перенос в свободное имя; `EEXIST` при занятом имени, оба файла нетронуты.
- [ ] `rules`: в `TrashJournal::trash` после `trash_item` — если имя в Корзине отличается от имени
  `subject.original`, exclusive-переименование в исходное имя, при `EEXIST` — «имя N.расш»; журнал
  хранит итоговый путь; при иной ошибке переименования — фактический путь без отката переноса.
- [ ] Тест `crates/rules/tests/tray_trash.rs` на APFS-образе: элемент под другим именем ложится в
  `.Trashes/<uid>` под исходным именем; при занятом имени — «имя 2.расш»; `restore` возвращает его на
  исходный путь.
- [ ] Тест `crates/dups/tests/clone_replace_image.rs`: имя старого тела в Корзине равно имени копии и
  не начинается с `TEMPORARY_PREFIX`; возврат по журналу по-прежнему даёт `copy 2`.
- [ ] Knowledge `docs/SPEC-DOMOVOI.md`: снять открытый вопрос про `.domovoi-tmp-<UUID>` в пункте [wire].

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-apfs-sys --test platform
```
- Exclusive-переименование не перезаписывает занятое имя (exit 0).
```
cargo test -p domovoi-rules --test tray_trash
```
- Элемент под временным именем ложится в Корзину образа под исходным, при совпадении — «имя 2.расш»,
  возврат работает (exit 0).
```
cargo test -p domovoi-dups --test clone_replace_image
```
- REAL-DEP: настоящая замена клоном на образе кладёт старое тело под именем копии и возвращает его
  (exit 0).
```
cargo clippy -p domovoi-apfs-sys -p domovoi-rules -p domovoi-dups --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).
