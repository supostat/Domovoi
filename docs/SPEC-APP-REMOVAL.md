<!--
  SPEC-APP-REMOVAL.md — дельта к docs/SPEC-DOMOVOI.md: приложение удаляется вместе со всеми своими файлами, остатки
  удалённых приложений видны карточкой рекомендаций; файлы привязываются к приложению по точным признакам, у каждого
  видна причина привязки. 4 фазы.

  ГРАНИЦА РЕШЕНИЯ (веер от 28.10.2026, выбран вариант A по рекомендации): индекс хранит владельца узла вместе с
  доказательством (имя-bundle id, метаданные контейнера, группа из подписи, Team ID, launchd, только имя), установленные
  приложения берутся из бандлов самого индекса, правила остатков — условие OwnerMissing, удаление приложения собирает
  бандл и всё, чем он владеет по точным признакам, в «К удалению». На ревью спеки: квитанции установщика вынесены из
  дельты, совпадение только по имени показывается, но не отмечается. Отвергнуты: B — отдельный разбор следов по запросу
  (кэш индекса не меняется, но две правды о владельце, диаграмма, свойства и ассистент причины не видят); C — сначала
  только остатки (быстрее, но закрывает половину задачи).
-->

# Baseline

- Предыдущая спека: `docs/SPEC-DOMOVOI.md`; все дельты из `docs/` применены, последняя — `docs/SPEC-DELETE-PROGRESS.md`
  (влита в `main`, `c740e05`).
- Prior spec-hash: sha256:<placeholder — confirm>
- Проблема. Индекс уже приписывает владельца детям `Caches`, `Containers`, `Application Support`,
  `Saved Application State`, `HTTPStorages`, `WebKit`, `Preferences` по имени (`index.rs::bundle_id`, `owner_of`);
  установленность решают Spotlight и `NSWorkspace` (`installed_applications.rs`); есть `Installation::Missing` и
  `App::is_leftover`. Но ни правила, ни интерфейс, ни ассистент этим не пользуются: остаток удалённого приложения виден
  только как гипотеза модели (книга, экран 04). `Group Containers` и задания launchd без владельца, удалить приложение
  вместе с его файлами нельзя, причина привязки нигде не показана.
- Уже есть и не меняется: `Tray::add` (поглощение вложенных), `Tray::take_for_delete` с вердиктом `Denylist`
  (`RunningApplication`, `ParentNotWritable`, `ProtectedFlag`), движок `delete_run` и лист «К удалению» со счётом
  файлов, `ClaimLedger` / `RuleCard`, `Condition` и приоритет правил по позиции в `rule_table`, `~/` = `Users/*` в
  `path_pattern.rs`, `property_list_dictionary` в `index/src/ffi.rs`.
- Изучен подход Nektony App Cleaner 8 (статический разбор, код и база не переносятся): привязка по сравнению имён и
  закрытой базе путей, без квитанций и Team ID, причина не видна. Эта дельта сознательно строится на точных признаках.
- Вне дельты: квитанции установщика (`pkgutil`). Их пути почти всегда в `/Library` и принадлежат root, Домовой прав не
  повышает — пользователь получил бы в основном строки с отказом.
- Версия: фазы повышения версии нет — `cargo xtask release` (`crates/xtask/src/release.rs`) сам инкрементирует патч;
  `.github/workflows/ci.yml` версию не меняет.
- Typecheck-критерий проекта: `cargo clippy -p <crate> --all-targets -- -D warnings`.
- Обоснование выбора — staged decision note (id после accept).

# Stack

- Workspace `Cargo.toml`: `objc2-security = { version = "=0.3.2", default-features = false, features = ["std"] }`;
  `crates/index/Cargo.toml` — фичи `CSCommon`, `SecBase`, `SecCode`, `SecStaticCode`.
- `crates/index/src/installed_bundle.rs` (новый): `InstalledBundle { node, bundle_id, name, team_id: Option<String>,
  app_groups: Vec<String> }` для каждого узла `*.app`, не лежащего внутри другого `.app`: `Contents/Info.plist`
  (`CFBundleIdentifier`; имя — `CFBundleDisplayName`, затем `CFBundleName`, затем имя бандла без `.app`), подпись —
  `SecStaticCodeCreateWithPath` + `SecCodeCopySigningInformation(kSecCSSigningInformation)` → `kSecCodeInfoTeamIdentifier`
  и `com.apple.security.application-groups` из `kSecCodeInfoEntitlementsDict`; ad-hoc и неподписанные — `team_id: None`.
  `Index::bundles()`.
