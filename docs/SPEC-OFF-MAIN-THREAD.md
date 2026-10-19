<!--
  SPEC-OFF-MAIN-THREAD.md — дельта к docs/SPEC-DOMOVOI.md: точный размер Корзины Домового и тяжёлые
  дельты уходят с главного потока, действия, которым нужна Session, ждут её. 4 фазы.

  ГРАНИЦА РЕШЕНИЯ (веер от 19.10.2026, выбран вариант A по рекомендации): фоновый замер выбранного
  пакетным PRIVATESIZE + тяжёлые листинги применяются в ApplyJob с Session в потоке, общий гейт
  «Session занята». Отвергнуты: B — тяжёлое поддерево → полный ScanJob (экран 02 на минуты после
  каждого большого переноса в Корзину и MustScanSubDirs на корне); C — фоновая пересборка без
  Session (полный пересчёт тома ради известного, пик памяти — два индекса, дубликаты точным
  PRIVATESIZE при подмене на главном потоке).
-->

# Baseline

- Предыдущая спека: `docs/SPEC-DOMOVOI.md`; дельты `docs/SPEC-CLONE-TRASH-NAME.md`,
  `docs/SPEC-RECLAIMABLE-ESTIMATE.md` и `docs/SPEC-VOLUME-DELTA.md` применены.
- Prior spec-hash: sha256:<placeholder — confirm>
- Проблема (приёмка 19.10.2026, выбрано 31 ГБ: node_modules, Gradle, кэши — сотни тысяч файлов):
  «В Корзину Домового» → `open_tray` → `TrayModel::build` → `Index::reclaimable` (getattrlist
  PRIVATESIZE на каждый файл) на главном потоке без прогресса; `refresh_tray` повторяет проход после
  каждой применённой дельты, пока лист открыт; «Переместить в Корзину» → `confirm_tray` →
  `tray_reclaimable()` — тот же проход ещё раз. После переноса событие `MustScanSubDirs` на корне
  Data-тома и перенесённые деревья в `~/.Trash` читаются в фоне `DeltaJob`, но применение листинга
  размером с поддерево — работа O(узлов листинга) на главном потоке. Во время поиска дубликатов
  `Session` в задаче, а «В Корзину Домового» делает `expect` открытой сессии — паника.
- Уже есть и не меняется: дельты FSEvents (`EventQueue`, `DeltaJob`, `Session::apply_listings`,
  `Recommendations::apply_delta`); полный скан `ScanJob`; `reclaimable_estimate` на карточках;
  точный расчёт дубликатов (`DuplicateGroup::of_members`); журнал Корзины.
- Версия: фазы повышения версии нет — `cargo xtask release` сам инкрементирует патч;
  `.github/workflows/ci.yml` версию не меняет.
- Обоснование выбора — staged decision note (id после accept).

# Stack

- `crates/index`: `measure_reclaimable(&Node) -> Reclaimable` — точный размер пункта по фактам
  обхода с `private_size` по правилам `Index::reclaimable`.
- `crates/app`: `TrayMeasureJob` (поток, прогресс, отмена); `TrayModel` по оценке с состоянием
  замера; лист экрана 08 со строкой прогресса; `EventQueue` с порогом тяжёлой пачки и
  `HEAVY_DELTA_NODES` из замера; `ApplyJob` (Session в потоке); `ShellAction::needs_session` и гейт в
  `allows`.
- Удаляются: `Session::tray_reclaimable`, `Tray::reclaimable`.
- Тесты: новые `crates/index/tests/measured_reclaimable.rs`, `crates/app/tests/smoke_tray_measure.rs`,
  `crates/app/tests/heavy_threshold.rs`, `crates/app/tests/session_gate.rs`; меняются
  `crates/app/tests/smoke_rules_dups.rs`, `crates/app/tests/snapshot_care.rs` (+ фикстура `care`),
  `crates/app/tests/smoke_volume_delta.rs`, `crates/app/tests/wire_main_path.rs`.

# Conventions

