<!--
  SPEC-TRAY-SESSION.md — дельта к docs/SPEC-DOMOVOI.md: пока открыт лист Корзины Домового, Session не покидает
  оболочку — тяжёлая пачка и полный скан по событиям ждут закрытия листа. 1 фаза.

  ГРАНИЦА РЕШЕНИЯ (веер от 19.10.2026, выбран вариант A по рекомендации): правило «Session уходит из оболочки
  только при закрытом листе» в ShellGate::lends_session; events_tick спрашивает его перед begin_apply и begin_scan.
  Отвергнуты: B — лист живёт без Session на сохранённой TrayModel (новое состояние «устаревший лист», Option во всех
  путях листа, лист поверх экрана 02, прогресс замера стоит); C — лист закрывается перед уходом Session (лист
  исчезает из-за фонового события, замер сбрасывается).
-->

# Baseline

- Предыдущая спека: `docs/SPEC-DOMOVOI.md`; дельты `SPEC-RESCAN` применены.
- Prior spec-hash: sha256:<placeholder — confirm>
- Проблема: при открытом листе Корзины Домового `events_tick` отдаёт `Session` в `ApplyJob` (`EventEffect::Heavy`) или в
  `begin_scan` (`RescanRequired`, `Err(Scan)`), а лист берёт её через `expect("the tray belongs to an open session")`
  в `measured_tray` на трёх путях: тик замера (`measure_tick → refresh_tray`), `refresh()`, `restyle()` (смена темы
  или контраста системой). Тяжёлая пачка FSEvents или потеря истории событий во время замера роняют приложение.
- Уже есть и не меняется: `ShellGate` и его ветки, `EventQueue` (при отложенной тяжёлой пачке `advance` сразу
  отдаёт `Heavy` и копит события), `TrayMeasurement`, действия листа за гейтом; открыть лист без `Session` нельзя.
- Версия: фазы повышения версии нет — `cargo xtask release` (`crates/xtask/src/release.rs`) сам инкрементирует
  патч; `.github/workflows/ci.yml` версию не меняет.
- Обоснование выбора — staged decision note (id после accept).

# Stack

- `crates/app/src/shell_gate.rs`: `ShellGate::lends_session()` — Session открыта и лист закрыт; ветка `Rescan`
  в `allows` идёт через него.
- `crates/app/src/shell_controller.rs`: `events_tick` — `Heavy` → `begin_apply` и `RescanRequired` / `Err(Scan)` →
  `begin_scan` только при `state.gate().lends_session()`; иначе тик ничего не делает.
- `docs/SPEC-DOMOVOI.md`: пункт [UI] про Корзину Домового и `ApplyJob` — пачка и скан ждут закрытия листа.
- Тесты: меняется `crates/app/tests/session_gate.rs`; новый `crates/app/tests/heavy_batch_waits.rs`.

# Conventions

- `Session` покидает оболочку (`ApplyJob`, `ScanJob`, `DuplicateJob`, вопрос) только при `lends_session()`:
  действия пользователя — через гейт `allows`, события тома — через ту же проверку в `events_tick`.
- `expect` в `measured_tray` остаётся: при открытом листе `Session` дома по правилу, а не по случаю.

# Knowledge

- Ожидание ничего не теряет. `EventQueue` держит тяжёлую пачку (`heavy`) и на каждом `advance` отвечает `Heavy`, а
  новые события копит в `queued`. `RescanRequired` повторяется на следующем тике сам: очередь не очищена.
  Нечитаемый листинг (`Err(Scan)`) прочитается заново. После закрытия листа ближайший тик (0,5 с) применяет пачку в
  `ApplyJob` или начинает скан.
- Пачка, прочитанная до «Переместить в Корзину», после переноса получает `StaleDelta`: `settle` считает это
  повтором, и события перечитываются. Так уже работает Корзина при ожидающем чтении.
- Пока лист открыт, тяжёлые пачки и полный скан ждут, а лёгкие дельты применяются как прежде. Окно под модальным
  листом недоступно, устаревших цифр никто не видит.
- Граница гейтов: тесты доказывают решение `lends_session` (лист закрывает уход `Session`, `Rescan` идёт через
  него) и то, что тяжёлая пачка, пролежавшая в `EventQueue` несколько тиков с новыми событиями, применяется
  настоящим `ApplyJob` без потерь. Не доказывает гейт: склейку в `events_tick` — `ShellController` в тестах не
  собирается. Приёмка человеком: открыть лист Корзины с большим выбором, во время «Уточняю размер» перенести в
  `~/.Trash` большое дерево (`MustScanSubDirs`) — лист живёт, после закрытия подзаголовок «Обновляю индекс…» и
  новые цифры.

# Gameplan

### Phase 1: tray-holds-session — пачка и скан ждут закрытия листа

<!-- deps: [] -->

- [ ] `ShellGate::lends_session`, `Rescan` через него, проверка в `events_tick` по Stack; пункт в `docs/SPEC-DOMOVOI.md`.
- [ ] `crates/app/tests/session_gate.rs`: `lends_session` — истина при открытой Session и закрытом листе, ложь без
  Session и при открытом листе; `Rescan` разрешён ровно тогда, когда `lends_session`.
- [ ] Тест `crates/app/tests/heavy_batch_waits.rs` на APFS-образе (`ImageVolume`, как в `smoke_volume_delta`):
  настоящая `Session`, `watch` и `EventQueue::new(2)`; новая папка с файлами → `advance` доходит до `Heavy`; затем
  ещё файлы в другом месте и несколько `advance` подряд — каждый отвечает `Heavy`, файлов первой папки в индексе
  ещё нет; затем `take_heavy` → `ApplyJob::start(...).finish()` → `settle` и `advance` до тишины — в индексе все
  файлы, и первой папки, и созданные во время ожидания.

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-app --test heavy_batch_waits
```
- Пачка, ждавшая закрытия листа, применяется без потерь (exit 0).
```
cargo test -p domovoi-app --test session_gate
```
- Лист Корзины держит Session дома (exit 0).
```
cargo xtask check-sources
```
- Согласованность документов (exit 0).
```
cargo clippy -p domovoi-app --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).
