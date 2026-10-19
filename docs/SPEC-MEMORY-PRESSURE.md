<!--
  SPEC-MEMORY-PRESSURE.md — дельта к docs/SPEC-DOMOVOI.md: давление памяти не обрывает ответ
  ассистента и не выдаёт себя за «Стоп»; сервис и клиент согласованы о выгрузке; причина ошибки —
  в системном журнале. 4 фазы.

  ГРАНИЦА РЕШЕНИЯ (веер от 19.10.2026, выбран вариант A по рекомендации): сервис сам выгружает модель
  по WARN после простоя, CRITICAL отвечает OutOfMemory, клиент перезагружает модель по ModelNotLoaded,
  os_log через C-обёртку. Отвергнуты: B — уровень давления пересылается приложению (PROTOCOL_VERSION 2,
  канал сообщений без запроса в xpc-sys, решение о выгрузке в двух процессах — непропорционально);
  C — WARN не обрабатывается (модель ~3 ГБ остаётся в памяти до таймера приложения, пока система
  просит память, и подталкивает её к CRITICAL; правка спеки).
-->

# Baseline

- Предыдущая спека: `docs/SPEC-DOMOVOI.md`; дельты `SPEC-CLONE-TRASH-NAME`, `SPEC-RECLAIMABLE-ESTIMATE`,
  `SPEC-VOLUME-DELTA`, `SPEC-OFF-MAIN-THREAD` применены.
- Prior spec-hash: sha256:<placeholder — confirm>
- Проблема (приёмка 19.10.2026, Mac 24 ГБ, `kern.memorystatus_vm_pressure_level` = 2, своп активен):
  первый вопрос — «Ответ остановлен.», хотя «Стоп» не нажимали; второй — «Модель не смогла
  ответить.». Причины в коде:
  - `InferenceService::report_pressure` на любом уровне выше Normal поднимает общий `CancelFlag`:
    WARN обрывает идущую загрузку (`LoadCancelled`) и генерацию (`StopReason::Cancelled` →
    `AgentError::Cancelled`), обе показываются как «Ответ остановлен.». Knowledge [каталог]
    `SPEC-DOMOVOI` велит на WARN только выгружать модель.
  - Сервис выгружает модель молча (WARN, CRITICAL, простой `idle_unload`); `InferenceClient::loaded`
    и `Assistant::loaded_entry` остаются выставленными, следующий вопрос идёт без `Load` →
    `ModelNotLoaded` → «Модель не смогла ответить.». «Следующий запрос перезагружает» не работает.
  - После CRITICAL `refusing_loads` снимается только на Normal; при спаде CRITICAL → WARN загрузка
    отказана, пока система не вернётся к норме.
  - `OutOfMemory` показывается общим «Модель не смогла ответить.»; описание ошибки отбрасывается,
    `eprintln` XPC-сервиса в журнал не попадает, `os_log` в проекте нет.
  - Тестов реакции на давление нет.
- Уже есть и не меняется: протокол (`PROTOCOL_VERSION` 1, `ErrorCode::OutOfMemory` есть); пользовательский
  «Стоп» → `Cancel` → «Ответ остановлен.»; `MemoryBudget` (отказ в загрузке класса не по бюджету);
  таймер выгрузки приложения (`UnloadDelay`); `idle_unload` сервиса; сторож зависания.
- Версия: фазы повышения версии нет — `cargo xtask release` сам инкрементирует патч;
  `.github/workflows/ci.yml` версию не меняет.
- Обоснование выбора — staged decision note (id после accept).

# Stack

- Новый крейт `crates/log-sys` (`domovoi-log-sys`): `shim.c` (`os_log_create`, `os_log_with_type` с
  форматом `%{public}s`) собирается `cc`; Rust-API `SystemLog::new(subsystem, category)`,
  `notice(&str)`, `error(&str)`. Workspace: член, зависимость `domovoi-log-sys`, `cc = "1.8"` (уже в
  `Cargo.lock`).
- `crates/inference`: `pub const PRESSURE_IDLE_GRACE` из замера; `InferenceService` — прерывание
  давлением отдельно от отмены, `Worker` — отложенная выгрузка по WARN, снятие отказа ниже CRITICAL,
  журнал `com.domovoi.inference`/`memory`.