- Главный поток не делает работы по размеру выбранного или по размеру тяжёлого листинга: точный
  замер — в `TrayMeasureJob`, тяжёлые листинги — в `ApplyJob`. Исключение: оценка выбранного при
  открытии листа (в памяти индекса, без системных вызовов).
- Точная цифра пункта имеет семантику `Index::reclaimable` этого пункта; тесты сравнивают с ним.
- Действие оболочки, которому нужна `Session`, проверяется гейтом до выполнения; `expect` открытой
  сессии в обработчиках недостижим.
- Проверки скорости — относительные. Порог тяжёлой пачки не задаётся на глаз: он выводится замером и
  закрепляется относительным тестом.

# Knowledge

- Точный размер — пакетный обход корня каждого пункта `scan` с `AttributeSet::ExtendedWithPrivateSize`
  (getattrlistbulk отдаёт PRIVATESIZE пачкой; обход медленнее обычного в 1,8–2,6×, по замеру
  `SPEC-RECLAIMABLE-ESTIMATE`) — в потоке, без индекса: на вход идут физические пути пунктов. Пункт-
  файл — одним `getattrlist` с тем же набором атрибутов. Исчезнувший пункт — нулевой результат.
- Семантика замера = `Index::reclaimable` пункта: обычные файлы — `reclaimable_file` с PRIVATESIZE;
  жёсткие ссылки — группы (REALDEVID, FILEID) внутри пункта, `link_count` больше числа вхождений →
  `held_by_hard_links`; сжатые — `estimate_file`, поток клонов считается внутри пункта. Клоны сжатых
  файлов на APFS несут `EF_SHARES_ALL_BLOCKS` (ext 0x41, замер `recommendations-delta`), поэтому
  партнёр вне пункта не меняет результат; частично общий сжатый клон с партнёром вне пункта — единственное
  расхождение, оно в пользу «держат клоны».
- Лист открывается сразу: строки и удержание — `reclaimable_estimate`, итог помечен оценкой, строка
  «Уточняю размер: N файлов»; замер один в полёте, отменяется при изменении состава Корзины Домового и
  при закрытии листа; результат хранится до изменения состава — `refresh_tray` перерисовывает без
  прохода. «Переместить в Корзину» не ждёт: замер отменяется, «Перемещено» — точная сумма, если
  готова, иначе оценка.
- Перенос папки в Корзину — переименование; в `~/.Trash` она новая и читается целиком в фоне, поэтому
  большой перенос даёт тяжёлый листинг, как и `MustScanSubDirs` на корне.
- Тяжёлая пачка: листинги, в которых не меньше `heavy_nodes` узлов; `EventQueue` не применяет её, а
  отдаёт вызывающему; оболочка уносит `Session` в `ApplyJob` (`apply_listings` целиком: вклейка,
  уплотнение, карточки, пересопоставление); экран остаётся со старыми данными и строкой «Обновляю
  индекс…»; события ждут в канале, как при поиске дубликатов. Лёгкие пачки и синхронные
  Корзина/возврат/замена клоном применяются на главном потоке.
- Порог `HEAVY_DELTA_NODES` выводится замером: release, синтетический индекс 500 000 файлов, листинг
  из 10 000 / 50 000 / 200 000 новых узлов — время `Index::apply_delta` + `Recommendations::apply_delta`
  и время `Index::build`; порог — наибольший размер ряда, который применяется не дольше 100 мс
  (заметное замирание окна); таблица замера и выбранное число записываются сюда. Стоимость применения
  пропорциональна листингу, а не индексу, поэтому порог — число узлов, без доли от размера тома;
  уплотнение при мусоре ≥ живых по-прежнему редкое и на лёгкой пачке остаётся на главном потоке.