- `crates/index/src/owner_evidence.rs` (новый): `enum OwnerEvidence { BundleIdName, ContainerMetadata, AppGroup,
  TeamPrefix, LaunchdAssociation, NameOnly }`. `crates/index/src/ownership.rs` (новый): `Ownership { app, root,
  evidence }`; `Index::ownership(node)` — от ближайшего предка, где владелец назначен; `Index::owned_roots(app)`.
- `installed_applications.rs`: `resolve` ищет в бандлах индекса (тот же `is_same_or_nested`), затем Spotlight, затем
  `NSWorkspace`; `Missing` — только если приложения нет нигде.
- Признаки владельца (по модулю на признак, вызываются из `categorize`):
  - `Containers/<id>`: `MCMMetadataIdentifier` из `.com.apple.containermanagerd.metadata.plist` → `ContainerMetadata`
    (при расхождении с именем побеждают метаданные); без файла — `BundleIdName`.
  - `Group Containers/<группа>` (`AppDataNaming::GroupContainer`): группа в `app_groups` установленного бандла →
    `AppGroup`; префикс `TEAMID.` (10 заглавных букв и цифр) и ровно один установленный бандл этой команды →
    `TeamPrefix`; ни одного бандла команды и ни одного совпадения по группе → владелец — сама группа как `App` с
    `Missing` и `TeamPrefix`; несколько бандлов команды без совпадения по группе — владельца нет.
  - Задания launchd (`~/Library/LaunchAgents`, `/Library/LaunchAgents`, `/Library/LaunchDaemons`,
    `AppDataNaming::LaunchdPlist`): `AssociatedBundleIdentifiers` (строка или массив) или `Program` /
    `ProgramArguments[0]` внутри пути установленного бандла → `LaunchdAssociation`; иначе `Label` вида bundle id →
    `BundleIdName`.
  - `NameOnly`: `~/Library/Application Support/<имя>` и `~/Library/Caches/<имя>`, где имя не похоже на bundle id и без
    учёта регистра равно `name` ровно одного установленного бандла.
- `apply_delta.rs`: появление или исчезновение `.app` в сплайсе перечитывает бандлы сплайса и заново решает
  установленность всех `App`.
- `cache.rs`: `FORMAT_VERSION` — 3 (бандлы и доказательства в кэше).
- `crates/rules/src/rule.rs`: `Condition::OwnerMissing` — у узла есть `Index::ownership`, и его `App::is_leftover`.
- `crates/rules/src/rule_table.rs`, перед `cache.app.generic` и `caches.user`: `apps.leftover-caches` (Безопасно:
  `~/Library/Caches/*`, `~/Library/HTTPStorages/*`, `~/Library/WebKit/*`, `~/Library/Saved Application State/*`) и
  `apps.leftover-data` (Проверьте сами: `~/Library/Application Support/*`, `~/Library/Containers/*`,
  `~/Library/Group Containers/*`, `~/Library/Preferences/*`, `~/Library/LaunchAgents/*`), оба
  `.when(Condition::OwnerMissing)`.
- `crates/rules/src/app_removal.rs` (новый): `AppRemoval::of(index, bundle_node) -> AppRemoval { app, items:
  Vec<(NodeId, RemovalReason)>, name_only: Vec<NodeId> }`, `RemovalReason { Bundle, Owner(OwnerEvidence) }`; в `items`
  — бандл и владения по точным признакам, `NameOnly` — только в `name_only`. `crates/rules/src/tray.rs`:
  `TrayItem.reason: Option<RemovalReason>`, `TrayItem.application: Option<String>`; `Tray::add_application(index,
  &AppRemoval)` кладёт только `items` и запоминает у группы приложения неотмеченные пути `name_only`, которые в
  `take_for_delete` не попадают.
