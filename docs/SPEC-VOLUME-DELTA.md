<!--
  SPEC-VOLUME-DELTA.md — дельта к docs/SPEC-DOMOVOI.md: события FSEvents применяются к индексу
  инкрементально, главный поток не делает работы по размеру индекса. 5 фаз.

  ГРАНИЦА РЕШЕНИЯ (веер от 19.10.2026, выбран вариант A по рекомендации): неглубокие листинги в
  фоновом потоке, вклейка в индекс без уплотнения, инкрементальные derive и рекомендации на главном
  потоке. Отвергнуты: B — фоновое задание с переездом Session (полный derive на каждую пачку: секунды
  ЦП в простое и отложенные клики); C — неглубокий листинг в главном потоке с троттлингом (O(N)
  compact, derive и claims остаются — замирания на 1–3 с).
-->

# Baseline

- Предыдущая спека: `docs/SPEC-DOMOVOI.md`; дельты `docs/SPEC-CLONE-TRASH-NAME.md` и
  `docs/SPEC-RECLAIMABLE-ESTIMATE.md` применены.
- Prior spec-hash: sha256:436c2875f0a255f539b987902f05c133808a53f54d36f0ad668b0c8a35032f9d
- Проблема (приёмка 19.10.2026, 4,8 млн файлов): `events_tick` (главный поток, раз в 0,5 с) →
  `Session::apply_events` → `rescan_paths` сводит пути к самому внешнему предку → `Index::rescan`
  читает каталог рекурсивно (событие в `~` — обход всего дома) → на каждый путь `replace_children`,
  `compact` (переинтернирование всех имён) и полный `derive` → полный `Recommendations::build`.
  История событий за время скана приходит одной пачкой на первом тике.
- Уже есть и не меняется: полный скан (`Session::scan`, `ScanJob`); `VolumeEventStream` (NoDefer,
  WatchRoot, задержка 0,2 с, `rescan_subtree` = MustScanSubDirs | UserDropped | KernelDropped);
  RootChanged/Unmount → полный скан; запись кэша при скане и выходе; `reclaimable_estimate` для
  карточек; исчезнувший каталог читается от ближайшего предка, который есть и в индексе, и на диске.
- Версия: фазы повышения версии нет — `cargo xtask release` сам инкрементирует патч; CI версию не меняет.
- Обоснование выбора — staged decision note (id после accept).

# Stack

- `crates/scanner`: `list_directory(path, &ScanOptions) -> Result<Vec<Node>, ScanError>` — записи
  каталога одним уровнем с теми же атрибутами и состояниями, что у обхода.
- `crates/index`: `ChangedPath { path, subtree }`; `Index::delta_request(&[ChangedPath]) -> DeltaRequest`
  (цели и поколение индекса); `DeltaTarget::read(&ScanOptions) -> Result<DirectoryListing, ScanError>`
  без доступа к индексу, листинг знает число каталогов, прочитанных вглубь; `Index::apply_delta(
  &DeltaRequest, Vec<DirectoryListing>, &Enrichment) -> Result<DeltaApplied, StaleDelta>`; уплотнение
  по порогу; `Index::hidden_folders` — счётчик; `Index::rescan` удаляется (фаза event-job).
- `crates/rules`: `Recommendations::apply_delta(…, &DeltaApplied)`.
- `crates/app`: `Session::delta_request`, `Session::apply_listings`; синхронный `Session::apply_events`
  и `rescan_paths` (`--e2e`, Корзина, возврат, замена клоном) через те же функции; `DeltaJob` (чтение в
  потоке, применение на главном); `Session::persist(&mut self)` уплотняет перед записью.
- Тесты: новые `crates/index/tests/delta_read.rs`, `crates/index/tests/delta_equivalence.rs`,
  `crates/index/tests/delta_scale.rs`, `crates/rules/tests/recommendations_delta.rs`,
  `crates/app/tests/smoke_volume_delta.rs`; меняются `crates/scanner/tests/fixtures_tempdir.rs`,
  `crates/index/tests/cache_roundtrip.rs`, `crates/rules/tests/recommendations_scale.rs`,
  `crates/app/tests/wire_main_path.rs`.