- `InferenceBackend::count_tokens` принимает `&mut self` (фаза `honest-answers`): агент считает токены
  до каждой генерации, и клиенту нужна перезагрузка по `ModelNotLoaded` и здесь; `Conversation::render`
  берёт `&mut dyn InferenceBackend`.
- `crates/app`: `InferenceClient` — перезагрузка по `ModelNotLoaded` и журнал ошибок сервиса
  `com.domovoi.app`/`inference`; `ChatModel::record_failure` — `OutOfMemory` → `ANSWER_LOW_MEMORY`;
  `strings.rs` — `ANSWER_LOW_MEMORY`.
- Тесты: новые `crates/log-sys/tests/unified_log.rs`, `crates/agent/tests/step_gap.rs`,
  `crates/inference/tests/memory_pressure.rs`, `crates/app/tests/answer_failures.rs`,
  `crates/app/tests/wire_assistant_pressure.rs`; меняется `crates/app/tests/smoke_inference_client.rs`.

# Conventions

- Давление памяти никогда не выдаёт себя за действие пользователя: «Ответ остановлен.» — только
  после «Стоп» или выключения ассистента.
- Работа, прерванная давлением, и отказ в загрузке отвечают `OutOfMemory`; текст пользователю — один,
  `ANSWER_LOW_MEMORY`.
- Выгрузка модели сервисом невидима для приложения по построению: клиент сам восстанавливает модель
  при `ModelNotLoaded`, если держит дескриптор весов; без него ошибка уходит наверх как есть.
- Каждая ошибка сервиса, которую видит клиент, пишется в `os_log` с кодом и описанием; смену уровня
  давления и отказы пишет сервис. Описания — наши строки, без путей пользователя, поэтому `public`.
- Пауза выгрузки по WARN не задаётся на глаз: она выводится замером промежутков между вызовами модели
  внутри ответа и закрепляется тестом.
- Реакция на давление проверяется тестом с подачей уровня через `InferenceService::report_pressure` —
  это граница системы (источник `DISPATCH_SOURCE_TYPE_MEMORYPRESSURE`).

# Knowledge

- WARN: флаг отмены не поднимается; рабочий поток ставит отметку «выгрузить в простое» и выгружает
  модель, если `PRESSURE_IDLE_GRACE` не пришло ни одной команды (`recv_timeout` берёт меньшее из
  `idle_unload` и паузы). Ответ из нескольких шагов доживает до конца, модель уходит из памяти через
  паузу после него. Normal снимает отметку.
- Замер паузы: синтетический индекс 500 000 файлов (построение как в `heavy_threshold`), настоящие
  `RuleSet` и `Denylist`, `Agent::new(..).calling_a_tool_first().ask(..)` с записывающей тестовой моделью:
  сценарий шагов вызывает каждый инструмент (`query_index` по всему тому, `inspect_node` самой крупной
  папки, `rule_explain`, `summarize`) и завершается ответом; промежуток — от возврата одного вызова
  модели (`generate` / `count_tokens`) до начала следующего; лучшее из трёх прогонов на шаг, release и
  debug. Пауза — наибольший промежуток debug с запасом ×4, округлённый вверх до 0,5 с, но не меньше
  1 с (тактирование потоков и XPC в замер внутри процесса не входят); тест `step_gap` (debug) держит
  наибольший промежуток ≤ паузы / 2. Таблица замера и выбранное число записываются сюда.
- Замер паузы (19.10.2026, `pressure-service`, лучшее из трёх, наибольший промежуток шага): вопрос →
  первый вызов — release 0,05 мс, debug 0,12 мс; `query_index` с `level: check_yourself` по всему тому
  (совпадений нет — обход всех 500 000 узлов с уровнем правила на каждом) — release 287–289 мс,
  debug 1,95–1,96 с; `inspect_node` `engine/target` (1 000 детей) — 0,07 / 0,42 мс; `rule_explain` —
  0,05 / 0,31 мс; `summarize` — 7,1–7,2 / 106–108 мс. Наибольший debug 1,964 с × 4 = 7,86 с →
  `PRESSURE_IDLE_GRACE = 8 с`; `step_gap` держит ≤ 4 с. Обход `query_index` линеен по индексу: на
  Data-томе приёмочного Mac 4,6 млн объектов (`df -i`) — оценка release ≈ 2,7 с; 8 с покрывают
  release-обход до ≈ 14 млн узлов.