- `crates/agent/src/tool_runner.rs`: запись узла получает `owner` — `{ bundle_id, installation: installed | system |
  missing, evidence }` или `null`.
- `crates/app/src/strings.rs`: причины — `OWNER_BUNDLE_ID_NAME` «папка названа его bundle id»,
  `OWNER_CONTAINER_METADATA` «метаданные контейнера», `OWNER_APP_GROUP` «группа из подписи приложения»,
  `OWNER_TEAM_PREFIX` «Team ID разработчика в имени группы», `OWNER_LAUNCHD` «задание launchd указывает на приложение»,
  `OWNER_NAME_ONLY` «совпадает только имя — проверьте сами», `REMOVAL_BUNDLE` «само приложение»; `OWNER` «Владелец»,
  `OWNER_REMOVED` «приложение удалено», `REMOVE_WITH_FILES` «Удалить вместе с файлами», `NOT_MARKED_NAME_ONLY`
  «Не отмечено — совпадает только имя»; пояснения двух правил.
- `crates/app/src/index_facts.rs`, `property_labels.rs`: строка владельца в свойствах — «Владелец: Slack · метаданные
  контейнера», у остатка — «Владелец: com.fake.app · приложение удалено · метаданные контейнера».
- `card_model.rs`, `recommendations_model.rs`, `recommendation_card.rs`: `CardModel.path_reasons: Vec<Option<String>>`
  рядом с `paths`, причина — тусклой строкой под путём.
- `shell_action.rs`, `shell_actions.rs`: `RemoveApplication(usize)` и `RemoveClickedApplication`; пункт
  `REMOVE_WITH_FILES` в контекстном меню строки (`level_list.rs`) только у узла из `Index::bundles()`, кнопка в панели
  свойств; `Session::add_application_to_tray(bundle_node)` → `AppRemoval::of` → `Tray::add_application`, затем лист
  «К удалению».
- `tray_model.rs`, `tray_row.rs`, `tray_screen.rs`: группа приложения — строка «<имя> и его файлы» с размером, под ней до
  `APP_FILE_LINES = 6` строк «путь · причина» и «и ещё N», ниже — `NOT_MARKED_NAME_ONLY` и пути `name_only`;
  `tests/fixtures/shell_state.json` — группа приложения в листе и карточка остатков.
- `scenario_step.rs`, `end_to_end.rs`: шаг `remove_application { path }`; отчёт `recommendations` несёт правила карточек.
- `docs/DOMOVOI.md`: 4.2 (владелец с доказательством), 4.4 (два правила остатков), 4.8 (действие на приложении,
  причина в свойствах, карточке и листе, неотмеченное совпадение по имени); `design/domovoi-design-book.html`: экран 04 —
  карточка остатков правилом с причинами, экран 08 — группа приложения.

# Conventions

- Владельца решают точные признаки; совпадение только по имени — `NameOnly`, всегда «Проверьте сами». `NameOnly` и
  общая группа вендора никогда не делают остаток.
- `NameOnly` никогда не уходит в «К удалению» само: при удалении приложения оно видно в листе как неотмеченное с
  причиной.
- Приложения нет, только если его нет среди бандлов индекса, в Spotlight и в `NSWorkspace`; `com.apple.*` — System и
  никогда не остаток.
- У каждого файла, который предлагается удалить как часть приложения или как остаток, видна причина: в карточке, в
  свойствах, в листе «К удалению».
- Удаляет только движок удаления через лист «К удалению»: без root, без `chflags`; файлы root (`/Library/LaunchAgents`,
  `/Library/LaunchDaemons`) видны с отказом denylist и его причиной.
- Пути и содержимое plist — данные: ничего не исполняется, `Program` только сравнивается с путём бандла.
- Голос: без восклицаний; «похоже» — только у гипотез модели, правило говорит утверждением с причиной.

# Knowledge

- Почему не имя: у App Cleaner привязка держится на сравнении имён в четырёх формах и закрытой базе путей — отсюда
  ложные остатки и невидимая причина. Точные признаки macOS: имя контейнера и `MCMMetadataIdentifier`, entitlements
  `application-groups`, Team ID подписи, `AssociatedBundleIdentifiers` (macOS 13+) у заданий launchd.