# Conventions

- После события главный поток не читает диск рекурсивно и не проходит по всем узлам индекса: чтение —
  в потоке `DeltaJob`, применение — пропорционально изменению. Исключения: уплотнение (Knowledge) и
  синхронные действия пользователя (Корзина, возврат, замена клоном).
- Инкрементальный результат совпадает с полной сборкой: тесты сравнивают дельту с `Index::build` и
  `Recommendations::build` по путям, а не по `NodeId`.
- Проверки скорости — относительные, к `Index::build` или `Recommendations::build` того же отчёта.
- Файлы уходят только через Корзину; гейт `check-sources` держится.

# Knowledge

- Событие каталога без `rescan_subtree` значит «изменились его прямые записи». Цель читается одним
  уровнем; дочерний каталог с тем же именем и FILEID сохраняет поддерево из индекса; вглубь читаются
  только новые и заменённые (другой FILEID) дочерние каталоги и цели с `rescan_subtree`.
- Сведение путей пачки: путь → ближайший каталог-предок, который есть в индексе и на диске; одинаковые
  цели сливаются (`subtree`, если его несёт хоть одно событие); цели внутри цели с `subtree`
  отбрасываются. Предок не поглощает потомков: изменение прямо в `~` читает только `~`.
- Вклейка: новый блок детей изменённого каталога дописывается в конец арены (дети лежат подряд и
  отсортированы по байтам имени — `child_named` ищет бинарно), старый блок становится мусором. Прямые
  дети получают новые `NodeId`, поддеревья сохранённых каталогов — прежние (у их детей обновляется
  `parent`). Мусор не участвует ни в одном проходе по узлам: `len`, агрегаты, группы, счётчики.
- Инкрементальный `derive`: категории и владельцы — для новых узлов и для каталога, у которого
  изменился набор маркеров проекта (тогда для его поддерева); `allocated`, `files`, `newest_*` —
  разницей вверх по цепочке предков; канон и бейджи клонов и жёстких ссылок — только для групп с
  пришедшими или ушедшими инодами, с поправкой агрегатов у предков всех затронутых членов. Группы
  живут в картах, которые строят полный `derive` и открытие кэша.
- Уплотнение — когда мусор не меньше живых узлов (амортизированно O(1) на дописанный узел) и в
  `Session::persist`. После него все `NodeId` недействительны: Session пересопоставляет Корзину
  Домового и дубликаты по путям и пересобирает рекомендации полностью. Это единственная работа по
  размеру индекса на главном потоке; на 4,8 млн узлов она случается раз на ~4,8 млн дописанных узлов.
- Поколение: `DeltaRequest` несёт поколение индекса. Если индекс изменился между запросом и
  применением (Корзина, возврат, замена клоном), `apply_delta` отвечает `StaleDelta`, пути уходят в
  следующую пачку — устаревший листинг не воскрешает перемещённое.
- Рекомендации: заявка зависит от пути, маркеров-соседей и условия правила. Дельта каталога D
  переоценивает его прямых детей; вглубь идёт только по новым детям и по детям, чей статус заявки
  сменился. Пункты, ставшие мусором, уходят из карточек; карточка, чей пункт содержит D,
  переоценивается — `reclaimable_estimate` и вердикт denylist этого пункта; итоги по уровням — из
  карточек. Уплотнение → полный `Recommendations::build`.
- Удержанные `NodeId` (Корзина Домового, группы дубликатов) пересопоставляются по пути, только если
  узел отцеплен; фокус диаграммы и так держится путём.
- Новые поля `Index` меняют формат кэша — версия формата в заголовке повышается, старый кэш
  сбрасывается полным сканом.
- Граница гейтов: они доказывают охват чтения, равенство с полной сборкой и отношение времени. Живое
  окно без замираний при постоянных событиях в `~/Library` — приёмка человеком после wire.

# Gameplan

### Phase 1: delta-read — неглубокое чтение и сведение путей

<!-- deps: [] -->

- [ ] `scanner`: `list_directory` и тест в `crates/scanner/tests/fixtures_tempdir.rs` — один уровень,
  атрибуты как у обхода, дочерний каталог без детей.
