<!--
  FIX-SIMULATOR-DEVICE-SET.md — дельта к docs/SPEC-DOMOVOI.md: служебный файл
  ~/Library/Developer/CoreSimulator/Devices/device_set.plist попадает в карточку «Симуляторы Xcode», и отметка
  карточки отправляет его в «К удалению». 1 фаза.

  ДИАГНОЗ (/mneme:fix от 09.11.2026, гипотеза 1 из трёх): шаблон Devices/* обоих правил симуляторов берёт любого
  ребёнка папки, а подбор правила не смотрит на вид узла; правка — признак правила «только папки».
-->

# Baseline

- Предыдущая спека: `docs/SPEC-DOMOVOI.md`; все дельты из `docs/` применены, последняя —
  `docs/FIX-BROKEN-CLONE-JOURNAL.md` (`f840e4b` в `main`).
- Prior spec-hash: sha256:<placeholder — confirm>
- Проблема. В карточке «Симуляторы Xcode» рядом с папками устройств стоит файл `Devices/device_set.plist`;
  отметка карточки отправляет его в «К удалению».
- Уже есть и не меняется: шаблоны и уровни правил симуляторов; условие «runtime устройства не установлен»; порядок
  правил; карточки рекомендаций.
- Не входит: `xcode.simulator-runtimes` — в `/Library/Developer/CoreSimulator/Images` кроме `images.plist` лежат
  служебные папки `Inbox` и `mnt`, «только папки» его не исправит, и правило информационное (в «К удалению» не
  попадает); остальные правила с `/*` законно забирают файлы.
- Версия: фазы повышения версии нет — `cargo xtask release` сам инкрементирует патч.

# Stack

- `crates/rules/src/rule.rs`: поле `folders_only: bool` (по умолчанию `false`) и построитель `Rule::folders_only()`.
- `crates/rules/src/rule_table.rs`: `.folders_only()` у `xcode.simulators-unavailable` и `xcode.simulators`.
- `crates/rules/src/rule_set.rs`: `matching_rule` — правило с `folders_only` подходит только узлу
  `NodeKind::Directory`; вид узла проверяется до `condition_holds`, поэтому для файла `plutil` не запускается.
- `crates/rules/tests/simulator_device_set.rs` (написан диагнозом): регрессия.
- `docs/DOMOVOI.md`, абзац об уровнях правил после таблицы (раздел правил): симулятор — папка устройства в
  `CoreSimulator/Devices`; служебный `device_set.plist` рядом с ними в карточки не попадает.

# Conventions

- Признак — свойство правила в таблице, а не условие и не фильтр карточки: один подбор `matching_rule` решает для
  `claims`, `claims_from` (дельты) и `claim_of`.
- Комментарии не ссылаются на спеку и фазы (`cargo xtask check-comments`).
- Typecheck-критерий — `cargo clippy -p domovoi-rules -p domovoi-app --all-targets -- -D warnings`; формат —
  `cargo fmt --all -- --check`.

# Knowledge

- **Красный прогон.** `cargo test -p domovoi-rules --test simulator_device_set` — 0 passed, 1 failed. С подделкой
  «недоступен только OLD-DEVICE» (как у настоящей проверки: для файла `device.plist` не читается) файл забирало
  `xcode.simulators`:
    panicked at crates/rules/tests/simulator_device_set.rs:45:5: only device folders are simulators
    left: {.../Devices/CURRENT-DEVICE: xcode.simulators, .../Devices/OLD-DEVICE: xcode.simulators-unavailable, .../Devices/device_set.plist: xcode.simulators}
  С итоговой подделкой «установлен только CURRENT-DEVICE» его забирает `xcode.simulators-unavailable`:
    left: {..., .../Devices/device_set.plist: xcode.simulators-unavailable}
- **Причина.** Оба правила — шаблон `~/Library/Developer/CoreSimulator/Devices/*` (`rule_table.rs:61-74`); подбор
  `matching_rule` проверяет путь и условие, но не вид узла (`rule_set.rs:229-254`); для файла условие недоступности
  ложно — `plutil` не читает `device_set.plist/device.plist` (`simulator_runtimes.rs:52-55`), и файл падает в
  `xcode.simulators`.
- **Настоящая папка.** В `~/Library/Developer/CoreSimulator/Devices` этой машины все каталоги — UUID устройств с
  `device.plist`, единственная не-папка — `device_set.plist`.
- **Отвергнуто.** Гипотеза 2 — условие «папка с `device.plist`»: второе условие только для симуляторов, недоудалённая
  папка без `device.plist` пропала бы из карточек. Гипотеза 3 — фильтр на карточке: правило продолжило бы забирать
  файл (уровень, итоги, «Удалить вместе с файлами»).
- **Тест.** Синтетический индекс Data-тома: `Devices` с `OLD-DEVICE` и `CURRENT-DEVICE` (по `device.plist`) и файлом
  `device_set.plist`; подделка рантаймов говорит «недоступен» для всего, кроме `CURRENT-DEVICE`, поэтому тест ловит
  отсутствие признака на любом из двух правил.
- **Проверка мутацией.** Одна мутация на прогон: без `.folders_only()` у `xcode.simulators-unavailable` — файл
  забирает оно; без него у `xcode.simulators` при признаке на первом — файл забирает `xcode.simulators`; без проверки
  вида узла в `matching_rule` — краснеет так же.

# Gameplan

<!-- 1 фаза: признак правила «только папки» на правилах устройств CoreSimulator. -->

### Phase 1: simulator-device-set — в карточках симуляторов только папки устройств

- [ ] `Rule::folders_only` (поле и построитель), проверка вида узла в `RuleSet::matching_rule` до условия,
  `.folders_only()` у `xcode.simulators-unavailable` и `xcode.simulators`.
- [ ] Предложение о папках устройств в абзаце об уровнях правил `docs/DOMOVOI.md`.
- [ ] Оставить `crates/rules/tests/simulator_device_set.rs` регрессионным тестом; мутации по Knowledge, по одной на
  прогон.

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-rules --test simulator_device_set
```
- Рядом с папками устройств `device_set.plist` не забирает ни одно правило симуляторов (exit 0).
```
cargo test -p domovoi-rules --test rules_table
```
- Каждое правило по-прежнему забирает свою фикстуру, включая оба правила симуляторов (exit 0).
```
cargo test -p domovoi-rules --test doc_parity
```
- Таблица правил и документ совпадают по id и уровням (exit 0).
```
cargo clippy -p domovoi-rules -p domovoi-app --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).
```
cargo fmt --all -- --check
```
- Формат совпадает с rustfmt (exit 0).