- Почему `NameOnly` не отмечается: папка `Application Support/<имя>` может принадлежать другому приложению с тем же
  названием или общему компоненту вендора; одно имя не даёт права удалить её вместе с приложением.
- Group Containers бывают `TEAMID.group.x`, `TEAMID.x` и `group.x`. У Microsoft Office одна группа на Word, Excel и
  Outlook: без точного совпадения по entitlements владельца нет, иначе удаление одного приложения унесло бы данные
  соседей.
- Ad-hoc и неподписанные бандлы не дают Team ID — признак просто отсутствует, это не ошибка.
- Стоимость: `Info.plist` и подпись читаются один раз на бандл при сборке индекса и лежат в кэше; дельта перечитывает
  только бандлы своего сплайса. Совпадение дельты и пересборки доказывает `delta_equivalence`.
- Правила остатков стоят перед `cache.app.generic` и `caches.user`, поэтому кэш удалённого приложения уходит в
  карточку остатков. Более ранние правила (кэши вендоров) сохраняют свой claim; `cache.app.generic` глубже контейнера,
  поэтому у остатка-контейнера его `Data/Library/Caches` остаётся в карточке кэшей, а остальное — в карточке остатков.
- Квитанции установщика вынесены: `pkgutil --file-info <путь>` назвал бы пакет, `--files` — его пути, но это почти
  всегда файлы root в `/Library`. Если понадобятся, их место — отдельная дельта с поддельным `pkgutil` в тестах (в
  `/private/var/db/receipts` тест писать не может).
- Признакам нужны настоящие файлы: `index/tests/app_evidence.rs` сканирует `TempTree`; бандл — копия `/usr/bin/true` в
  `Contents/MacOS`, `Info.plist` и подпись `/usr/bin/codesign --force --sign - --entitlements <plist>` с
  `application-groups`. Синтетические отчёты `apps.rs` остаются для имён.
- Правила `~/` совпадают с `Users/*`, поэтому фикстурам нужен `Users/<имя>/Library`: вне дома правила не срабатывают.
- Предел проверки: настоящие приложения пользователя (из App Store — root, Office с общей группой) в тесте не
  воспроизвести — приёмка человеком на границе wire (бандл: `cargo xtask bundle --profile adhoc`).

# Gameplan

### Phase 1: owners — установленные бандлы и владелец с доказательством в индексе

- [ ] `objc2-security`, `installed_bundle.rs`, `Index::bundles()`, `resolve` через бандлы индекса.
- [ ] `owner_evidence.rs`, `ownership.rs`, признаки Containers, Group Containers, launchd, `NameOnly`.
- [ ] `apply_delta.rs`: бандлы сплайса и установленность; `FORMAT_VERSION` 3.
- [ ] `index/tests/app_evidence.rs` (новый): каждый признак на настоящих файлах; контейнер с чужим
  `MCMMetadataIdentifier`; группа по entitlements, по Team ID, общая группа без владельца, группа удалённой команды —
  остаток; launchd по `AssociatedBundleIdentifiers` и по `Program`; `NameOnly` только у установленного; бандл в
  `~/Downloads` — установлен. `index/tests/apps.rs`: прежние случаи с доказательством `BundleIdName`.

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-index --test app_evidence
```
- Каждый точный признак и NameOnly на настоящих файлах (exit 0).
```
cargo test -p domovoi-index --test apps
```
- Владельцы по имени и остатки (exit 0).
```
cargo test -p domovoi-index --test delta_equivalence
```
- Дельта с появлением и исчезновением бандла равна пересборке (exit 0).
```
cargo test -p domovoi-index --test cache_roundtrip
```
- Бандлы и доказательства переживают кэш (exit 0).
```
cargo clippy -p domovoi-index --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).

### Phase 2: leftovers — правила остатков и владелец у ассистента

<!-- deps: [owners] — потребление: Index::ownership, InstalledApplications::resolve и App::is_leftover из фазы 1. -->