- [ ] `index`: `ChangedPath`, `Index::delta_request` (сведение путей по Knowledge, поколение,
  известные дети цели — имя и FILEID), `DeltaTarget::read`, `DirectoryListing` с числом каталогов,
  прочитанных вглубь.
- [ ] Тест `crates/index/tests/delta_read.rs` (`TempTree`): изменение прямо в корне даёт одну цель без
  `subtree`; вложенные изменённые каталоги — отдельные цели; цель с `subtree` поглощает цели внутри;
  исчезнувший каталог — цель у существующего предка; одинаковые цели сливаются с `subtree`. Чтение:
  сохранённые дочерние каталоги не читаются вглубь (счётчик 0), новый и заменённый (другой FILEID)
  читаются целиком, цель с `subtree` — целиком.

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-scanner --test fixtures_tempdir
```
- Каталог читается одним уровнем с атрибутами обхода (exit 0).
```
cargo test -p domovoi-index --test delta_read
```
- Пути пачки сводятся без поглощения потомков, вглубь читаются только новые каталоги (exit 0).
```
cargo clippy -p domovoi-scanner -p domovoi-index --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).

### Phase 2: delta-apply — инкрементальная вклейка в индекс

<!-- deps: [delta-read] — потребление: вклеиваются листинги настоящего DeltaTarget::read (delta_equivalence). -->

- [ ] `index`: `Index::apply_delta` (вклейка, проверка поколения, инкрементальный `derive`, счётчик
  скрытых папок), карты групп клонов и жёстких ссылок, уплотнение по порогу, повышение версии формата
  кэша.
- [ ] Тест `crates/index/tests/delta_equivalence.rs` (`TempTree`, листинги из настоящего
  `DeltaTarget::read`): новый, удалённый и изменённый файл; новый каталог с поддеревом; удалённый
  каталог; каталог заменён (то же имя, другой FILEID); появился и исчез `Cargo.toml` (категория Dev);
  клон `cp -c` и жёсткая ссылка (канон, бейджи, `allocated` предков); цель с `subtree`; устаревший
  запрос → `StaleDelta`. После каждого шага дамп живых узлов по путям равен `Index::build` свежего
  скана, `hidden_folders` равен полному подсчёту.
- [ ] Тест `crates/index/tests/delta_scale.rs`: синтетический индекс на 500 000 файлов; листинг корня с
  сохранённым большим поддеревом и листинг каталога из 100 записей применяются каждый не дольше 1 %
  `Index::build` того же отчёта; уплотнение по порогу даёт индекс, равный свежему.

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-index --test delta_equivalence
```
- REAL-DEP: дельта из настоящих листингов равна полной сборке на всех сценариях (exit 0).
```
cargo test -p domovoi-index --test delta_scale
```
- Дельта каталога на индексе из 500 000 файлов не дороже 1 % его сборки (exit 0).
```
cargo test -p domovoi-index --test cache_roundtrip
```
- Кэш с новым форматом и прежний путь пересканирования не сломаны (exit 0).
```
cargo clippy -p domovoi-index --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).

### Phase 3: recommendations-delta — карточки по дельте

<!-- deps: [delta-apply] — потребление: карточки обновляются по DeltaApplied настоящего Index::apply_delta (recommendations_delta). -->

- [ ] `rules`: `Recommendations::apply_delta` по Knowledge; после уплотнения — полный `build`.
- [ ] Тест `crates/rules/tests/recommendations_delta.rs` (`TempTree`, настоящий `apply_delta`):
  `Cargo.toml` появился рядом с `target/` → карточка `cargo.target`; файл добавлен в `target/` → оценка
  растёт; `node_modules` удалён → карточка `dev.deps` уходит; пункт с `uchg` → `blocked`. После каждого
  шага равенство с `Recommendations::build` по правилам, путям пунктов, оценке и итогам.
