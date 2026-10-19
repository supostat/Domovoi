<!--
  SPEC-AGENT-PATHS.md — дельта к docs/SPEC-DOMOVOI.md: агент знает домашнюю папку и пути правил,
  пустой запрос говорит, есть ли такой путь. 3 фазы.

  ГРАНИЦА РЕШЕНИЯ (веер от 19.10.2026, выбран вариант A по рекомендации; на ревью спеки диагностика
  сужена до prefix_found — обход вариантов фильтров в index не делается): home в сводке и AgentContext,
  `~/` в путях инструментов и гипотез, нейтральный пример промпта, rule_explain с paths и found, пустой
  query_index с prefix_found. Отвергнуты: B — правила с находками прямо в сводке (+0,5–1 тыс. токенов на
  каждом шаге, ≈ +0,5 с на шаг, молчаливые нули остаются); C — только home и пример (выдумывать «igor»
  модель перестанет, но pnpm так и не найдёт).
-->

# Baseline

- Предыдущая спека: `docs/SPEC-DOMOVOI.md`; дельты по `SPEC-CARDS-LAYOUT` применены.
- Prior spec-hash: sha256:<placeholder — confirm>
- Проблема (приёмка 19.10.2026): на «что насчёт хранилища pnpm?» модель искала `/Users/igor/.pnpm`,
  `/Users/igor/.npm`, `/Users/igor/Library/Caches/pnpm` с `category: dev`, `level: safe` и получила 0 узлов,
  хотя хранилище есть (`~/Library/pnpm/store`, 14 ГБ, правило `cache.pnpm-store`, уровень Recoverable).
  Причины в коде:
  - пример ответа в `system_prompt` (`crates/grammar/src/system_prompt.rs`) — `/Users/igor/Library/…`;
    сводка индекса (`ToolRunner::overview`) путей не содержит — домашней папки модель не знает;
  - инструменты принимают только абсолютные пути; `~/` правил — шаблон `Users/*/…`;
  - `rule_explain` отдаёт пояснение и уровень, но не пути правила и не найденные на диске папки;
  - пустой `query_index` отдаёт `total: 0` и не говорит, есть ли такой путь в индексе;
  - гипотеза с `~/` отбрасывается фильтром «путь есть в индексе».
- Уже есть и не меняется: схема ответа и аргументов инструментов (`domovoi-grammar`), `Index::query`,
  действия только из узлов инструментов, гипотезы только по путям индекса, `PRESSURE_IDLE_GRACE` = 8 с.
- Версия: фазы повышения версии нет — `cargo xtask release` сам инкрементирует патч;
  `.github/workflows/ci.yml` версию не меняет.
- Обоснование выбора — staged decision note (id после accept).

# Stack

- `crates/rules`: `Rule::paths()` — шаблоны правила текстом, как в таблице (`~/Library/pnpm/store`,
  `**/target|Cargo.toml`), плюс абсолютные пути из `ConfigOverrides` (`also_at`).
- `crates/grammar`: пример гипотезы в `system_prompt` — `~/Library/Application Support/com.tinyapp.helper`.
- `crates/agent`: `AgentContext::home`; `"home"` в сводке; разворот `~/` в `path_prefix`, `path` и путях
  гипотез; `rule_explain` + `paths`, `found`, `found_total`; пустой `query_index` + `prefix_found`; две
  строки в `INSTRUCTIONS`.
- `crates/app`: `SessionPaths::home` (`for_user` — домашняя папка пользователя, `in_directory` — сам
  каталог, `with_home(path)`); `Session::ask` передаёт его в `AgentContext`.
- `crates/eval`: `AgentContext::home` = `FIXTURE_HOME`; в фикстуре — `Library/pnpm/store`, в `tasks.json` —
  задача про pnpm.
- Тесты: новые `crates/rules/tests/rule_paths.rs`, `crates/agent/tests/paths.rs`,
  `crates/app/tests/wire_agent_paths.rs`; меняется `crates/grammar/tests/response.rs`.

# Conventions

- В системном промпте нет имени ни одного пользователя: пути примеров — через `~/`.
- Пути в результатах инструментов всегда абсолютные; `~/` принимается только на входе.
- Новые поля инструментов не меняют паузу выгрузки по WARN (`PRESSURE_IDLE_GRACE`): `step_gap` её охраняет.

# Knowledge

- `home`: в приложении — домашняя папка пользователя (`SessionPaths::for_user`), в эвале — `FIXTURE_HOME`
  (`/Users/fixture`), в тестах — `SessionPaths::with_home`. Сводка индекса: `{"home": "/Users/ingvar",
  "used_bytes": …, "reclaimable": …, "categories": …}`.
- Разворот: путь `~` или `~/<хвост>` → `home` или `home/<хвост>`; остальные пути не меняются. Применяется к
  `query_index.path_prefix`, `inspect_node.path` и `path` гипотез до проверки «путь есть в индексе».
- `rule_explain(id)`: прежние поля + `paths` (`Rule::paths()`), `found` — до 5 узлов, на которые правило
  заявлено, по убыванию `allocated` (описание как у `query_index`), `found_total` — сколько всего.
- Пустой `query_index` с `path_prefix`: + `prefix_found` (`true`, если префикс есть в индексе) — один
  `Index::find`; без префикса поле не выводится.
- `INSTRUCTIONS` (+2 строки): «Домашняя папка пользователя — поле home сводки; пути в ней можно писать
  через ~/.» и «Фильтры query_index сужают поиск: если узлов 0, ослабь фильтры; prefix_found: false
  значит, что такого пути в индексе нет.»