- CRITICAL: флаг отмены поднимается вместе с отметкой «прервано давлением»; рабочий поток после
  загрузки или генерации переводит `LoadCancelled` / `StopReason::Cancelled` при поднятой отметке в
  `Error{OutOfMemory, "memory pressure is critical"}`; модель выгружается, загрузки отказаны. Отметка
  опускается вместе с флагом перед каждой `Load` / `Generate`.
- Отказ после CRITICAL снимается любым уровнем ниже CRITICAL (WARN или Normal): на многих Mac WARN
  держится часами, отказ «до нормы» означал бы ассистента без ответа на всё это время. Если загрузка
  снова поднимет давление до CRITICAL, сработает аварийная выгрузка.
- Клиент: `generate` и `count_tokens` при `ModelNotLoaded` и сохранённых `LoadedWeights` один раз
  повторяют `load_weights` с клоном дескриптора (как `reconnect`) и повторяют запрос; отказ в
  перезагрузке (`OutOfMemory`) уходит наверх. Под постоянным WARN каждый вопрос перезагружает модель —
  несколько секунд, с прогрессом загрузки.
- `ANSWER_LOW_MEMORY` = «Модели не хватает памяти. Закройте тяжёлые приложения или выберите модель
  поменьше и спросите ещё раз.» — честен и для отказа по давлению, и для класса не по бюджету.
- Журнал: `log show --predicate 'subsystem BEGINSWITH "com.domovoi"'`. Сервис — `notice` на смену
  уровня, `error` на отказ и прерывание; клиент — `error` на каждый `Message::Error` и на
  остановку зависшего сервиса, `notice` на перезагрузку по `ModelNotLoaded`.
- Граница гейтов: тесты доказывают семантику уровней на настоящем `InferenceService` с подставной
  моделью, паузу замером, перезагрузку клиентом, тексты, запись в журнал и главный путь вопроса с
  давлением через настоящие `Session::ask` + `InferenceClient` + `InferenceService`. Путь от настоящего
  сигнала ядра (`MemoryPressureMonitor`) не доказуем тестом: симуляция `memory_pressure -S` требует
  root, а кому слать предупреждение, решает ядро. Это приёмка человеком после wire: на Mac под WARN
  вопрос получает ответ, `log show` показывает смену уровня.

# Gameplan

### Phase 1: system-log — запись в единый журнал macOS

<!-- deps: [] -->

- [ ] Крейт `crates/log-sys`: `shim.c` + `build.rs` (`cc`), `SystemLog::new(subsystem, category)`,
  `notice`, `error`; строка передаётся с `%{public}s`; NUL внутри строки не обрывает запись.
