<!--
  FIX-CLONE-IN-SHARED-FOLDER.md — дельта к docs/SPEC-DOMOVOI.md: замена клоном в каталоге другого владельца
  (например /Users/Shared) проходит, но показывается в «Не заменено». 1 фаза.

  ДИАГНОЗ (/mneme:fix от 09.11.2026, гипотеза 1, правка A из трёх): после подмены и удаления старого тела execute
  возвращает каталогу прежнее время изменения, а задать время чужому каталогу система не даёт (EACCES), и ошибка
  превращает состоявшуюся замену в отказ; правка — время возвращается только своему каталогу, журнал закрывается
  до этого шага.
-->

# Baseline

- Предыдущая спека: `docs/SPEC-DOMOVOI.md`; все дельты из `docs/` применены, последняя —
  `docs/FIX-CLONES-IN-BACKGROUND.md` (`83178f8` в `main`).
- Prior spec-hash: sha256:<placeholder — confirm>
- Проблема. «Заменить клоном» для копии в каталоге другого владельца (например `/Users/Shared/…`): копия стала клоном
  и место освободилось, но Домовой показал её в «Не заменено».
- Уже есть и не меняется: отказы `ClonePlan::prepare` (копия своя, каталог доступен на запись и прочие); порядок
  клонирования, проверки, подмены и удаления старого тела; восстановление времени изменения своего каталога; фоновая
  замена `CloneJob` и итоги «Заменено клоном» / «Не заменено».
- Версия: фазы повышения версии нет — `cargo xtask release` сам инкрементирует патч.

# Stack

- `crates/dups/src/clone_plan.rs`: поле `directory_modified: Option<SystemTime>` — время изменения каталога копии,
  если каталог принадлежит текущему пользователю (`uid` каталога равен `current_user_id()`), иначе `None`.
  `execute` после удаления старого тела закрывает запись журнала (`settle`) и только потом возвращает время каталогу,
  когда оно запомнено.
- `crates/dups/tests/clone_in_shared_folder.rs` (написан диагнозом): регрессия на `/Users/Shared`.
- `docs/DOMOVOI.md`, раздел 3, п. 2: время изменения каталога возвращается, если каталог свой; в общем каталоге чужого
  владельца замена проходит, а время каталога остаётся новым.

# Conventions

- Условие «возвращать ли время каталогу» решается в `prepare` явно по владельцу, а не по коду ошибки в `execute`;
  ошибки `set_modified` в своём каталоге по-прежнему не проглатываются.
- Комментарии не ссылаются на спеку и фазы (`cargo xtask check-comments`).
- Typecheck-критерий — `cargo clippy -p domovoi-dups -p domovoi-app --all-targets -- -D warnings`; формат —
  `cargo fmt --all -- --check`.

# Knowledge

- **Красный прогон.** `cargo test -p domovoi-dups --test clone_in_shared_folder` — 0 passed, 1 failed:
    panicked at crates/dups/tests/clone_in_shared_folder.rs:75:29:
    a replacement that happened is reported as replaced: Io(Os { code: 13, kind: PermissionDenied, message: "Permission denied" })
  Проверка `clone_id` в тесте до этой строки прошла — копия стала клоном; временных `.domovoi-tmp-*` в `/Users/Shared`
  не осталось — старое тело удалено.
- **Причина.** `execute`: подмена → `delete` старого тела (`clone_plan.rs:223`) → `directory.set_modified(...)?`
  (`clone_plan.rs:230`) → `clone_journal.settle` (`clone_plan.rs:231`); каталог открыт только на чтение
  (`clone_plan.rs:206`). На `/Users/Shared` (root, `drwxrwxrwt`) задать время через дескриптор нельзя: «set explicit
  times on fd: 13 Permission denied», «set now: 13 Permission denied». `prepare` проверяет, что копия своя
  (`clone_plan.rs:109`) и каталог доступен на запись (`clone_plan.rs:112`), но не владельца каталога.
- **Журнал.** Ошибка выходила до `settle`, и запись о замене оставалась в журнале до следующей уборки.
- **Тест.** Оставляемый файл — во временной папке `TempTree::under(/Users/Shared)`, копия — прямо в `/Users/Shared`
  (`.domovoi-clone-copy-<pid>.raw`, удаляется в Drop), скан и индекс — по `/Users/Shared` (около 1 с).
- **Отвергнуто.** Правка B — восстанавливать всегда и пропускать EACCES/EPERM: прячет ошибку и решает правило по коду
  ошибки. Гипотеза 2 — отказывать в замене в чужих каталогах: замена там физически проходит и освобождает место.
  Sticky-бит каталога не мешает подмене и удалению: копия стала клоном, временных файлов не осталось.
- **Проверка мутацией.** Одна мутация на прогон: запоминать время каталога всегда — `clone_in_shared_folder`
  краснеет; не возвращать время своему каталогу — краснеет `clone_replace_image` (`clone_replace_image.rs:234-235`).

# Gameplan

<!-- 1 фаза: время каталога возвращается только своему каталогу. -->

### Phase 1: clone-in-shared-folder — замена в чужом каталоге показывается как замена

- [ ] `ClonePlan::directory_modified` по владельцу каталога в `prepare`; в `execute` — `settle` до восстановления
  времени, восстановление только для запомненного.
- [ ] Строка о времени каталога в разделе 3, п. 2 `docs/DOMOVOI.md`.
- [ ] Оставить `crates/dups/tests/clone_in_shared_folder.rs` регрессионным тестом; мутации по Knowledge, по одной на
  прогон.

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-dups --test clone_in_shared_folder
```
- Замена копии в `/Users/Shared` возвращает успех с освобождённым местом (exit 0).
```
cargo test -p domovoi-dups --test clone_replace_image
```
- Замена в своём каталоге по-прежнему возвращает каталогу прежнее время, отказы не изменились (exit 0).
```
cargo clippy -p domovoi-dups -p domovoi-app --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).
```
cargo fmt --all -- --check
```
- Формат совпадает с rustfmt (exit 0).