- Эвал: в фикстуре `Library/pnpm/store/v10/files/00/data.bin` (300 МБ — меньше гигабайта, чтобы не сдвинуть
  эталоны задач про кэши больше 1 ГБ, крупнейшие пункты и итог «восстановимо»); задача `query-pnpm-store`:
  «Что насчёт хранилища pnpm?», `expected_tool: query_index`, эталон — называет `~/Library/pnpm/store`
  (300 МБ), уровень «восстанавливаемое», следующая установка скачает пакеты заново.
- Правила с `~/` сравниваются с абсолютным путём `/Users/<имя>/…` (`RuleSet::root_components` берёт канонический
  путь корня индекса), поэтому на тестовом образе APFS (`/private/tmp/…/mount/Users/tester`) `cache.pnpm-store` не
  срабатывает, а писать в настоящий `/Users/…` тест не вправе. Сквозной тест `wire_agent_paths` проверяет
  `rule_explain` на `cargo.target` (шаблон на любой глубине), а хранилище pnpm — разворотом `~/` и гипотезой;
  тексты путей `cache.pnpm-store` — `rule_paths`, срабатывание на настоящем доме — живая приёмка.
- Граница гейтов: тесты доказывают пути правил, разворот `~/`, поля `rule_explain` и `prefix_found` на
  настоящих `Index`/`RuleSet`, отсутствие `/Users/` в промпте, стоимость шагов (`step_gap`) и сквозной
  путь через настоящую `Session::ask`. Что настоящая модель теперь находит pnpm, гейт не докажет: это
  ночной эвал (новая задача) и приёмка человеком — на вопрос про pnpm ответ называет
  `~/Library/pnpm/store` и 14 ГБ.

# Gameplan

### Phase 1: rule-paths — пути правил текстом

<!-- deps: [] -->

- [ ] `Rule::paths()` по Stack; `also_at` добавляет абсолютный путь текстом.
- [ ] Тест `crates/rules/tests/rule_paths.rs`: `cache.pnpm-store` → `["~/Library/pnpm/store"]`;
  `cargo.target` → шаблон с маркером; override `pnpm_store` добавляет абсолютный путь.

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-rules --test rule_paths
```
- Пути правил отдаются текстом, с override (exit 0).
```
cargo clippy -p domovoi-rules --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).

### Phase 2: agent-context — дом, `~/`, пути правил и prefix_found в агенте

<!-- deps: [rule-paths] — потребление: paths.rs гоняет настоящий Rule::paths через rule_explain. -->

- [ ] `grammar`: пример промпта по Stack; `response.rs` — в промпте нет `/Users/`.
- [ ] `agent`: `AgentContext::home`, сводка, разворот `~/`, `rule_explain`, `prefix_found`,
  `INSTRUCTIONS` по Knowledge.
- [ ] `app`: `SessionPaths::home`, `with_home`; `Session::ask` передаёт `home`.
- [ ] `eval`: `home` = `FIXTURE_HOME`, фикстура и задача по Knowledge.
- [ ] Тест `crates/agent/tests/paths.rs` (фикстура агента, `home` внутри дерева): `~/…` в `path_prefix`
  и `inspect_node` находит узел; гипотеза `~/…` остаётся с абсолютным путём; `rule_explain` отдаёт
  `paths` и `found`; пустой запрос с несуществующим префиксом — `prefix_found: false`, с существующим —
  `true`; в сводке `home`.

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-agent --test paths
```
- REAL-DEP: агент на настоящем Rule::paths (exit 0).
```
cargo test -p domovoi-agent --test step_gap
```
- Новые поля укладываются в паузу выгрузки (exit 0).
```
cargo test -p domovoi-grammar --test response
```
- В промпте нет имени пользователя (exit 0).
```
cargo test -p domovoi-app --test smoke_agent_downloader
```
- Настоящая Session::ask с home (exit 0).
```
cargo clippy -p domovoi-grammar -p domovoi-agent -p domovoi-app -p domovoi-eval --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).

### Phase 3: wire — вопрос про pnpm на главном пути

<!-- deps: [rule-paths, agent-context] — потребление ВСЕХ code-фаз: wire-терминал по WIRE-PHASE-RULE. -->

- [ ] `crates/app/tests/wire_agent_paths.rs`: APFS-образ с `Users/tester/Library/pnpm/store/…` и
  проектом; `Session` с `SessionPaths::with_home(<образ>/Users/tester)`; `Session::ask` + `FakeBackend`
  со сценарием: `rule_explain cache.pnpm-store` → `found` содержит хранилище; `query_index` с
  `path_prefix: ~/Library/pnpm/store` → узел найден; `path_prefix: /Users/igor/.pnpm` →
  `prefix_found: false`; гипотеза `~/Library/pnpm/store` → абсолютный путь в ответе.
- [ ] Прогрев гейта: `cargo xtask bundle --profile adhoc` выполняется задачей до гейта.
- [ ] Knowledge `docs/SPEC-DOMOVOI.md` [агент]: home, `~/`, `rule_explain` с путями, `prefix_found`.

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-app --test wire_agent_paths
```
- Главный путь вопроса про pnpm на настоящих модулях (exit 0).
```
cargo xtask bundle --profile adhoc
```
- Бандл для e2e собран (exit 0).
```
cargo test -p domovoi-app --test wire_main_path
```
- Вопрос из бандла на настоящей модели не сломан (exit 0).
```
cargo xtask check-sources
```
- Согласованность документов (exit 0).
```
cargo clippy --workspace --all-targets -- -D warnings
```
- Весь workspace чист (exit 0).