- [ ] Workspace: член и зависимости по Stack.
- [ ] Тест `crates/log-sys/tests/unified_log.rs`: запись `error` с уникальной меткой читается
  `/usr/bin/log show --last 1m` по `processID` и `subsystem` (опрос до `test_timeout`).

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-log-sys --test unified_log
```
- Запись видна в едином журнале (exit 0).
```
cargo clippy -p domovoi-log-sys --all-targets -- -D warnings
```
- Typecheck-критерий фазы и нового крейта (exit 0).

### Phase 2: pressure-service — уровни давления в сервисе по Knowledge

<!-- deps: [system-log] — потребление: сервис пишет журнал через настоящий SystemLog (memory_pressure читает запись). -->

- [ ] Замер паузы по Knowledge (release и debug): таблица промежутков по инструментам и выбранное
  `PRESSURE_IDLE_GRACE` записываются в Knowledge этой спеки; константа — `pub` в `domovoi-inference`.
- [ ] Тест `crates/agent/tests/step_gap.rs`: сценарий замера (debug) — наибольший промежуток между
  вызовами модели внутри ответа ≤ `PRESSURE_IDLE_GRACE / 2`.
- [ ] `InferenceService::report_pressure`: WARN — без флага отмены; CRITICAL — флаг и отметка
  «прервано давлением».
- [ ] `Worker`: отложенная выгрузка по WARN после `PRESSURE_IDLE_GRACE`; перевод прерванного давлением
  в `OutOfMemory`; снятие отказа ниже CRITICAL; Normal снимает отметку выгрузки.
- [ ] Журнал `com.domovoi.inference`/`memory` по Knowledge.
- [ ] Тест `crates/inference/tests/memory_pressure.rs` (`AnonymousService` + `FakeBackend`; для
  загрузки — тестовая модель, чья `load` ждёт флага отмены):
  WARN во время генерации — ответ дописан (`EndOfGeneration`), вторая генерация в пределах паузы
  идёт без загрузки, после паузы — `ModelNotLoaded`; CRITICAL во время генерации и во время загрузки —
  `OutOfMemory`; после CRITICAL загрузка отказана `OutOfMemory`, после WARN разрешена; `Cancel`
  пользователя — по-прежнему `StopReason::Cancelled` / `LoadCancelled`; отказ после CRITICAL виден в
  журнале процесса.

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-agent --test step_gap
```
- Пауза выгрузки закреплена замером промежутков ответа (exit 0).
```
cargo test -p domovoi-inference --test memory_pressure
```
- Семантика WARN / CRITICAL / Normal и REAL-DEP запись настоящего SystemLog (exit 0).
```
cargo clippy -p domovoi-inference -p domovoi-agent --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).

### Phase 3: honest-answers — клиент перезагружает модель, ошибки названы честно

<!-- deps: [pressure-service, system-log] — потребление: smoke_inference_client гоняет настоящий InferenceService с давлением; клиент пишет журнал через настоящий SystemLog. -->

- [ ] `InferenceClient`: перезагрузка и повтор по `ModelNotLoaded` по Knowledge; журнал
  `com.domovoi.app`/`inference`.
- [ ] `ANSWER_LOW_MEMORY` в `strings.rs`; `ChatModel::record_failure`: `OutOfMemory` →
  `ANSWER_LOW_MEMORY`; `LoadCancelled` и `AgentError::Cancelled` — «Ответ остановлен.».
- [ ] `smoke_inference_client`: новый тест — после WARN и паузы клиент генерирует без явной загрузки
  (перезагрузка сама); после CRITICAL генерация — `OutOfMemory`; ошибка сервиса видна в журнале.
- [ ] Тест `crates/app/tests/answer_failures.rs`: таблица `AgentError` / `ErrorCode` → текст чата.

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-app --test smoke_inference_client
```
- REAL-DEP: клиент против настоящего InferenceService под давлением (exit 0).
```
cargo test -p domovoi-app --test answer_failures
```
- Тексты отказов (exit 0).
```
cargo test -p domovoi-app --test voice
```
- Новая строка держит голос продукта (exit 0).
```
cargo clippy -p domovoi-app --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).

### Phase 4: wire — вопрос под давлением на главном пути

<!-- deps: [system-log, pressure-service, honest-answers] — потребление ВСЕХ code-фаз: wire-терминал по WIRE-PHASE-RULE. -->

- [ ] `crates/app/tests/wire_assistant_pressure.rs`: `Session` по скану `TempTree`, `Session::ask` +
  `InferenceClient` + `AnonymousService` (`FakeBackend` со сценарием шагов агента): WARN посреди
  ответа — ответ получен; CRITICAL — `ChatModel` показывает `ANSWER_LOW_MEMORY`; после спада до WARN
  следующий вопрос отвечен с перезагрузкой; ошибки видны в журнале.
- [ ] Прогрев гейта: `cargo xtask bundle --profile adhoc` выполняется задачей до гейта.
- [ ] Knowledge `docs/SPEC-DOMOVOI.md` [каталог]: правило WARN / CRITICAL по этой спеке, пауза из
  замера, перезагрузка клиентом, журнал `com.domovoi.*`.

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-app --test wire_assistant_pressure
```
- Главный путь вопроса под давлением на настоящих модулях (exit 0).
```
cargo xtask bundle --profile adhoc
```
- Бандл для e2e собран (exit 0).
```
cargo test -p domovoi-app --test wire_main_path
```
- Вопрос из бандла на настоящей модели не сломан перезагрузкой клиента (exit 0).
```
cargo xtask check-sources
```
- Согласованность документов держится (exit 0).
```
cargo clippy --workspace --all-targets -- -D warnings
```
- Весь workspace чист (exit 0).
