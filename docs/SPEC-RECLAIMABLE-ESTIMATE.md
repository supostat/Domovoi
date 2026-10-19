<!--
  SPEC-RECLAIMABLE-ESTIMATE.md — дельта к docs/SPEC-DOMOVOI.md: экран 03 сразу после обхода,
  рекомендации без системных вызовов на каждый файл. 1 фаза.

  ГРАНИЦА РЕШЕНИЯ (веер от 19.10.2026, выбран вариант A по рекомендации): карточки и итоги считают
  оценку по фактам индекса, точный PRIVATESIZE перечитывается только для выбранного (Корзина
  Домового, дубликаты, журнал). Отвергнуты: B — PRIVATESIZE при обходе (обход медленнее в 1,8–2,6×
  по замеру спеки, значение нельзя кэшировать из-за снапшотов); C — фоновый пакетный пересчёт по
  папкам (точная семантика, но общий снимок индекса, жизненный цикл задачи и сопоставление по путям —
  втрое дороже).
-->

# Baseline

- Предыдущая спека: `docs/SPEC-DOMOVOI.md`; дельта `docs/SPEC-CLONE-TRASH-NAME.md` применена.
- Prior spec-hash: sha256:b55a6810ddd750e370056b00ec3531640f66c039d5a7c5d3c8d794f911a72e7a
- Проблема (приёмка 19.10.2026, 4,8 млн файлов): после обхода `Session::install` →
  `refresh_recommendations` → `Recommendations::build` вызывает `Index::reclaimable` на каждый узел
  каждой рекомендации и гипотезы; внутри — `physical_path` и `getattrlist PRIVATESIZE` на каждый
  файл. Экран 02 минутами стоит без признаков работы; тот же пересчёт идёт после каждой пачки
  FSEvents и Корзины в главном потоке.
- Уже есть и не меняется: `reclaimable_file` (правила сжатых файлов, клонов, снапшотов),
  `Index::reclaimable` с перечитыванием `PRIVATESIZE` — для `Tray::reclaimable`, `TrayModel`,
  `DuplicatesModel`, `DuplicateGroup`, `TrashJournal::reclaimable_after_emptying`; обход сканера
  без `PRIVATESIZE`.
- Версия: фазы повышения версии нет — `cargo xtask release` сам инкрементирует патч; CI версию не
  меняет.
- Обоснование выбора — staged decision note (id после accept).

# Stack

- `crates/index/src/reclaimable.rs`: `Index::reclaimable_estimate(node) -> Reclaimable` — та же
  группировка по инодам и правило жёстких ссылок, что у `reclaimable`, без системных вызовов;
  `estimate_file(facts)` для одного инода.
- `crates/rules/src/recommendations.rs`: карточки, итоги по уровням и гипотезы — через
  `reclaimable_estimate`.
- `crates/app/src/tray_model.rs`, `crates/app/src/tray_screen.rs`: лист Корзины Домового показывает
  строку удержания (снапшоты, клоны, жёсткие ссылки) из точного `reclaimable` выбранного.
- `crates/app/src/session.rs`, `crates/app/src/scan_job.rs`, `crates/app/src/scanning_screen.rs`:
  после обхода экран 02 показывает стадию «Собираю индекс…» до установки индекса.
- Тесты: `crates/index/tests/reclaimable.rs`, новый `crates/rules/tests/recommendations_scale.rs`,
  `crates/app/tests/snapshot_care.rs`, `crates/app/tests/smoke_render_scanner.rs`,
  `crates/app/tests/smoke_rules_dups.rs`.

# Conventions

- Системные вызовы на каждый файл недопустимы в путях, которые идут после обхода и после FSEvents;
  точные значения перечитываются только для того, что выбрал пользователь.
- Строки интерфейса — только в `strings`; форматирование размеров — только `domovoi-theme`.
- Тесты — спецификации: оценка и точное значение проверяются раздельно; проверки скорости —
  относительные (отношение к линейному шагу в памяти на той же машине), а не абсолютные бюджеты.

# Knowledge

- Оценка для инода: сжатый (`UF_COMPRESSED`) — как сейчас (удержан клоном, если
  `EF_SHARES_ALL_BLOCKS` или в группе потока больше одного инода, иначе освобождается `ALLOCSIZE`);
  обычный с общими блоками (`EF_SHARES_ALL_BLOCKS`, `CLONE_REFCNT > 1` или поток из нескольких
  инодов) — весь `ALLOCSIZE` в `held_by_clones`; остальные — `ALLOCSIZE` освобождается целиком;
  `held_by_snapshots` в оценке всегда 0. Жёсткие ссылки — то же правило, что у точного расчёта.