- [ ] `Condition::OwnerMissing`, `apps.leftover-caches`, `apps.leftover-data` в `rule_table`.
- [ ] `docs/DOMOVOI.md` 4.4: два правила в таблице (`doc_parity`).
- [ ] `tool_runner.rs`: `owner` в записи узла.
- [ ] `rules/tests/app_leftovers.rs` (новый) на настоящем скане `TempTree` с `Users/<имя>/Library`: остаток по
  метаданным контейнера и по группе удалённой команды — в карточках правил с их уровнями; `NameOnly`, общая группа
  вендора и `com.apple.*` — не остатки; кэш вендора остаётся у своего правила. `agent/tests/owner_facts.rs` (новый):
  `inspect_node` на остатке и на файле установленного приложения.

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-rules --test app_leftovers
```
- Остатки на настоящем индексе фазы 1 в карточках правил (exit 0).
```
cargo test -p domovoi-rules --test doc_parity
```
- Правила остатков совпадают с документом (exit 0).
```
cargo test -p domovoi-agent --test owner_facts
```
- Ассистент видит владельца и доказательство (exit 0).
```
cargo clippy -p domovoi-rules -p domovoi-agent --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).

### Phase 3: app-removal — удаление приложения с файлами и причины на экранах

<!-- deps: [owners, leftovers] — потребление: Index::bundles, owned_roots и ownership (фаза 1), карточки apps.leftover-* (фаза 2). -->

- [ ] `app_removal.rs`, `TrayItem.reason` / `application`, неотмеченные `name_only` у группы, `Tray::add_application`.
- [ ] Строки, строка владельца в свойствах, `path_reasons` в карточке, пункт меню и кнопка «Удалить вместе с файлами»,
  `Session::add_application_to_tray`, группа приложения в листе с неотмеченными путями; фикстура `shell_state.json`.
- [ ] `app/tests/app_removal.rs` (новый): настоящий индекс → `add_application_to_tray` → бандл и все владения по
  точным признакам с причинами; `NameOnly` виден как неотмеченный и не попадает в `take_for_delete`; файл root — отказ
  denylist с причиной; карточка остатков несёт `path_reasons`; строка владельца в свойствах. `app/tests/voice.rs`:
  новые строки.

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-app --test app_removal
```
- Приложение и его файлы с причинами доходят до листа «К удалению», совпадение по имени не отмечено (exit 0).
```
cargo test -p domovoi-app --test voice
```
- Голос причин и действия (exit 0).
```
cargo test -p domovoi-app --test truncation
```
- Причины и группа приложения не обрезают текст (exit 0).
```
cargo test -p domovoi-app --test snapshot_care
```
- Экраны 04 и 08 с причинами держат размеры в обеих темах (exit 0).
```
cargo clippy -p domovoi-rules -p domovoi-app --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).

### Phase 4: wire — удаление приложения и остатки на главном пути бандла

<!-- deps: [owners, leftovers, app-removal] — потребление: настоящий индекс, правила остатков и Session::add_application_to_tray в шагах сценария. -->

- [ ] `scenario_step.rs`, `end_to_end.rs`: `remove_application { path }`; отчёт — удалённые, неотмеченные `name_only`,
  `paths_left`; правила карточек в отчёте `recommendations`.
- [ ] `app/tests/wire_main_path.rs`: на томе — установленное `Applications/Tiny.app` (ad-hoc, группа в entitlements) с
  контейнером, группой, заданием launchd и папкой по имени; остатки удалённого `com.domovoi.gone` (контейнер с
  метаданными, кэш, группа его команды). Проверки: карточки `apps.leftover-caches` и `apps.leftover-data` с путями
  удалённого; `remove_application` удаляет бандл и все владения по точным признакам, `paths_left` — 0; папка по имени
  осталась на диске и названа в отчёте неотмеченной; общие каталоги на месте.
- [ ] `docs/DOMOVOI.md` 4.2 и 4.8; книга: экраны 04 и 08.

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-app --test wire_main_path
```
- Главный путь бандла: остатки в карточках, удаление приложения с файлами, совпадение по имени на месте (exit 0).
```
cargo xtask check-comments
```
- Комментарии без ссылок на книгу, фазы и спеки (exit 0).
```
cargo xtask check-sources
```
- Удаление только в модуле удаления, книга и документ без устаревших формулировок (exit 0).
```
cargo xtask check-tokens
```
- Цвета только в теме (exit 0).
```
cargo clippy -p domovoi-app --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).