- [ ] Тест `crates/rules/tests/recommendations_scale.rs`: на индексе из 500 000 файлов дельта каталога
  вне карточек не дольше 1 % `Index::build`, дельта внутри `target/` не дольше одного
  `Recommendations::build`.

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-rules --test recommendations_delta
```
- REAL-DEP: карточки по дельте настоящего индекса равны полной сборке (exit 0).
```
cargo test -p domovoi-rules --test recommendations_scale
```
- Дельта карточек не дороже порогов на 500 000 файлах (exit 0).
```
cargo test -p domovoi-rules --test smoke_index
```
- Полная сборка карточек по настоящему индексу не сломана (exit 0).
```
cargo clippy -p domovoi-rules --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).

### Phase 4: event-job — события читаются в фоне, применяются дельтой

<!-- deps: [recommendations-delta] — потребление: Session применяет настоящие Index::apply_delta и Recommendations::apply_delta к событиям настоящего VolumeEventStream (smoke_volume_delta). -->

- [ ] `app` `Session`: `delta_request(events)` (RootChanged/Unmount → `RescanRequired`),
  `apply_listings(request, listings)` — индекс, рекомендации, пересопоставление только отцепленных
  `NodeId`, `last_event_id`; `StaleDelta` → пути остаются в очереди; синхронные `apply_events` и
  `rescan_paths` через те же функции; `persist(&mut self)` уплотняет; `Index::rescan` удалён.
- [ ] `app` оболочка: `DeltaJob` — одно чтение в полёте; `events_tick` копит события, запускает чтение
  в потоке, применяет готовые листинги на главном потоке и обновляет экран; пока Session в задаче
  (скан, дубликаты, вопрос), события ждут, как сейчас.
- [ ] `ShellData.hidden_folders` — из счётчика индекса.
- [ ] `crates/index/tests/cache_roundtrip.rs`: тесты дельты переходят на `delta_request`/`read`/`apply_delta`.
- [ ] Тест `crates/app/tests/smoke_volume_delta.rs` (APFS-образ, browsable, настоящий
  `VolumeEventStream`): Session сканирует образ; файл создаётся прямо в корне, каталог с файлами — в
  глубине, `Cargo.toml` — рядом с `target/`, папка удаляется; события → `delta_request` → чтение в
  отдельном потоке → `apply_listings`. Индекс содержит изменения, вглубь прочитаны только новые
  каталоги, рекомендации равны полной сборке; перемещение в Корзину между запросом и применением даёт
  `StaleDelta`, перемещённое не возвращается.

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-app --test smoke_volume_delta
```
- REAL-DEP: настоящие события образа применяются дельтой из фонового чтения (exit 0).
```
cargo test -p domovoi-index --test cache_roundtrip
```
- Кэш и дельты после удаления `Index::rescan` (exit 0).
```
cargo test -p domovoi-app --test smoke_rules_dups
```
- Корзина Домового и замена клоном идут через дельты (exit 0).
```
cargo test -p domovoi-app --test smoke_render_scanner
```
- Скан и открытие кэша не сломаны (exit 0).
```
cargo clippy -p domovoi-index -p domovoi-rules -p domovoi-app --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).

### Phase 5: wire — дельты на главном пути из бандла

<!-- deps: [event-job] — потребление ВСЕХ code-фаз: wire-терминал по WIRE-PHASE-RULE. -->

- [ ] `crates/app/tests/wire_main_path.rs`: шаг `await_change` отчитывается числом каталогов,
  прочитанных вглубь; сценарий после скана создаёт файл прямо в корне образа и в глубине, оба доходят
  до индекса без чтения вглубь.
- [ ] Прогрев гейта: `cargo xtask bundle --profile adhoc` выполняется задачей до гейта.
- [ ] Knowledge `docs/SPEC-DOMOVOI.md`: пункт [UI] про `Index::rescan` и `compact` заменить правилами
  дельт.

**Done when (EXECUTABLE):**
```
cargo xtask bundle --profile adhoc
```
- Бандл для e2e собран (exit 0).
```
cargo test -p domovoi-app --test wire_main_path
```
- Главный путь из бандла применяет дельты без чтения вглубь (exit 0).
```
cargo xtask check-sources
```
- Гейт удаления и согласованность документов держатся (exit 0).
```
cargo clippy --workspace --all-targets -- -D warnings
```
- Весь workspace чист (exit 0).