- Оценка — верхняя граница для обычных файлов: снапшоты Time Machine удерживают блоки, а без
  `PRIVATESIZE` это не видно. Точная цифра и удержание снапшотами появляются в листе Корзины
  Домового — там, где пользователь подтверждает перемещение.
- Как тест отличает путь без системных вызовов: синтетический индекс указывает на несуществующие
  пути; точный расчёт там получает ENOENT и даёт 0, оценка — сумму `ALLOCSIZE`.
- Скорость проверяется отношением: `Recommendations::build` ≤ 2× `Index::build` того же отчёта на
  500 000 файлов. Оба шага линейны и идут в памяти, поэтому отношение не зависит от машины и
  профиля сборки. Отношение ловит нелинейность, а не системные вызовы: на несуществующих путях
  `getattrlist` сразу отвечает ENOENT, и точный расчёт укладывается в то же отношение (замер фазы,
  debug: индекс 930 мс, оценка 67 мс, точный расчёт 618 мс). Путь без системных вызовов отличает
  равенство карточки сумме `ALLOCSIZE` — точный расчёт там даёт 0.
- Реальная цифра — время от конца обхода до экрана 03 на диске приёмки (4,8 млн файлов) — снимается
  человеком после фазы и записывается сюда; в гейт не входит.
- Пункт Knowledge [идентичность] основной спеки про `reclaimable(node)` «в момент показа» уточняется:
  карточки и итоги — оценка, точное перечитывание — для выбранного.

# Gameplan

### Phase 1: reclaimable-estimate — рекомендации без системных вызовов на каждый файл

- [ ] `index`: `Index::reclaimable_estimate` и `estimate_file` по правилам Knowledge; группировку по
  инодам и правило жёстких ссылок разделяют `reclaimable` и `reclaimable_estimate`.
- [ ] `rules`: `Recommendations::build` считает карточки, итоги и гипотезы через
  `reclaimable_estimate`; `Tray::reclaimable` и `TrashJournal` остаются на точном расчёте.
- [ ] `app`: `TrayModel` несёт удержание выбранного из точного `reclaimable`; лист Корзины Домового
  показывает строку «Останется занято: …», если удержание больше нуля (общая функция строки
  удержания для карточки и листа); фикстура `care.tray` получает пример удержания.
- [ ] `app`: `Session::scan` сообщает об окончании обхода в канал прогресса; `ScanJob` и экран 02
  показывают стадию «Собираю индекс…» до установки индекса.
- [ ] Тест `crates/index/tests/reclaimable.rs`: оценка для одиночного сжатого файла, сжатого с
  клоном, клонированного обычного (всё в `held_by_clones`), обычного (весь `ALLOCSIZE`), жёстких
  ссылок; `held_by_snapshots` оценки равен 0.
- [ ] Тест `crates/rules/tests/recommendations_scale.rs`: синтетический индекс с проектом
  (`Cargo.toml` + `target/` на 500 000 файлов с разными FILEID) — карточка `cargo.target` равна сумме
  `ALLOCSIZE`; время `Recommendations::build` не больше двух времён `Index::build` того же отчёта.
- [ ] Тест `crates/app/tests/smoke_render_scanner.rs`: после обхода канал прогресса получает стадию
  индексации.
- [ ] Knowledge `docs/SPEC-DOMOVOI.md`: уточнить пункт [идентичность] про `reclaimable(node)`.

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-index --test reclaimable
```
- Оценка делит клоны, сжатые файлы и жёсткие ссылки без системных вызовов (exit 0).
```
cargo test -p domovoi-rules --test recommendations_scale
```
- Рекомендации по индексу на 500 000 файлов считают сумму ALLOCSIZE не дольше двух сборок индекса
  (exit 0).
```
cargo test -p domovoi-rules --test smoke_index
```
- Настоящий индекс по-прежнему даёт карточки и итоги по уровням (exit 0).
```
cargo test -p domovoi-app --test smoke_render_scanner
```
- Стадия индексации видна после обхода (exit 0).
```
cargo test -p domovoi-app --test smoke_rules_dups
```
- REAL-DEP: настоящие rules и Корзина Домового на образе — перемещение и возврат (exit 0).
```
cargo test -p domovoi-app --test snapshot_care
```
- Снимки экранов 04/06/08 с новой строкой удержания детерминированы (exit 0).
```
cargo clippy -p domovoi-index -p domovoi-rules -p domovoi-app --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).