- Замер порога (19.10.2026, `heavy-apply`): синтетический индекс 500 000 файлов с карточкой `cargo.target`,
  листинг корня с новой папкой из N файлов, время `Index::apply_delta` + `Recommendations::apply_delta`
  (лучшее из трёх). Release: `Index::build` 80–84 мс; 10 000 узлов — 3,9 мс (0,05 сборки); 50 000 —
  21,7 мс (0,27); 200 000 — 98 / 124 / 107 мс в трёх прогонах (1,23–1,48). Debug: `Index::build` 1,02 с;
  10 000 — 61 мс (0,06); 50 000 — 305–324 мс (0,30–0,32); 200 000 — 1,26–1,31 с (1,24–1,28).
  200 000 узлов устойчиво не укладываются в 100 мс, поэтому `HEAVY_DELTA_NODES = 50 000`; тест
  `heavy_threshold` (debug) держит долю 0,32 × 1,5 ≈ 0,48 сборки.
- Гейт: `ShellAction::needs_session` — Корзина Домового (добавить, открыть, переместить), возврат,
  дубликаты, вопрос Домовому; пока `Session` в задаче (применение, поиск дубликатов, вопрос), `allows`
  их отклоняет; навигация, выбор карточек, диаграмма доступны.
- Граница гейтов: тесты доказывают равенство замера с `Index::reclaimable`, отмену и прогресс, уход
  тяжёлой пачки в поток с равенством полной сборке, отношение стоимости порога к `Index::build`,
  таблицу гейта. Живое окно без замираний на 31 ГБ выбранного и при переносе в Корзину — приёмка
  человеком после wire.

# Gameplan

### Phase 1: tray-measure — точный замер выбранного без индекса

<!-- deps: [] -->

- [ ] `index`: `measure_reclaimable(&Node) -> Reclaimable` по Knowledge.
- [ ] `app`: `TrayMeasureJob` — поток обходит физические корни пунктов (`ExtendedWithPrivateSize`,
  пункт-файл одним `getattrlist`), прогресс (файлы, байты) через канал, отмена `CancelToken`, результат —
  `Reclaimable` на пункт.
- [ ] Тест `crates/index/tests/measured_reclaimable.rs` (`TempTree`): обычные файлы, клон `cp -c` с
  партнёром внутри и снаружи, жёсткие ссылки внутри и снаружи, сжатый файл и его клон —
  `measure_reclaimable` скана с PRIVATESIZE равен `Index::reclaimable` по индексу того же скана.
- [ ] Тест `crates/app/tests/smoke_tray_measure.rs` (APFS-образ): замер папки и файла отдаёт прогресс,
  итог равен `Index::reclaimable`; отмена завершает поток досрочно; исчезнувший пункт — ноль.

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-index --test measured_reclaimable
```
- Замер по обходу равен `Index::reclaimable` (exit 0).
```
cargo test -p domovoi-app --test smoke_tray_measure
```
- Фоновый замер с прогрессом и отменой на настоящем томе (exit 0).
```
cargo clippy -p domovoi-index -p domovoi-app --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).

### Phase 2: tray-sheet — лист открывается сразу, точная цифра приходит из фона

<!-- deps: [tray-measure] — потребление: лист и шаг --e2e запускают настоящий TrayMeasureJob (smoke_rules_dups). -->

- [ ] `TrayModel`: строки и удержание по `reclaimable_estimate`, состояние замера (идёт: файлы и байты /
  готов); точные строки и удержание подставляются из результата; итог до конца замера помечен оценкой.
- [ ] Лист экрана 08: строка «Уточняю размер: N файлов» пока идёт замер.
- [ ] Оболочка: `open_tray` строит модель по оценке и запускает замер (один в полёте, отмена при смене
  состава и закрытии листа), таймер прогресса; `refresh_tray` перерисовывает из сохранённого замера.
- [ ] «Переместить в Корзину» по Knowledge; `Session::tray_reclaimable` и `Tray::reclaimable`
  удалены; шаг `--e2e` `trash_recommendations` меряет `TrayMeasureJob` и ждёт его.
- [ ] Фикстура `care` и `snapshot_care`: лист в состоянии замера и после него.
- [ ] `smoke_rules_dups`: Корзина Домового строится по оценке, замер сходится с `Index::reclaimable`
  пунктов, перемещение не требует замера.

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-app --test smoke_rules_dups
```
- REAL-DEP: Корзина Домового на настоящем томе меряется настоящим `TrayMeasureJob` (exit 0).
```
cargo test -p domovoi-app --test snapshot_care
```
- Лист экрана 08 в обоих состояниях замера (exit 0).
```
cargo clippy -p domovoi-rules -p domovoi-app --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).

### Phase 3: heavy-apply — тяжёлые пачки в фоне, действия ждут Session

<!-- deps: [tray-sheet] order-only — обе фазы правят shell_controller.rs. -->

- [ ] Замер порога по Knowledge (release, синтетический индекс 500 000 файлов, листинги 10 000 /
  50 000 / 200 000 узлов): таблица и выбранное `HEAVY_DELTA_NODES` записываются в Knowledge этой спеки.
- [ ] Тест `crates/app/tests/heavy_threshold.rs`: на синтетическом индексе 500 000 файлов применение
  листинга из `HEAVY_DELTA_NODES` новых узлов (индекс + карточки) не дороже доли `Index::build`, взятой из
  замера с запасом 1,5×.
- [ ] `EventQueue::new(heavy_nodes)`: тяжёлая пачка по Knowledge возвращается вызывающему вместе с
  листингами; очередь снимает её события только после применения; оболочка создаёт очередь с
  `HEAVY_DELTA_NODES`.
- [ ] `ApplyJob`: `Session` и пачка уходят в поток, `apply_listings` там, `Session` возвращается →
  `refresh`; строка «Обновляю индекс…» на главном экране.
- [ ] `ShellAction::needs_session` и гейт в `allows` по Knowledge; обработчики Корзины Домового,
  возврата и дубликатов не вызываются без `Session`.
- [ ] `smoke_volume_delta`: новый тест — на APFS-образе папка из многих файлов при низком `heavy_nodes`
  → очередь отдаёт тяжёлую пачку, `ApplyJob` применяет её в потоке, индекс и карточки равны полной
  сборке.
- [ ] Тест `crates/app/tests/session_gate.rs`: действия Корзины Домового, возврата, дубликатов и вопроса
  требуют `Session`, навигация и выбор карточек — нет.

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-app --test heavy_threshold
```
- Порог тяжёлой пачки закреплён замером относительно `Index::build` (exit 0).
```
cargo test -p domovoi-app --test smoke_volume_delta
```
- Тяжёлая пачка настоящего тома применяется в потоке и равна полной сборке (exit 0).
```
cargo test -p domovoi-app --test session_gate
```
- Гейт действий, которым нужна Session (exit 0).
```
cargo clippy -p domovoi-app --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).

### Phase 4: wire — Корзина Домового и тяжёлый перенос на главном пути из бандла

<!-- deps: [tray-measure, tray-sheet, heavy-apply] — потребление ВСЕХ code-фаз: wire-терминал по WIRE-PHASE-RULE. -->

- [ ] `crates/app/tests/wire_main_path.rs`: шаг `trash_recommendations` отчитывается точной суммой
  замера и числом измеренных файлов; после перемещения шаг `await_change` с низким `heavy_nodes` ждёт
  перенесённое в `.Trashes` образа и отчитывается, что пачка применена в `ApplyJob`.
- [ ] Прогрев гейта: `cargo xtask bundle --profile adhoc` выполняется задачей до гейта.
- [ ] Knowledge `docs/SPEC-DOMOVOI.md` [UI]: Корзина Домового — оценка сразу, точный замер в фоне;
  тяжёлые пачки — `ApplyJob`; гейт `Session`.

**Done when (EXECUTABLE):**
```
cargo xtask bundle --profile adhoc
```
- Бандл для e2e собран (exit 0).
```
cargo test -p domovoi-app --test wire_main_path
```
- Главный путь из бандла: замер Корзины и тяжёлый перенос в фоне (exit 0).
```
cargo xtask check-sources
```
- Гейт удаления и согласованность документов держатся (exit 0).
```
cargo clippy --workspace --all-targets -- -D warnings
```
- Весь workspace чист (exit 0).
