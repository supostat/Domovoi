<!--
  SPEC-DOMOVOI.md — дельта-спека полного продукта DOMOVOI (greenfield). 19 фаз.
  Прогоняется через /mneme:dev в репозитории DOMOVOI.

  ИСТОЧНИКИ ИСТИНЫ: docs/DOMOVOI.md (продукт, архитектура, 17 решений) и
  design/domovoi-design-book.html (разделы I–XI). При расхождении в визуальных вопросах
  права книга, в остальных — документ. Спека не переоткрывает решения, а уточняет четыре
  формулировки по итогам разведки (Baseline, фаза workspace) и приводит оба источника в
  соответствие с принятыми решениями (фаза workspace).

  ГРАНИЦА РЕШЕНИЯ (веер от 16.10.2026, выбрано пользователем по рекомендации):
  визуализация — оба режима с переключателем, sunburst по умолчанию (отвергнуты: только
  sunburst — теряются пропорции и подписи; только treemap — теряются макеты книги и
  радиальный зум); дубликаты — в Корзину по умолчанию + явная замена клоном по группе
  (отвергнуты: только Корзина — нет сценария двух путей; только клон — предусловия
  закрывают половину групп); каталог — только семейство ChatML: Qwen3.5-4B / T-lite-it-2.1 /
  Qwen3-14B как primary, альтернативы того же семейства (отвергнуты: один вендор Qwen —
  русский не дообучен; лучшие по бенчмаркам с Gemma 4 и Phi-4 — второе и третье семейство
  шаблонов, у Gemma 4 неподтверждённый русский); workspace — крейт на подсистему (отвергнуты:
  два крейта — монолит; три слоя — FFI смешан); тесты сканера — два яруса (отвергнуты: только
  temp — нет томов; только образы — медленно и флаки); тесты инференса — слоистые
  (отвергнуты: только tiny-GGUF — косвенно и долго; только мок — биндинги не проверены).

  обоснование выбора — staged decision note (id после accept).
  Формат — дельта. Baseline с хэшами источников истины.

  ФОРМА (контракт конвертера from-spec): буллеты в Knowledge и задачи `- [ ]` фаз — одна строка
  с продолжениями, каждое продолжение начинается с отступа; конвертер склеивает их в одну.
  Внутри фазы допустимы только задачи `- [ ]`, блоки done-when и HTML-комментарии. Заголовок
  фазы — `### Phase N: <id> — <заголовок>`, разделитель строго « — » (пробел, длинное тире
  U+2014, пробел); `<id>` — латиница, цифры, дефис, не длиннее 64 символов; дефис или короткое
  тире вместо U+2014 молча портит id. Блок `**Done when (EXECUTABLE):**` — маркер побайтно
  точный, сразу за ним ``` без пустой строки, между критериями пустых строк нет, команда — ровно
  одна строка внутри ```…```, после закрывающего ``` обязательна строка прозы (буллет `- …`
  допустим, можно переносить с отступом). Строка `**Done when (AGENT-JUDGED):** …` — вся проза
  на ОДНОЙ строке маркера, переносить нельзя. Комментарии `<!-- deps: … -->` под заголовками —
  только документация потребления: конвертер их не читает и строит deps по порядку фаз; при
  перестановке фаз править вручную. Knowledge копируется в КАЖДЫЙ фазовый файл, поэтому несёт
  только факты, нужные двум и более фазам; факты одной фазы живут в её задачах.
-->

# Baseline

- **Prior spec:** нет — greenfield. Источники истины на коммите `3073094` (`main`):
  `docs/DOMOVOI.md` и `design/domovoi-design-book.html`.
- **Prior spec-hash:** sha256:19c6e33bffbc7caa52b422a8c7fefe71b69d3576b47fb5ce8adfc4f4f0bfc0d9 (docs/DOMOVOI.md) ·
  sha256:84c44fcaeb0aed20616812997e254077049be3d03906e42b32817fa104ca8147 (design/domovoi-design-book.html) —
  подтвердить перед прогоном.
- **Что уже есть (не трогается):** `CLAUDE.md` с контрактом критериев, `.gitignore`. Кода нет. Оба документа
  правятся фазой workspace ровно в местах, названных ниже.
- **Чего нет:** кода, CI, релизной автоматизации. VERSION-BUMP-RULE: автоматизации бампа не найдено, но и версии
  ещё нет — отдельная bump-фаза не порождается; версия `0.1.0` задаётся в `[workspace.package]` (фаза workspace),
  релизная фаза создаёт `cargo xtask release` с ИНКРЕМЕНТОМ патча (никогда литерал «X → Y») и штампует версии
  обоих Info.plist и appcast из этого единственного источника.
- **Уточнения формулировок принятых решений (вносятся в docs/DOMOVOI.md фазой workspace):** решение 2 — ключи
  идентичности `(ATTR_CMNEXT_REALDEVID, ATTR_CMN_FILEID)` вместо `(dev, inode)`; решение 7 —
  `GGML_METAL_EMBED_LIBRARY` встраивает исходник шейдеров и компилирует их при каждой инициализации, поэтому в
  бандл сервиса кладутся предкомпилированные `default.metallib` и `ggml-tensor.metallib`; решение 10 — конвертер
  JSON-схема→GBNF пишется на Rust (публичный `llama.h` принимает только готовую GBNF, конвертер llama.cpp живёт в
  C++ `common/`); раздел 4.3 — растровый атлас глифов вместо SDF (пункт 6.7 закрыт отрицательно); раздел 4.4 —
  уровни трёх стартовых правил по разведке. Новое решение: только Apple Silicon (arm64). Книга приводится к тем же
  решениям (плашки «Решение открыто», строки IX и X, таблица RULES).
- **Выбор веера:** 1·3, 2·3, 3·2, 4·2, 5·3, 6·3 и допущения 1–15.
- **Вне объёма гейтов (ручная приёмка или ночной self-hosted прогон):** зонды разведки в репозиторий не
  переносятся — их факты закреплены платформенными тестами `apfs-sys`; App Translocation при первом запуске из
  DMG — ручная проверка при приёмке distribution; `xcode-27` preview не прогоняется; нотаризация, онбординг FDA на
  VM macOS 15 и окно в живом WindowServer — приёмка человеком после зелёного wire; точные пиксельные эталоны,
  латентность отмены на Metal, TPOT в сервисе, память по классам, эвал каталога и бенчмарки — ночной self-hosted.

# Stack

- **Язык и тулчейн:** Rust 1.94 (`rust-toolchain.toml`), target только `aarch64-apple-darwin`; Xcode 26.1 SDK с
  Metal Toolchain (`xcodebuild -downloadComponent metalToolchain`, в CI — отдельный кэшируемый шаг); cmake ≥ 3.17;
  Python ≥ 3.10 только для скриптов `research/` (gguf-py требует ≥ 3.10 и numpy ≥ 2.2.6; jinja2 для golden-промптов);
  в CI Python не нужен — артефакты закоммичены.
- **Workspace `crates/`:** `apfs-sys`, `testkit`, `scanner`, `index`, `rules`, `dups`, `theme`, `render`, `grammar`,
  `llama-sys`, `xpc-sys`, `inference-proto`, `inference` (bin `domovoi-inference`), `downloader`, `agent`, `eval`,
  `app` (bin `domovoi`), `xtask`. Пакеты с префиксом `domovoi-` (кроме `xtask`).
- **Пины (workspace.dependencies):** objc2 =0.6.5; objc2-foundation / -app-kit / -metal / -metal-kit /
  -core-foundation / -core-graphics / -core-text / -quartz-core / -core-services =0.3.2 (default-features = false,
  фичи поимённо); block2 =0.6.2; dispatch2 =0.3.1; libc 0.2.190; blake3 1.8.7 (rayon); rayon 1.12.0;
  rkyv 0.8.18; serde 1.0.229 (derive); serde_json 1.0.154; bindgen 0.73.2; cmake 0.1.58; bytemuck 1.25.2; scraper 0.27.0; cssparser 0.38.0; quote +
  prettyplease; tiny_http (dev).
- **Внешнее:** llama.cpp как git submodule `vendor/llama.cpp` на коммите `ceefd16daa2093730bf3340315fa2b84b286ddbc`
  (16.10.2026); Sparkle 2.10.0 (`Sparkle-2.10.0.tar.xz`, 16 319 840 байт, SHA-256
  `c2bf58aa8387266ac179357b1415d6f2635f044da8be41042af32425dae6da0c`); шрифты Onest 2.001 и JetBrains Mono (OFL 1.1)
  в `assets/fonts/`; тестовые модели `assets/test/tiny-random.gguf` (115 КБ, закоммичена) и `stories260K.gguf`
  (1 185 376 байт, SHA-256 `270cba1b…`; MIT по происхождению karpathy/tinyllamas — в репозиториях ggml-org лицензия
  не проставлена, источник фиксируется в манифесте) через `xtask fetch-test-models`.
- **CI:** GitHub Actions, матрица `macos-15` (минимальная ОС, метка явно) и `macos-26` arm64; sccache + rust-cache;
  шаг загрузки Metal Toolchain; job-канарейка hdiutil; nightly `self-hosted, macOS, ARM64` (allow-failure):
  `cargo test --workspace -- --ignored` с `DOMOVOI_GOLDEN_EXACT=1`, `cargo bench` (full_scan, frame),
  `cargo xtask eval`.
- **Идентификаторы:** приложение `com.domovoi.app`, сервис `com.domovoi.inference` (домен команды подставляется
  в одном месте `xtask` перед релизом).

# Conventions

- Весь код на Rust; Swift и SwiftUI отсутствуют. `unsafe` разрешён в `*-sys`, `render`, `inference`, `app`,
  `downloader` (делегат `NSURLSession` через `define_class!`) и `testkit`; в `scanner`, `index` и `theme` — только
  в модуле `ffi.rs` (`#![deny(unsafe_code)]` на крейте, `#[allow(unsafe_code)]` на модуле: FSEvents и MDQuery;
  в `theme` — статики `NSAppearanceName*` и подписка на `NSWorkspaceAccessibilityDisplayOptionsDidChangeNotification`,
  которые objc2 0.3.2 помечает unsafe); `rules`, `dups`, `grammar`, `inference-proto`, `agent`, `eval` —
  `#![forbid(unsafe_code)]` и ходят через безопасные обёртки `apfs-sys`.
- Комментарии в коде не ссылаются на книгу, фазы и спеку (`cargo xtask check-comments` — гейт).
- Цвета только из `domovoi-theme` (сгенерировано из раздела XI книги); hex-литералы вне крейта запрещены
  (`cargo xtask check-tokens`).
- Интерфейс только на русском; строки — модуль `strings` крейта `app`, голос по разделу VIII; размеры, счётчики и
  давность форматирует только `domovoi-theme` (десятичные единицы, запятая, NBSP, склонения).
- Ошибки доступа различаются по errno (EPERM / EACCES / ENOENT), не по `io::ErrorKind` (std схлопывает EPERM и
  EACCES в PermissionDenied).
- Идентичность файлов: инод `(REALDEVID, FILEID)`, поток данных `(REALDEVID, CLONEID)`, запись `LINKID`;
  `st_dev` никогда не ключ; узлы сопоставляются ключами, не строками путей.
- Удаление только `trashItemAtURL:resultingItemURL:error:`; `unlink` / `removeItem` вне тестов запрещены
  (grep-гейт wire).
- done-when фаз: одна argv-команда без кавычек и shell-операторов; команда существует к моменту гейта и
  укладывается в 300 с (лимит гейт-раннера mneme, не переопределяется; по таймауту SIGKILL без чтения вывода) —
  сборочные гейты инкрементальны, фаза прогревает тот же профиль задачей до гейта.
- Таймауты ожидания в тестах (простой сервиса, FSEvents, retry hdiutil, сторож) задаются через окружение и в
  CI не превышают 10 с; образы hdiutil ≤ 64 МБ; один тест-файл — один образ.
- Тесты = спецификации в `crates/<name>/tests/`; имя файла теста названо в задачах фазы; фикстуры генерирует
  `domovoi-testkit`, бинарных образов в git нет.
- Подпись: dev-сборки — тем же Developer ID, что релиз (TCC запоминает идентичность), с hardened runtime;
  `adhoc`-профиль (`-`, без `--options runtime` — Library Validation не даст загрузить ad-hoc Sparkle) только для
  CI без сертификата, фича `adhoc` отключает peer requirement XPC.
- Коммиты без Co-Authored-By; один коммит на фазу.

# Knowledge

- [генератор] TYPECHECK-CRITERION-RULE: роль typecheck-скрипта в Rust играет
  `cargo clippy -p <крейт> --all-targets -- -D warnings`; критерий добавлен каждому крейту, рождающемуся в фазе.
- [генератор] WIRE-PHASE-RULE: 18 code-фаз → терминальная фаза `wire` с e2e главного пути на настоящих модулях
  через headless-режим `domovoi --e2e` из собранного бандла; мок только на границе сети (локальный `tiny_http`) и
  GPU (tiny-GGUF на CPU); предел UI: окно в живом WindowServer, онбординг FDA и нотаризация — приёмка человеком,
  без маскировки под agent-judged.
- [генератор] REAL-DEP-SMOKE-RULE: ребро потребления кода несёт тест `smoke_<upstream>` на настоящем модуле;
  рёбра xpc → inference и llama-sys → inference покрыты тестами `xpc_bundle` и `tiny_gguf` (шире smoke);
  непрямые рёбра index / rules / dups → apfs-sys покрыты тестами фаз на temp-дереве и образе без моков; app →
  theme — `theme_pixels`; событие «Модель»: тип `ModelEvent` живёт в index, downloader его эмитит (ребро
  downloader → index, тест `catalog`); downloader потребляет `MemoryBudget` / `ClassFootprint` из inference-proto
  (тест `catalog`), `trash_item` из rules (тесты `download` и `storage` на образе) и xattr / `pwrite_at` из
  apfs-sys; рёбра `theme → apfs-sys`, `dups → grammar`, `grammar → llama-sys`,
  `llama-sys → xpc`, `inference → downloader`, `downloader → agent`, `agent → render` — order-only.
- [платформа] macOS 15+, только arm64 (Tahoe — последний релиз для Intel; классы каталога «по общей памяти» и
  Metal-бэкенд предполагают unified memory). Замеры разведки сделаны на macOS 26.1 — платформенные тесты
  `apfs-sys` повторяют их на каждом хосте CI (`macos-15` явно), чтобы различия версий всплывали гейтом.
- [идентичность] У System и Data одинаковые `st_dev`/`ATTR_CMN_DEVID`; настоящий dev — `ATTR_CMNEXT_REALDEVID`;
  смена тома в дереве видна только по `ATTR_DIR_MOUNTSTATUS`. Канонический элемент группы потока — минимальный
  `(REALDEVID, FILEID)` среди членов группы (запись с `FILEID == CLONEID` может отсутствовать), с предпочтением
  пути вне `node_modules`/`.pnpm` (там клоны массовы); остальные члены — виртуальный 0 и бейдж; сумма по дереву
  не зависит от порядка и числа потоков обхода.
- [идентичность] `CLONE_REFCNT` считает уникальные иноды потока, не записи каталога; у decmpfs-сжатых файлов
  (`UF_COMPRESSED`) `CLONE_REFCNT == 0` и `PRIVATESIZE == 0` даже без клонов и даже при живом клоне (кроме мелких
  сжатых с inline-данными: alloc 4096, priv 4096, refcnt 1); `EF_MAY_SHARE_BLOCKS` — липкий признак «когда-либо
  клонировался», факт разделения — `EF_SHARES_ALL_BLOCKS` или `CLONE_REFCNT > 1`; клоновые атрибуты описывают
  только data-форк. Единственный честный инвариант: Σ ALLOCSIZE по уникальным инодам == `du -sk`.
- [идентичность] `reclaimable(node)`: `UF_COMPRESSED` → 0, если `EF_SHARES_ALL_BLOCKS` или группа потока содержит
  больше одного инода (удержано клоном), иначе `ALLOCSIZE`; не `UF_COMPRESSED` → `PRIVATESIZE`, перечитываемый
  `getattrlist` в момент показа (меняется снапшотами без FSEvents, в кэш и журнал не пишется); удержание
  различается: клон (`CLONE_REFCNT > 1` или `EF_SHARES_ALL_BLOCKS`) и снапшот (при Time Machine `PRIVATESIZE ≈ 0`
  почти у всего старше часа) — UI показывает обе цифры.
- [генератор] Гейт-раннер даёт каждой done-when команде 300 с (gate-runner.ts, не переопределяется); сборочные
  команды гейтятся только после прогрева тем же профилем в задаче фазы; таймауты тестов — через окружение, ≤ 10 с
  в CI.
- [пути] Канонический путь индекса, кэша, правил и FSEvents — пользовательская firmlink-форма (`/Users/…`,
  `/Applications/…`) по карте `/usr/share/firmlinks`; FSEvents и MDQuery отдают Data-том именно в ней; физическое
  расположение (`/System/Volumes/Data/…`) — только `ATTR_CMNEXT_NOFIRMLINKPATH` для отображения.
- [пути] Первые строки `main()` сканера и сервиса: `setiopolicy_np(IOPOL_TYPE_VFS_MATERIALIZE_DATALESS_FILES,
  IOPOL_SCOPE_PROCESS, …_OFF)` и `IOPOL_TYPE_VFS_TRIGGER_RESOLVE …_OFF` — без первой обход `~` виснет на iCloud Drive;
  `SF_DATALESS` и EDEADLK → узел «в облаке» вне суммы, такие файлы не хэшируются и не клонируются.
- [давность] `kMDItemLastUsedDate` файлов живёт в xattr `com.apple.lastuseddate#PS` (16 байт timespec LE; читать
  `getxattr` полным именем с `XATTR_NOFOLLOW`); каталоги и бандлы — только в индексе Spotlight (один `MDQuery`
  по завершении скана); копии, клоны и `cp -p` наследуют xattr даты — поле `last_opened` несёт источник
  (`Xattr | SpotlightIndex | Inherited`); mtime никогда не подставляется.
- [FDA] Публичного API проверки Full Disk Access нет; проба — реальный `opendir` (не `access`, не TCC.db): отказ
  TCC = EPERM при успешном `stat`, юникс-права = EACCES, Data Vault = EPERM уже на `stat` (FDA не поможет); первая
  реальная попытка добавляет приложение в список FDA выключенным; вердикт кэшируется ядром (по наблюдению — на
  ответственное приложение, гранулярность и TTL не установлены) — выданное право надёжно видно только после
  перезапуска; проба — при запуске и перед каждым сканом, не в цикле.
- [FDA] Desktop/Documents/Downloads и облачные папки защищены отдельными промптами при первом чтении — сканируются
  последними и после закрытия онбординга; при FDA промптов нет. URL панели:
  `x-apple.systempreferences:com.apple.settings.PrivacySecurity.extension?Privacy_AllFiles` (macOS 13+, ESClient.h);
  Apple анонсировала новые контроли FDA — тексты онбординга хранятся данными.
- [том] На APFS `statfs` и `ATTR_VOL_SIZE/SPACEFREE/SPACEAVAIL` — величины контейнера; занятое томом — только
  `ATTR_VOL_SPACEUSED` (как `df -k` Used и Disk Utility). «Свободно» = контейнерное `SPACEAVAIL`; purgeable —
  аннотация `Important − Available` из `NSURLVolumeAvailableCapacityForImportantUsageKey` (значение из CacheDelete по
  XPC: вне main thread, не кэшировать надолго); источник каждой цифры показывается в тултипе.
- [том] Снапшоты перечисляет `fs_snapshot_list(dirfd корня тома)` без root и entitlement; размер снапшота публично
  недоступен (`ALLOCSIZE` там — не байты); классы по префиксу `com.apple.TimeMachine.*.local`,
  `com.apple.os.update-*` (не purgeable), прочие — сторонние; удаление снапшотов вне продукта.
- [корзина] `trashItem` — перемещение внутри тома (на Data — в `~/.Trash`, на других томах — в
  `<том>/.Trashes/<uid>`; `URLForDirectory(.trashDirectory)` для образа возвращает nil — preflight бесполезен, DTS:
  просто вызывать); по выводу разведки это `rename` внутри ФС (Apple не документирует — тест `tray_trash` проверяет
  FILEID и время); ошибки по элементу: 513 нет доступа, 642 read-only, 4 исчез, 3328 том без корзины, 640 нет места;
  на macOS 26.1 read-only образ (даже с `.Trashes/<uid>`) отвечает 3328, а не 642.
  Put Back Finder для `trashItem` сломан с 2015 г. — продукт ведёт журнал возврата через `moveItemAtURL:toURL:`;
  публичного API очистки Корзины нет — «Открыть Корзину». «Освободится после очистки Корзины» = Σ
  `reclaimable(node)` в момент показа; журнал хранит ALLOCSIZE, флаги и CLONEID, не PRIVATESIZE.
- [дубликаты] Хэш читается `pread` кусками по 4 МиБ с дескриптора под `F_NOCACHE`, куски ≥ 128 КиБ идут в
  `update_rayon`, а не через `update_mmap_rayon`: страницы mmap проходят мимо `F_NOCACHE`, а файл, ставший dataless
  после проверки флагов, при обращении к странице даёт SIGBUS вместо EDEADLK. `fcopyfile` с `COPYFILE_METADATA`
  удаляет на клоне xattr, которых нет у копии (тест `clone_replace_image`).
- [текст] SDF/MSDF ниже ~18 px нечитаемы (нет хинтинга; субпиксельное LCD-сглаживание в macOS по вторичным
  источникам отключено с Mojave — проверяется визуально на 1x) — весь текст диаграммы растровый на точный
  device-размер через Core Text; AppKit-подписи поверх MTKView отклонены (`presentsWithTransaction`, размытие
  `NSTextField` под трансформацией на macOS 26) — AppKit только для панелей.
- [текст] Onest и JetBrains Mono в системе нет — бандлятся (OFL) и подключаются ключом `ATSApplicationFontsPath`
  Info.plist приложения; форматирование размеров, счётчиков и давности — только `domovoi-theme`.
- [metal] `CAMetalLayer.colorspace` по умолчанию nil (без colormatching) — выставляется sRGB; drawable
  `bgra8Unorm_srgb` (аппаратный блендинг геометрии в линейном пространстве), premultiplied-альфа для `*-soft`
  токенов; текст — режим `linear-corrected` (поправка в фрагментном шейдере, как у Ghostty), потому что чистый
  линейный блендинг делает тёмный текст тоньше, чем у `NSTextField`; паритет цвета и толщины закрепляют тесты
  `color_parity` и `golden_offscreen`. GitHub-раннеры дают «Apple Paravirtual device» (Apple5, без simdgroup/bfloat) —
  пиксельные эталоны с допуском, точные — на self-hosted; пороги кадра задаются по результатам `benches/frame`, до
  этого критерий — только корректность раскладки.
- [инференс] `GGML_METAL_EMBED_LIBRARY=ON` встраивает ИСХОДНИК шейдеров и компилирует их при каждой инициализации
  бэкенда (тёплый системный кэш 0,015–0,09 с); холодная компиляция исходников ядер (36 библиотек параллельно, ключ
  кэша сброшен уникальным `#define`) — 10,2 и 9,5 с на M4 Pro (`metal_cold_compile`, `#[ignore]`). С `EMBED=OFF`
  ggml ищет `default.metallib` (и `ggml-tensor.metallib` при SDK ≥ 26) в бандле (`bundleForClass` → бандл сервиса),
  затем рядом с исполняемым файлом (`argv[0]`); `GGML_METAL_PATH_RESOURCES` на пине указывает только каталог
  ИСХОДНИКОВ для запасной компиляции (`<путь>/kernels/*.metal`), не путь к metallib — тест с Metal и без бандла
  кладёт metallib из `domovoi_llama_sys::METALLIB_DIRECTORY` рядом с бинарником. `LLAMA_CURL` на пине устарел
  (только предупреждение); сеть из сборки убирает `LLAMA_OPENSSL=OFF`.
- [инференс] `n_gpu_layers=0` Metal не отключает; CPU-профиль = пустой `devices` в `llama_model_params` плюс окружение
  `GGML_METAL_DEVICES=0` (иначе даже `vocab_only` создаёт MTLDevice); GPU-профиль = `devices` из одного
  `ggml_backend_dev_by_type(GPU)` — без Metal-устройства загрузка завершается ошибкой, а не тихим CPU-фолбэком.
  `llama_set_abort_callback` на пине раздаёт колбэк всем бэкендам контекста через proc-адрес
  `ggml_backend_set_abort_callback` (Metal включён), но Metal на пине проверяет его только в capture-режиме — отмена
  срабатывает на границе графа (ubatch): задержка ≈ время одного ubatch, на M4 Pro с Qwen3.5-4B при `n_batch =
  n_ubatch = 256` — 222–228 мс после прогрева (первый decode собирает pipeline-объекты ядер, 522 мс);
  `metal_cancel_latency` прогревает; TPOT через сервис в песочнице 17,6 мс против 17,7–18,1 мс in-process (≤ 2,3 %);
  `llama_decode` возвращает 2 при отмене. Модель из дескриптора — `fdopen` → `llama_model_load_from_file_ptr` (llama.h
  на пине, строка 532), `load_mode = MMAP` (секция данных GGUF выровнена на 32 байта); `FILE*` остаётся за вызывающим.
  KV q8_0 требует размер головы, кратный 32: tiny-GGUF — одна голова 32.
- [инференс] `llama_sampler_init_grammar` возвращает NULL на невалидной грамматике (NULL в цепочке — UB) и no-op
  на пустой строке; `llama_sampler_init_grammar_lazy` deprecated; KV-кэш не ниже q8_0; под грамматикой ≈ 2× время
  на токен (issue #4218 открыт).
- [инференс] XPC-сервис stateless: launchd убивает по простою (SIGKILL), `xpc_transaction_begin/end` deprecated —
  настройка «выгрузка модели» имеет значения «сразу / 5 / 15 / 60 минут / без таймера приложения» (при idle-kill
  launchd модель перезагружается следующим запросом, «никогда» как «всегда в памяти» нереализуемо); QoS потока
  генерации — `USER_INITIATED`; сторож зависшего сервиса — `kill(pid, SIGKILL)` из приложения по pid из handshake.
- [инференс] Именованный сервис доступен только процессу из бандла (`Contents/XPCServices/*.xpc`,
  `CFBundlePackageType XPC!`); анонимные listener/endpoint есть только в connection-API (`xpc_connection_create(NULL)`,
  `xpc_endpoint_create`, `xpc_connection_create_from_endpoint`) — ими тестируется кодирование протокола в одном
  процессе; `xpc_listener_*`/`xpc_session_*` покрываются только тестом из временного `TestHost.app` с ad-hoc
  подписью. Решение 9 обеспечивается entitlements сервиса `com.apple.security.app-sandbox` без `network.client`:
  TCP-соединение даже на localhost из сервиса — EPERM, веса через `xpc_fd_create` читаются; Metal из бандла сервиса в
  песочнице работает (`metal_in_sandbox`, M4 Pro, macOS 26.1; ночной тест остаётся). Хост `TestHost.app` — копия
  самого тестового бинарника (`--exact host_scenario --ignored`; ночной `--ignored` идёт с `--skip host_scenario`),
  сервис — копия бинарника с боевым `Info.plist` (идентификатор подменён `plutil`, ключ Team ID удалён,
  `XPCService.EnvironmentVariables` вставлены) и metallib в `Contents/Resources`; после падения сервиса новая сессия
  получает новый процесс (другой pid). Таймер выгрузки сервиса — `DOMOVOI_INFERENCE_IDLE_UNLOAD_SECS` из plist
  (тесты); настройку пользователя исполняет приложение командой `unload` («сразу» — после `done`). `FakeBackend`
  исполняет строки промпта: `token <текст>`, `delay <мс>`, `fail <код> <описание>`, `hang`, `crash`, `probe-network
  <адрес>`, `weights`. Ночные тесты с моделью каталога берут путь к GGUF компактной записи из
  `DOMOVOI_NIGHTLY_COMPACT_MODEL` (переменная репозитория для self-hosted раннера).
- [грамматика] GBNF ставит `-INFINITY` недопустимым токенам — даже случайная модель выдаёт валидный JSON (ветку
  `tool_call`/`final` она выбирает произвольно); правило `space` без пробельных цепочек, `n_predict ≥ 256` в тестах;
  `maxLength ≥ 2000` ломает парсер. `decode_utf8` сэмплера на пине маскирует ведущий байт по полубайту: F5–FF
  декодируются как 4-байтные последовательности до 0x3FFFFF, суррогаты и overlong не проверяются — случайная модель
  под грамматикой выдавала невалидный UTF-8; класс `char` исключает `\uD800-\uDFFF` и `\U00110000-\U003FFFFF`,
  overlong грамматикой не закрывается — его ловит строгий парсер (`test-gbnf-validator` декодирует строго через
  unicode.cpp и путь сэмплера не представляет). По arXiv:2606.25605 (medium: модели и движки не названы) совмещение
  tool calling и схемы ответа в одном проходе может подавлять вызовы; формат «один объект: вызов или ответ» — тот же
  сценарий, поэтому метрика эвала «доля вызовов инструмента при явной необходимости» гейтит фиксацию формата; схема и
  пример видны в system-промпте.
- [каталог] Только семейство ChatML, все Apache-2.0, GGUF Q4_K_M: компактная — Qwen3.5-4B 2,74 ГБ (primary),
  Qwen3-4B-Instruct-2507 2,50 ГБ; стандартная — T-lite-it-2.1 5,03 ГБ (primary, ruBFCL 56,5, non-thinking),
  Qwen3.5-9B 5,68 ГБ; расширенная — Qwen3-14B 9,00 ГБ (primary, единственный кандидат: Hermes-4-14B без
  документированного русского, Gemma 4 12B и Mistral Nemo — другие семейства). Один Rust-шаблон ChatML с префиксом
  ассистента `<think>\n\n</think>\n\n` (официальные шаблоны Qwen3.5 при `enable_thinking=false` и T-lite всегда);
  golden-промпт — на каждую запись каталога (репозиторий + ревизия), а не на семейство: вендоры правят шаблоны и
  перевыкладывают GGUF. Запись каталога хранит ревизию HF, размер, SHA-256 и текст LICENSE.
- [шаблоны] В GGUF-репозиториях каталога нет `chat_template.jinja` — официальный шаблон записи = `tokenizer.chat_template`
  из заголовка её Q4_K_M-файла на ревизии (скрипт читает заголовок HTTP Range-запросами). Без `tools` и при
  `enable_thinking=false` шаблоны различаются двумя параметрами записи (`template` в `assets/catalog.json`):
  префикс `<think>\n\n</think>\n\n` перед ответом — у Qwen3.5-4B/9B, T-lite-it-2.1 и Qwen3-14B, но не у
  Qwen3-4B-Instruct-2507; тот же пустой блок у шагов ассистента после последнего вопроса пользователя — только у
  Qwen3.5. Шаблоны Qwen3.5 обрезают пробелы по краям содержимого, остальные — нет: Rust-шаблон обрезает всегда.
  GBNF грамматик ответа проверен `test-gbnf-validator` llama.cpp на пине.
- [каталог] GPU-бюджет `recommendedMaxWorkingSetSize` ≈ 74 % памяти (8 ГБ → ≈ 5,9 ГБ) на веса + KV + compute рядом
  с индексом скана 0,6–0,9 ГБ; `n_ctx` по классу 8k / 16k / 32k; выгрузка модели по
  `DISPATCH_SOURCE_TYPE_MEMORYPRESSURE` WARN (следующий запрос перезагружает), CRITICAL — аварийная выгрузка и отказ
  от новой загрузки до спада давления. Таблица `ClassFootprint` и `MemoryBudget` живут в inference-proto рядом с
  `ModelClass`: их читают и сервис (отказ в загрузке), и downloader (модель по умолчанию — класс по `hw.memsize`,
  при нехватке бюджета ступенью ниже).
- [загрузка] Проверено на HF (16.10.2026): resolve `huggingface.co/<repo>/resolve/<rev>/<file>` → 302 с
  `X-Repo-Commit`, `X-Linked-Etag` = SHA-256 файла и `X-Linked-Size`; `Location` — подписанная ссылка xet-CDN
  (`us.aws.cdn.hf.co/xet-bridge-us/…`) с `Expires=<unix>` ≈ 1 ч. ETag CDN — xet-хэш, НЕ SHA-256, и CDN игнорирует
  `If-Range` (206 даже с чужим валидатором): загрузчик хранит ETag CDN в xattr `com.domovoi.download-validator`
  частичного файла, шлёт его в `If-Range` и принимает 206 только при `Content-Range` ровно со смещения и том же ETag,
  иначе начинает файл заново; SHA-256 сверяется с `X-Linked-Etag` до передачи и потоково (CommonCrypto) после. Делегат
  `NSURLSession` отказывается от редиректа (completion с nil) — 302 приходит как финальный ответ; сессия ephemeral без
  `URLCache`. Докачка после двух пауз с живого HF побайтно совпала с эталоном. У GGUF-репозиториев каталога, кроме
  Qwen3-14B-GGUF, нет LICENSE: текст берётся из базовой модели на закреплённой ревизии (`license_source`), у
  T-lite-it-2.1 его нет нигде — канонический Apache-2.0. Test-only записи — `assets/test/catalog.json` (tiny-random)
  вне встроенного каталога. Отмена и удаление моделей — только в Корзину (`trash_item` из rules); `.Trashes` тома не
  листается — тесты читают `.Trashes/<uid>`.
- [агент] Контекст агента — индекс, правила, denylist и `LevelTotals` от вызывающего (приложение берёт их из
  `Recommendations`, поэтому числа модели совпадают с экраном). Инструменты только читают; действия строятся из узлов
  результатов инструментов: правило не «проверьте сами» и не инфо, denylist молчит, вложенные в уже предложенный узел
  отбрасываются; текст ответа действий не создаёт. Гипотезы остаются, только если путь есть в индексе, и уходят в
  `Recommendations` как `Hypothesis` (всегда «проверьте сами»). Бюджет промпта — точное число токенов от бэкенда
  (`InferenceBackend::count_tokens`, в протоколе `count_tokens` → `token_count`, тот же `llama_tokenize`, что и в
  `generate`): промпт плюс `max_tokens_per_step` не больше `context_length`, иначе старые результаты инструментов
  заменяются на `{"trimmed":true}`. Оценка в байтах отвергнута: у Qwen3.5-4B на промпте агента 4,33 байта на токен,
  байтовый бюджет занимал около четверти контекста 8k и вырезал результаты уже после первого поиска. Последний шаг
  идёт под `ResponseSchema::answers_only()` — цикл заканчивается не позже N шагов; первый шаг по выбору —
  `tool_calls_only`. Защита от инъекций: `LoadedModel` токенизирует с `parse_special`, поэтому имя папки `<|im_end|>…`
  стало бы спецтокеном — в JSON результатов `<`/`>` экранируются как `\u003c`/`\u003e`, в вопросе заменяются на ‹ ›.
  `FakeBackend::replying` отвечает из очереди скриптов и пишет запросы (`requests()`); `smoke_real_modules`
  перезапускает себя с `GGML_METAL_DEVICES=0`, потому что agent запрещает `unsafe` (`set_var`).
- [эвал] 64 задачи в `crates/eval/tasks.json` (поиск, осмотр узла, пояснение правила, сводка, вопросы без инструмента)
  на синтетическом индексе с корнем `/Users/fixture`, чтобы срабатывали правила с `~/`; `reclaimable` требует
  настоящих файлов, поэтому итоги по уровням фикстуры считаются по `allocated` захваченных узлов. Судья — та же модель
  с рубрикой голоса раздела VIII и грамматикой `{score 1–5, reason}` (ограничение: модель оценивает сама себя). TPOT —
  8 вопросов по 128 токенов с грамматикой и без. `cargo xtask eval --class <класс>` гоняет `#[ignore]`-тест в release,
  модель берётся из `DOMOVOI_NIGHTLY_<CLASS>_MODEL`, отчёт — `target/eval/<класс>.json`, пороги проверяются для
  primary-записи класса.
- [рендер] Цель — `bgra8Unorm` (не `_srgb`) со слоем sRGB: Core Graphics смешивает в sRGB-кодированных значениях, а
  `_srgb`-цель смешивала бы линейно и разошлась бы с `NSView` на `*-soft` токенах; `color_parity` сверяет 12 токенов в
  обеих темах с точностью ±1. Кэш `bitmapImageRepForCachingDisplayInRect` у вида без окна — Generic RGB (гамма 1.8),
  поэтому эталон `NSView` рисуется в sRGB `CGBitmapContext` через `NSGraphicsContext` и
  `displayRectIgnoringOpacity:inContext:`. `.metallib` собирает `build.rs` крейта render (`xcrun metal` / `metallib` в
  OUT_DIR, путь — `BUILT_METALLIB`), как llama-sys: тестам и бандлу не нужен отдельный шаг xtask. Глифы растеризуются
  белым по чёрному в BGRA-контексте со сглаживанием (дилатация как у `NSTextField`), покрытие — зелёный канал, атлас
  R8 2048², 4 субпиксельных варианта; текст атласа отличается от `CTLineDraw` в среднем не больше чем на 4/255.
  Рендерер перезаливает атлас по паре «идентичность атласа, ревизия»: новый атлас с той же ревизией иначе рисовал
  чужие глифы (поймал `golden_offscreen`). Onest и JetBrains Mono регистрируются `CTFontManagerRegisterFontsForURL`
  (процесс), вес задаёт ось `wght` через `kCTFontVariationAttribute`. Строки диаграммы (категории, «занято», «↑
  наверх») render получает в `DiagramStrings` от вызывающего. Тесты с AppKit (`color_parity`, `accessibility`) идут с
  `harness = false`: AppKit требует главный поток. Эталоны — `crates/render/tests/golden/*.png` (M4 Pro, macOS 26.1):
  допуск — не больше 0,5 % пикселей с отклонением канала больше 24, точное сравнение при `DOMOVOI_GOLDEN_EXACT=1`,
  перезапись при `DOMOVOI_GOLDEN_UPDATE=1`. Квад дуги ограничен рамкой сектора (концы и экстремумы осей): GPU-кадр
  sunburst при 2x — 0,46 мс вместо 7,9 мс.
- [рендер] Первый замер `benches/frame` (M4 Pro, 2x): 1,01 млн узлов — sunburst раскладка 2 мс, 121 инстанс, GPU 0,46
  мс; treemap 0,9 мс, 11 306 инстансов (фигуры и глифы), GPU 1,0 мс, подписано 91,8 % площади; 5,67 млн — sunburst 15
  мс, 172 инстанса, GPU 0,19 мс; treemap 2,5 мс, 24 164 инстанса, GPU 1,15 мс, подписано 90 %. Пороги фиксируются по
  ночному self-hosted.
- [сборка] objc2 0.7 / frameworks 0.4 объявлены (BREAKING `ivars` в `define_class!`); примеры objc2 на `main`
  написаны под нерелизный API — референс только тег `objc2-0.6.5`; код `define_class!` держит ivars отдельной
  структурой; libc 1.0 — в ту же фазу миграции.
- [тесты] В `mkdtemp` на Data-томе воспроизводимы hard links, клоны (`cp -c`), sparse, decmpfs (`ditto
  --hfsCompression` ≥ 64 КиБ), `chmod 000` (EACCES), ELOOP, глубина > PATH_MAX; там нельзя ассертить `PRIVATESIZE` и
  `ATTR_VOL_*`. Только образ даёт точку монтирования, второй REALDEVID, детерминированные объёмы, case-sensitive,
  Unmount и обе ветки истории FSEvents; образы — `hdiutil create -fs APFS` с `-owners on`, один файл на тест,
  `mdutil -i off` только на browsable-монтировании (на `-nobrowse` команда падает), detach с retry и `-force`;
  `tmutil localsnapshot` в тестах запрещён (снапшотит системный том) — ветка «удержано снапшотом» проверяется
  unit-тестом с подставленными атрибутами, на живом снапшоте — ручная приёмка.
- [UI] Снимки для agent-judged: `domovoi --snapshot <экран|all> --theme <dark|light> --scale <1|2> --state
  <fixture.json> [--contrast high] <dir>` рендерит окна offscreen (sRGB `CGBitmapContext` +
  `displayRectIgnoringOpacity:inContext:` + текстура диаграммы; кэш `bitmapImageRepForCachingDisplayInRect` — Generic
  RGB, не годится) без Screen Recording, состояния экранов — из фикстуры, не с хоста; файлы
  `<screen>-<theme>@<scale>x.png`, ревьюер читает их из `target/snapshots/` (gitignored); «Open in Low Resolution» —
  по архивному документу Apple 2012 г. magnified mode без фильтрации, актуальность на macOS 15/26 проверяется
  вручную.
- [UI] Доступность: `NSAccessibilityElement` на сегмент диаграммы; при `accessibilityDisplayShouldIncreaseContrast`
  рамки и разделители — системные `NSColor::separatorColor`/`labelColor` вместо `line`/`line-soft` (у книги нет
  HC-токенов); `reduce_motion` — смена кадра вместо анимации.
- [UI] Рядом с каждым PNG пишется `<stem>.json`: окно, масштаб и пробы (фон окна, сайдбар, акцентная кнопка) в точках
  от верхнего левого угла — по ним `theme_pixels` сверяет токены. На macOS 26 `NSSegmentedControl` — SwiftUI-хостинг на
  слоях (`_NSCoreHostingView`) и в offscreen-снимок не попадает: переключатель режимов — свой из `ShellButton`.
  `NSSplitViewController` включает автолэйаут всей иерархии: вид без собственного размера в `NSStackView` сжимается до
  нуля, а стек шире панели раздвигает её — легенда раскладывается фреймами с переносом строк. Делегаты таблиц слабые:
  экранная структура живёт до конца отрисовки снимка.
  `NSAppearance(named:)` для имён `AccessibilityHighContrast*` возвращает обычные `DarkAqua`/`Aqua`: вариант высокого
  контраста существует только при включённой системной настройке, поэтому снимок `--contrast high` проверяет замену
  `line`/`line-soft` на `separatorColor`, но не его системное усиление — оно видно только в живом окне.
- [UI] Живое окно раскрывает дерево индекса по бюджету 200 тыс. узлов: цепочка фокуса и кольца под ним, затем
  крупнейшие каталоги; зум за раскрытое перестраивает дерево вокруг фокуса без анимации (`DiagramState::jump_to`).
  «Скрытое», «Другие тома», «macOS» — из `Volumes` с источником цифр в тултипе; нулевые сегменты и тома других
  контейнеров в круг не входят. «в облаке» — логический размер dataless-файлов вне суммы, строкой в шапке. Имя диска —
  `NSURLVolumeLocalizedNameKey` тома `/` для Data-тома, иначе корня скана.
- [UI] `--e2e <scenario.json> --report <report.json>`: шаги `scan`, `reopen_cache`, `await_change {path,
  timeout_seconds}`, `recommendations`, `snapshots {directory, scale}`; отчёт — `steps`, `totals` по уровням,
  `snapshots`. Шаги Корзины, дубликатов, загрузки модели и вопроса Домовому добавляют ui-care и ui-ai вместе со своими
  экранами и XPC-клиентом.
- [UI] В бандле шейдеры диаграммы — `Resources/diagram.metallib` (через `NSBundle`), шрифты — `ATSApplicationFontsPath =
  Fonts`; вне бандла — `BUILT_METALLIB` и регистрация `assets/fonts`. Шаблон — `crates/app/bundle/Info.plist`, версии
  штампует distribution. Живое окно (проба FDA, возврат из Настроек → «Перезапустить», скан с прогрессом, дельты
  FSEvents, тултип-панель, смена темы и контраста) гейтами не покрывается — приёмка человеком после wire.
- [UI] `Index::rescan` пересобирает таблицу узлов (`compact`): любой `NodeId` после изменения индекса недействителен.
  `Session` после каждого изменения (дельты FSEvents пачкой, Корзина, возврат, замена клоном) пересопоставляет Корзину
  Домового и группы дубликатов по путям и пересчитывает рекомендации; фокус диаграммы и раскрытие дерева держатся по
  пути. Удалённый каталог пересканируется от ближайшего предка, который есть и в индексе, и на диске.
- [UI] Экраны 04/06/08 строятся из моделей вида (`RecommendationsModel`, `DuplicatesModel`, `TrayModel`): снимки берут
  их из фикстуры (`care`), живое окно и `--e2e` — из `Session`. Состояние Корзины и журналов передаётся путями
  (`SessionPaths`): `for_user` в приложении, `in_directory` в тестах и сценарии — тесты не трогают журналы
  пользователя. Предусловия «Заменить клоном» (`ClonePlan::prepare`, включая поиск открытых дескрипторов)
  проверяются только в режиме клона. Все элементы управления — свои `drawRect`-виды (чекбокс, переключатель, сегменты):
  системные контролы macOS 26 на слоях в offscreen-снимок не попадают. `--e2e` умеет `trash_recommendations {level}`,
  `restore_last`, `find_duplicates`, `replace_with_clone`.
- [UI] Экраны 05/07 строятся из `ChatModel` и `ModelScreenModel` (фикстура `ai`: `chat`, `model`, `model_off`); состояние
  чата (`Availability`: выключен, нет модели, загружается, выгружена, готов, отвечает) — вариант экрана, файлы
  `05-<состояние>-<тема>@<масштаб>x.png`, основное состояние без суффикса; `--snapshot 05` пишет все варианты, `--e2e`
  — только основные. `NSGridView` внутри разметки на фреймах требует фиксированных размеров ячеек (ограничения ширины и
  высоты) — `intrinsicContentSize` своих кнопок не совпадает с фреймом. Шаги агента в чате — по `ToolStep`, кнопка
  «Добавить в Корзину» хранит пути и правила ответа (не `NodeId`) и разрешает их в момент нажатия.
- [инференс] Клиент приложения (`InferenceClient`) — `InferenceBackend` поверх XPC: запрос идёт во вспомогательном
  потоке, токены приходят отдельными сообщениями в канал; сторож следит за паузой без токенов (`stall`) и делает
  `kill(pid, SIGKILL)` по pid из handshake, затем переподключается и заново грузит веса из сохранённого дубликата
  дескриптора; ошибки различимы кодами `Stalled` и `Unavailable`. Именованный сервис проверяется только из
  `TestHost.app` (`smoke_inference_client`: тест-бинарник в роли хоста и сервиса), анонимный endpoint — в процессе.
- [модель] Скан папки моделей сверяет SHA-256 каждого файла — идёт в фоне, до его конца карточки в состоянии «проверяю
  SHA-256» и загрузки не стартуют; дальше отчёт обновляется по событиям (`ModelEvent` в индекс через `Session`,
  отложенно, если `Session` занята задачей). Настройки AI (`AssistantSettings`) — в `NSUserDefaults`; зеркало
  принимается только `https://`; таймер выгрузки ставится после ответа и снимается при новом вопросе.

# Gameplan

Линейная цепочка (конвертер строит deps по порядку фаз; комментарии `<!-- deps: … -->` — документация
потребления). Границы `until`: `scanner`, `index`, `rules`, `inference`, `render`, `ui-shell`. Терминал — `wire`.

### Phase 1: workspace — каркас workspace, конвенции, CI, правки источников истины

- [ ] `Cargo.toml` workspace (resolver 3, `[workspace.package]` version 0.1.0, edition 2024, rust-version 1.94),
  `rust-toolchain.toml` (stable 1.94, clippy, rustfmt), `.cargo/config.toml` с alias `xtask = "run --package xtask --"`.
- [ ] Каркасы крейтов в `crates/`: apfs-sys, testkit, scanner, index, rules, dups, theme, render, grammar, llama-sys,
  xpc-sys, inference-proto, inference (bin `domovoi-inference`), downloader, agent, eval, app (bin `domovoi`), xtask —
  `lib.rs`/`main.rs`; политика `unsafe` по Conventions (`forbid` / `deny` + модуль `ffi.rs` / разрешено).
- [ ] `[workspace.dependencies]` с пинами из Stack; `[workspace.lints]` (clippy pedantic выборочно,
  `unsafe_op_in_unsafe_fn = "deny"`), `rustfmt.toml`.
- [ ] `xtask check-comments`: падает, если комментарий в `crates/` упоминает книгу (`design-book`, `книг`), фазы
  (`Phase `, `фаза `) или спеку (`SPEC-`).
- [ ] `xtask check-tokens`: падает на hex-литералах цветов (`#[0-9A-Fa-f]{6}`) и `rgba(` вне `crates/theme`.
- [ ] `xtask check-sources`: падает, если в книге остались плашки «Решение открыто» или строка «SDF для чёткости»,
  а в документе — `(dev, inode)` или «metallib встроена».
- [ ] `.github/workflows/ci.yml`: матрица `macos-15` и `macos-26` (arm64), шаг `xcodebuild -downloadComponent
  metalToolchain` (кэшируется), sccache для cargo и cmake (`CMAKE_C_COMPILER_LAUNCHER`), rust-cache, fmt / clippy /
  test, job-канарейка hdiutil attach/detach с retry, nightly job `self-hosted, macOS, ARM64` (allow-failure) с
  содержимым из Stack.
- [ ] Правки `docs/DOMOVOI.md`: решение 2 (ключи `ATTR_CMNEXT_REALDEVID` / `FILEID` / `CLONEID` / `LINKID`),
  решение 7 (предкомпилированные `default.metallib` и `ggml-tensor.metallib` в бандле сервиса), решение 10 (свой
  конвертер на Rust), раздел 4.3 (растровый атлас), раздел 4.4 (`xcode.device-support` и `downloads.installers` —
  Восстановимо, `xcode.simulators` — Проверьте сами, с причинами; абзац о клонах pnpm и выигрыше по `reclaimable`),
  новое решение «только Apple Silicon», раздел 3 закрыт выбором веера, раздел 6 — итоги проверок.
- [ ] Правки `design/domovoi-design-book.html`: плашки «Решение открыто» (разделы V, экраны 06 и 07) → принятые
  решения (оба режима, Корзина по умолчанию + клон, каталог ChatML); IX строка «Рантайм» — metallib
  предкомпилирована в бандле сервиса; X строка «подписи treemap» — растровый атлас, масштабирование битмапа при
  зуме; RULES — уровни по разделу 4.4 документа.
- [ ] `README.md`: сборка, профили подписи, команды xtask, минимальная macOS 15, Python ≥ 3.10 для `research/`.

**Done when (EXECUTABLE):**
```
cargo build --workspace
```
- Все 18 крейтов собираются (exit 0). Обоснование executable: каркас проверяется компилятором.
```
cargo clippy --workspace --all-targets -- -D warnings
```
- Линты workspace чисты (exit 0).
```
cargo fmt --all -- --check
```
- Форматирование единое (exit 0).
```
cargo xtask check-comments
```
- Комментарии не ссылаются на книгу, фазы и спеку (exit 0).
```
cargo xtask check-sources
```
- Документ и книга приведены к принятым решениям (exit 0).
```
grep -q ATTR_CMNEXT_REALDEVID docs/DOMOVOI.md
```
- Решение 2 переформулировано в документе (grep exit 0).
```
grep -q default.metallib docs/DOMOVOI.md
```
- Решение 7 переформулировано в документе (grep exit 0).
```
test -f .github/workflows/ci.yml
```
- CI-конфигурация существует (exit 0).

### Phase 2: theme — токены раздела XI в Rust, движение, шрифты, форматирование

<!-- deps: [workspace] — потребление: workspace даёт xtask и каркас крейта. -->

- [ ] `xtask theme`: парсер `<style>` книги (scraper + cssparser): блок `:root{…}` → Dark,
  `:root[data-theme="light"]{…}` → Light; цвета (hex, rgba), радиусы, шрифтовые стеки; падение при токене,
  отсутствующем в одной из тем.
- [ ] Генерация `crates/theme/src/generated.rs` (quote + prettyplease): `Palette` с полями по токенам (`bg`,
  `surface`, `wax`, `c_apps` …), `Palette::DARK` / `LIGHT`, `Rgba` `#[repr(C)]` `bytemuck::Pod` (`[f32;4]`, sRGB,
  premultiplied-альфа для `*-soft`), `ns_color()` через `colorWithSRGBRed_green_blue_alpha`,
  `palette_for(&NSAppearance)` через `bestMatchFromAppearancesWithNames([Aqua, DarkAqua])`.
- [ ] `xtask theme --check` сравнивает сгенерированное с закоммиченным (ensure_file_contents, подсказка
  «run cargo xtask theme»).
- [ ] `Motion`: 220 / 120 / 320 мс из таблицы раздела IV; 1400 мс орнамента завершения — из демо-скрипта книги,
  фиксируется как решение спеки; `reduce_motion()` через `NSWorkspace::accessibilityDisplayShouldReduceMotion` и
  подписка на `NSWorkspaceAccessibilityDisplayOptionsDidChangeNotification`.
- [ ] Шрифты `assets/fonts/`: Onest 2.001 (`Onest[wght].ttf`, Google Fonts commit `8739b19`), JetBrains Mono
  (релиз с OFL), файлы `OFL.txt`; манифест `fonts.toml` с SHA-256; подключение через `ATSApplicationFontsPath`.
- [ ] Типографика раздела III: `format_size` (десятичные единицы, запятая, NBSP, «ГБ» / «МБ», один знак < 100 ГБ),
  `format_count` («1 файл / 2 файла / 5 файлов», разряды через NBSP) и `format_age(then, now, source)` («сегодня /
  вчера / N дней / N месяцев / N лет назад» со склонениями и подписью источника даты) — единственная реализация для
  AppKit и Metal.
- [ ] Тесты (`crates/theme/tests/`): `tokens.rs` — значения против книги (`wax` Dark `#E3AE52`, Light `#A66C10`;
  `c_model` Dark `#E3AE52`, Light `#B07714`), премультипликация `*-soft`; `typography.rs` — golden `format_size`
  («222,1 ГБ», «38,2 ГБ», «46 МБ»), `format_count` («1 284 112 файлов»), `format_age`; `motion.rs` — константы и
  reduce motion.

**Done when (EXECUTABLE):**
```
cargo xtask theme --check
```
- Сгенерированный модуль совпадает с книгой (exit 0). Обоснование executable: генератор детерминирован.
```
cargo test -p domovoi-theme
```
- Токены, движение и форматирование соответствуют разделам II–IV, XI (exit 0).
```
cargo clippy -p domovoi-theme --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).

### Phase 3: apfs-sys — биндинги и safe-обёртки над APFS, TCC-errno, libproc, ввод-вывод

<!-- deps: [theme] — order-only (кода theme не потребляет). -->

- [ ] bindgen над `wrapper.h` (`sys/attr.h`, `sys/stat.h` с `_DARWIN_C_SOURCE`, `sys/clonefile.h`, `sys/snapshot.h`,
  `sys/resource.h`, `sys/mount.h`, `sys/fcntl.h`, `copyfile.h`, `libproc.h`, `xattr_flags.h`): allowlist `ATTR_.*`,
  `FSOPT_.*`, `EF_.*`, `SF_.*`, `UF_.*`, `CLONE_.*`, `DIR_MNTSTATUS_.*`, `VOL_CAP_.*`, `IOPOL_.*`, `MNT_EXT_.*`,
  `COPYFILE_.*`, `F_NOCACHE`, `fs_snapshot_list`, `renamex_np`, `renameatx_np`, `RENAME_SWAP`, `RENAME_EXCL`,
  `proc_listpids`, `proc_pidpath`, `proc_listpidspath`, `xattr_flags_from_name`. Факт: libc 0.2.190 не содержит
  `EF_*`, `CLONE_*`, `ATTR_CMNEXT_CLONE_REFCNT`, `ATTR_CMN_ERROR`, `SF_FIRMLINK`, `fs_snapshot_*`.
- [ ] Тест значений констант на пин SDK: `EF_SHARES_ALL_BLOCKS` 0x40, `EF_MAY_SHARE_BLOCKS` 0x1, `EF_IS_PURGEABLE` 0x8,
  `EF_IS_SPARSE` 0x10, `ATTR_CMNEXT_CLONE_REFCNT` 0x1000, `ATTR_CMNEXT_PRIVATESIZE` 0x8, `SF_FIRMLINK` 0x00800000,
  `SF_RESTRICTED` 0x00080000, `SF_DATALESS` 0x40000000, `ATTR_CMN_ERROR` 0x20000000, `FSOPT_ATTR_CMN_EXTENDED` 0x20,
  `DIR_MNTSTATUS_MNTPOINT` 0x1.
- [ ] `BulkListing`: запрос по `dirfd` (RETURNED_ATTRS | NAME | ERROR | OBJTYPE | FLAGS | FILEID | MODTIME | CRTIME |
  ADDEDTIME; DIR_MOUNTSTATUS; FILE_LINKCOUNT | TOTALSIZE | ALLOCSIZE | DATAALLOCSIZE; CMNEXT REALDEVID | LINKID |
  CLONEID | EXT_FLAGS | CLONE_REFCNT, PRIVATESIZE опционально), парсер по битам `RETURNED_ATTRS` с 4-байтовым
  выравниванием через `memcpy`, буфер 256 КиБ с удвоением при ERANGE и защитой от пустого повтора; буфер имени
  ≥ 766 байт (имя APFS — 255 UTF-16 единиц). Факт: набор обязан содержать `ATTR_CMN_OBJTYPE` и не содержать
  `ATTR_CMN_UUID` / `GRPUUID` / `EXTENDED_SECURITY`, иначе ядро уходит в медленный `readdirattr`.
- [ ] Обёртки тома и идентичности: `volume_used(path)` (ATTR_VOL_SPACEUSED), `container_capacity(path)` (ATTR_VOL_SIZE /
  SPACEAVAIL), `data_volume()` (`getfsstat` + `MNT_EXT_ROOT_DATA_VOL`), `volume_capabilities(path)`
  (`VOL_CAP_FMT_CASE_SENSITIVE`, `VOL_CAP_INT_CLONE`, `VOL_CAP_INT_RENAME_SWAP`), `real_dev(path)`, `firmlinks()` (парсер
  `/usr/share/firmlinks`), `snapshots(root_fd)` (`fs_snapshot_list`: имя, CRTIME, класс по префиксу; вызов принимает
  только flags = 0, поэтому CMNEXT-атрибуты и LINKID через него недоступны), `private_size(path)`, `gen_count(path)`
  (требует `FSOPT_ATTR_CMN_EXTENDED`), `last_used_xattr(fd)`.
- [ ] Обёртки действий и ввода-вывода (для крейтов с `forbid(unsafe_code)`): `open_at`, `open_search`, `mkdir_at`,
  `pwrite_at`, `ftruncate`, `set_xattr`, `chflags`, `clone_at`, `rename_swap`, `copy_metadata` (copyfile
  `COPYFILE_METADATA | COPYFILE_NOFOLLOW`), `set_nocache(fd)` (`F_NOCACHE`), `set_thread_disk_policy_utility()`
  (`IOPOL_TYPE_DISK`, `IOPOL_SCOPE_THREAD`, `IOPOL_UTILITY`) и `thread_disk_policy()`, `mmap_readonly(path)`,
  `set_vfs_policies()`, `raise_nofile_limit()` (GUI получает мягкий лимит 256 fd; потолок ядро применяет молча),
  `running_bundle_roots()` (proc_listpids + proc_pidpath, работает для чужих uid), `pids_with_open(path)` (только
  свой uid).
- [ ] Ошибки: `AccessError::{Tcc(EPERM), Unix(EACCES), Missing(ENOENT), Other}` из errno.
- [ ] Тесты (`crates/apfs-sys/tests/`): `constants.rs` — значения на пин SDK; `bulk.rs` — парсер на temp-дереве и
  CMNEXT-атрибуты; `platform.rs` — Data-том найден и отличен от `/` по REALDEVID при равном `st_dev`;
  `fs_snapshot_list` работает без root; `/private/etc/master.passwd` → EACCES; firmlinks ≥ 18 строк; у Data-тома
  `VOL_CAP_INT_CLONE` и `VOL_CAP_INT_RENAME_SWAP`; каталог `--x` через `open_search` даёт EBADF на bulk;
  `proc_pidpath(1)` = `/sbin/launchd`; `volume_used(Data)` равен Used из `df -k /System/Volumes/Data` (± 1 блок),
  `container_capacity().available` равен `APFSContainerFree` из `diskutil info -plist /System/Volumes/Data`.

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-apfs-sys
```
- Константы, парсер bulk, обёртки и платформенные факты подтверждены на хосте (exit 0). Обоснование executable:
  всё проверяется вызовами ядра и системными утилитами.
```
cargo clippy -p domovoi-apfs-sys --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).

### Phase 4: scanner — обход Data-тома, классификация доступа, FSEvents, генератор фикстур

<!-- deps: [apfs-sys] — потребление: сканер и testkit читают и создают объекты через apfs-sys (smoke_apfs_sys). -->

- [ ] `domovoi-testkit`: builder-DSL дерева через обёртки apfs-sys (`file(size, fill)`, `hardlink`, `clone`, `sparse`,
  `xattr`, `compressed` через `ditto --hfsCompression` ≥ 64 КиБ, `mode(0o000)`, `symlink_loop`, `deep(> PATH_MAX)`
  через `mkdir_at`-цепочку, `busy_bit`, `last_used(ts)`, `pnpm_store(projects)`); `TempTree` (mkdtemp на Data-томе);
  `ImageVolume` (`hdiutil create -fs APFS` или `Case-sensitive APFS`, ≤ 64 МБ, attach `-nobrowse -owners on
  -mountpoint`, вариант browsable с `mdutil -i off` для истории FSEvents, detach с retry/backoff и `-force`; бэкенд
  `diskutil image` за флагом `DOMOVOI_IMAGE_BACKEND=diskutil` — `hdiutil` deprecated в macOS 27; корень тома
  `diskutil image` принадлежит root, поэтому этот бэкенд монтирует с `noowners`; на `-nobrowse` образе `.fseventsd`
  не создаётся; `mdutil -i off` отказывает томам под `/var/folders`, поэтому `TempTree` живёт в `/private/tmp`).
- [ ] Обход: корень — `data_volume()` (путь не хардкодить), DFS по `dirfd` одним компонентом (`open_at O_RDONLY |
  O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC`), `rayon::scope` work-stealing по поддеревьям, один fd на уровень на воркер;
  `raise_nofile_limit` и `set_vfs_policies` первыми строками. Факт: `PATH_MAX`=1024 исчерпывается на глубине 2
  при именах в 255 UTF-16 единиц (до 765 байт UTF-8), `std::fs` падает с ENAMETOOLONG — только dirfd.
- [ ] Правило пропуска по данным листинга родителя: не VDIR → не спускаться; `MNTPOINT | TRIGGER` → узел «другой
  том»; `SF_FIRMLINK` → не спускаться; `ATTR_CMN_ERROR` / EPERM при успешном `stat` → `HiddenByPrivacy`; `stat`
  EPERM → `DataVault`; EACCES и EBADF на `--x` → `OtherUser`; `SF_DATALESS` / EDEADLK → `InCloud`; симлинки не
  разыменовывать.
- [ ] Запись узла: имя, тип, flags, FILEID, REALDEVID, LINKID, CLONEID, EXT_FLAGS, CLONE_REFCNT, ALLOCSIZE, TOTALSIZE,
  LINKCOUNT, MODTIME, CRTIME, ADDEDTIME, `UF_COMPRESSED`; `last_used` из xattr для файлов без `EF_NO_XATTRS`;
  PRIVATESIZE не запрашивается в обходе (на деревьях 1,15–1,68 млн файлов набор с PRIVATESIZE/CLONE замедляет
  обход в 1,8–2,6×: 62,5 → 164,8 с и 51,4 → 92,5 с однопоточно; +70 % — только на тёплом Xcode.app).
- [ ] `FullDiskAccess::probe()`: `opendir` по `~/.Trash`, `/Library/Application Support/com.apple.TCC`,
  `~/Library/Safari` (ENOENT — пропуск, EPERM — нет, OK — есть); при запуске и перед каждым сканом; очередь обхода:
  Desktop / Documents / Downloads, `~/Library/Mobile Documents`, `~/Library/CloudStorage` — последними и только после
  сигнала «онбординг закрыт».
- [ ] Прогресс и отмена: поток событий (байт, файлов, текущий путь в firmlink-форме) для сектора скана.
- [ ] FSEvents в модуле `ffi.rs` через `objc2-core-services` (feature FSEvents; `fsevent-sys` мёртв, `notify` тянет
  лишнее): поток с корнем Data-тома, `FSEventsCopyUUIDForDevice` + `sinceWhen` для дельт; `HistoryDone` без событий
  при NULL-UUID → «история недоступна» → полный пересчёт; `Unmount`.
- [ ] Канонический путь — firmlink-форма (обратная карта `firmlinks()`); узлы Data вне карты — под
  `/System/Volumes/Data/<name>` как служебные; пути глубже PATH_MAX — цепочка имён.
- [ ] `benches/full_scan` (ignored, nightly): полный обход Data-тома тёплый / холодный, базовый набор, +CMNEXT без
  PRIVATESIZE, +PRIVATESIZE (разведка: +25 % на тёплом Xcode.app; 1,8–2,6× с PRIVATESIZE на миллионах файлов);
  цифры фиксируются следующей правкой спеки.
- [ ] Тесты (`crates/scanner/tests/`, `crates/testkit/tests/`): `builder.rs` (testkit) — все виды объектов и образ;
  `fixtures_tempdir.rs` — дедуп по иноду, клоны по CLONEID / EF, sparse, decmpfs, EACCES, ELOOP, глубина > PATH_MAX,
  инвариант Σ ALLOCSIZE по уникальным инодам == `du -sk`; `fixtures_image.rs` — пропуск точки монтирования с другим
  REALDEVID, case-sensitive том, детерминированные ATTR_VOL_*, FSEvents в реальном времени и Unmount, история
  доступна на browsable и недоступна на nobrowse; `smoke_apfs_sys.rs` — настоящий `BulkListing` на temp-дереве;
  `icloud_dataless.rs` (`#[ignore]`, nightly) — обход `~/Library/Mobile Documents` ≤ 2 с при наличии iCloud.

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-testkit
```
- Генератор фикстур создаёт все виды объектов и монтирует образ (exit 0).
```
cargo test -p domovoi-scanner --test fixtures_tempdir
```
- Дедуп по иноду, клоны, sparse, decmpfs, EACCES, ELOOP, глубина > PATH_MAX, инвариант с `du -sk` (exit 0).
```
cargo test -p domovoi-scanner --test fixtures_image
```
- Точка монтирования, case-sensitive том, ATTR_VOL_*, FSEvents, Unmount, обе ветки истории (exit 0).
```
cargo test -p domovoi-scanner --test smoke_apfs_sys
```
- REAL-DEP-SMOKE: настоящий `BulkListing` из apfs-sys на temp-дереве (exit 0).
```
cargo clippy -p domovoi-scanner --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).
```
cargo clippy -p domovoi-testkit --all-targets -- -D warnings
```
- Typecheck-критерий testkit (exit 0).

### Phase 5: index — дерево, агрегаты, давность, числа тома, кэш скана

<!-- deps: [scanner] — потребление: индекс строится из записей сканера (smoke_scanner). -->

- [ ] Дерево узлов (арены, интернирование имён); агрегаты: `allocated` с дедупом по потоку `(REALDEVID, CLONEID)`
  и каноническим элементом по Knowledge [идентичность], бейджи «жёсткая ссылка» (FILEID равен) / «клон» и «делит
  блоки с N»; `files`, `newest_modified`, `newest_opened`, `added_at` (`ATTR_CMN_ADDEDTIME` — «появился здесь»,
  для hard links своя дата у каждой записи); категории (Приложения, Разработка, Медиа, Документы, Кэши, Система,
  Скрытое, Модель, Дом) по путям и UTI.
- [ ] `reclaimable(node)` по Knowledge [идентичность] с раздельным удержанием «клон» / «снапшот» и перечитыванием
  `private_size` в момент показа.
- [ ] Давность: xattr-канал сканера плюс один синхронный `MDQuery kMDItemLastUsedDate == "*"`
  (`kMDQueryScopeComputer`, модуль `ffi.rs` над `objc2-core-services`, ≈ 0,2 с на ~1 тыс. результатов) по
  завершении скана для каталогов и бандлов, нормализация firmlink-формы; `last_opened` с источником. Факт:
  per-item `MDItemCopyAttribute` подставляет mtime для FS-only записей и стоит минуты на миллион файлов — не
  используется; `atime` на APFS обновляется только если старше mtime — не сигнал.
- [ ] Принадлежность приложению: bundle id ↔ `.app` через `MDQuery kMDItemCFBundleIdentifier` и
  `NSWorkspace::URLForApplicationWithBundleIdentifier`; контейнер без приложения — признак «остаток удалённого
  приложения» (данные для гипотез модели); давность данных приложения = `last_opened` бандла-владельца.
- [ ] `Volumes`: `SPACEUSED` Data и `/` («macOS», без обхода — sealed read-only), VM / Preboot / Update — «Другие
  тома», тома других контейнеров в круг не входят; свободно = контейнерное `SPACEAVAIL`; purgeable-аннотация;
  снапшоты списком без размера; «Скрытое» = `SPACEUSED(Data) − Σ видимого` с подписанными слагаемыми (скрыто
  правами — счётчик папок, снапшоты — число и даты, накладные расходы); отрицательное — «расхождение учёта».
- [ ] Запросы с фильтрами (категория, уровень, минимальный размер, давность, префикс пути) — одна функция для UI и
  инструментов агента; источник каждой цифры (`statfs` / `getattrlist` / оценка macOS) в модели для тултипов;
  тип события `ModelEvent` (модель скачана / удалена) для категории «Модель».
- [ ] Кэш скана (rkyv 0.8): заголовок magic / версия формата / Volume UUID / FSEvents UUID / `sw_vers` build / версия
  приложения; `mmap_readonly` + `access` с bytecheck; атомарная замена (tmp + rename) в
  `~/Library/Caches/com.domovoi.app/`; несовпадение заголовка → полный пересчёт; применение дельт FSEvents пересчётом
  поддеревьев.
- [ ] Бюджет памяти: тест `size_of::<Node>() × 5 650 000 ≤ 900 МБ`; открытие кэша фикстуры на 1 млн узлов ≤ 1 с.
- [ ] Тесты (`crates/index/tests/`): `aggregates.rs` — канон группы, бейджи, категории, фикстура `pnpm_store` (канон —
  путь в store, у клонов виртуальный 0 и «делит блоки с 3»), Σ `allocated` одинакова при обходе в один и N потоков и
  при перестановке детей; `reclaimable.rs` — decmpfs-фикстура (ALLOCSIZE), сжатый файл + его `cp -c` (0),
  клон-фикстура (PRIVATESIZE); ветка «снапшот» — unit-тест с подставленными атрибутами; `last_used.rs` — xattr и
  MDQuery-канал, источник `Inherited` у копии; `apps.rs` — `Library/Containers/com.fake.app` без `.app` → «остаток
  удалённого приложения»; `volumes.rs` — формула «Скрытого» и источники цифр; `smoke_scanner.rs` — настоящий сканер
  на temp-дереве → индекс, Σ по иноду == `du -sk`; `cache_roundtrip.rs` — атомарная запись, mmap, отказ при чужом
  заголовке, применение дельты.

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-index
```
- Агрегаты, канон, `reclaimable`, давность, категории, числа тома, запросы (exit 0).
```
cargo test -p domovoi-index --test smoke_scanner
```
- REAL-DEP-SMOKE: настоящий сканер на temp-дереве → индекс, Σ по иноду == du -sk (exit 0).
```
cargo test -p domovoi-index --test cache_roundtrip
```
- Кэш пишется атомарно, открывается через mmap, отвергается при чужом заголовке (exit 0).
```
cargo clippy -p domovoi-index --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).

### Phase 6: rules — правила, denylist, рекомендации, Корзина Домового

<!-- deps: [index] — потребление: правила ходят по индексу и его запросам (smoke_index). -->

- [ ] Модель правила: `id` в схеме `<домен>.<что>`, шаблоны путей (`**`, «на любой глубине» с маркером проекта
  `Cargo.toml` / `package.json` / `build.zig`), уровень (Безопасно / Восстановимо / Проверьте сами), пояснение
  голосом раздела VIII, bundle id приложения-владельца («закройте приложение»), признак «инфо» (root-пути: только
  команда вендора); сопоставление шаблонов — по сегментам, без учёта регистра на томах без
  `VOL_CAP_FMT_CASE_SENSITIVE`, с нормализацией NFC обеих сторон.
- [ ] Таблица правил: десять стартовых ids документа сохраняются дословно (`xcode.derived-data`,
  `xcode.device-support`, `cargo.target`, `dev.deps`, `caches.user`, `downloads.installers`, `xcode.simulators`,
  `docker.vm`, `ios.backups`, `xcode.archives`) с уровнями раздела 4.4 после правки фазы workspace; новые из
  разведки: кэши homebrew, npm, pnpm (store / metadata), yarn classic / berry, bun, cargo registry / git, pip, uv
  (`~/.cache/uv`), poetry (cache / venvs), gradle, maven, cocoapods (cache / specs), swiftpm, go (build / mod),
  composer, nuget (http / packages), carthage; Xcode (CoreSimulator `unavailable` — Безопасно, runtimes-инфо, dyld,
  caches); `orbstack`; `cache.chrome`, `cache.slack` (сборка с сайта); `cache.app.generic`; пути только дефолтные плюс
  чтение конфигов (`~/.config/pnpm/rc`, `~/.cargo/config.toml` `target-dir`) без запуска бинарников — GUI не видит
  экспортов shell, пояснение называет это ограничение. Не включаются до фикстуры с реальной установкой: Spotify и
  контейнерный путь Slack из App Store (confidence low/medium).
- [ ] Denylist по фактам, не по префиксам: `SF_RESTRICTED` по флагу элемента (есть и на Data-томе: Safari.app,
  /Library/Apple); REALDEVID системного тома; `SF_NOUNLINK` / `UF_IMMUTABLE` / `SF_IMMUTABLE` / `UF_APPEND` /
  `SF_APPEND` / `UF_DATAVAULT` / `SF_FIRMLINK` / MNTPOINT; родитель без W_OK (`ATTR_CMN_USERACCESS`); Keychains;
  Preferences (включая контейнерные); контейнеры Apple (`com.apple.*`, `group.com.apple.*`, `*.com.apple.*`);
  iCloud Drive и CloudStorage; TCC-данные (Mail, Messages, Safari, Cookies, HomeKit, Suggestions, CoreSpotlight,
  Accounts, AddressBook, Calendars, `*.photoslibrary`, Music); корневые бандлы запущенных процессов
  (`running_bundle_roots`; `NSWorkspace.runningApplications` не видит helper-ы и годится только для имён и иконок;
  обновление перед показом и действием); собственный бандл и данные; `~/.Trash`; служебные каталоги тома. Другие
  тома — вне охвата скана, но Корзина Домового работает на любом томе с `.Trashes/<uid>`.
- [ ] Рекомендации: карточки по правилам с суммой `reclaimable` и объяснением удержания, итог по уровням, предвыбор
  только «Безопасно»; гипотезы модели — отдельный вход без уровня, всегда «Проверьте сами».
- [ ] Корзина Домового: список выбранного с итогом Σ `reclaimable`; `trashItemAtURL_resultingItemURL_error` на
  элемент верхнего уровня, ошибки по элементу (513 / 642 / 4 / 3328 / 640) без прерывания пакета; журнал возврата
  (исходный путь в обеих формах, `resultingItemURL` как есть, LINKID, ALLOCSIZE, флаги, CLONEID, правило, время) в
  Application Support; «Вернуть» через `moveItemAtURL_toURL` после проверки LINKID и родителя, имя при коллизии по
  правилу Finder; «освободится после очистки Корзины» пересчитывается при показе; «Открыть Корзину».
- [ ] Тесты (`crates/rules/tests/`): `rules_table.rs` — фикстура на каждый `id` (совпадение путей, уровень, пояснение
  без «!»), регистронезависимое совпадение `~/library/caches/homebrew` на Data-томе и несовпадение на
  case-sensitive образе, имя в NFD против шаблона в NFC, `~/.cargo/config.toml` с `target-dir` и
  `~/.config/pnpm/rc` с `store-dir` → переопределённый путь найден; `doc_parity.rs` — таблица раздела 4.4
  документа парсится и совпадает по ids и уровням с таблицей правил; `denylist.rs` — temp-дерево с флагами (`chflags
  uchg` и восстановление), запущенный бандл — дочерний процесс из каталога `*.app` во временной папке;
  `smoke_index.rs` — настоящий индекс temp-дерева → рекомендации с итогом по уровням; `tray_trash.rs` — перемещение
  файлов temp-дерева в Корзину (FILEID до и после совпадает, дерево из 10 000 файлов ≤ 1 с), возврат по журналу с
  уборкой, read-only образ → 642 по элементу без прерывания пакета.

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-rules
```
- Правила, denylist и рекомендации по фикстурам (exit 0).
```
cargo test -p domovoi-rules --test doc_parity
```
- Ids и уровни стартовых правил совпадают с разделом 4.4 документа (exit 0).
```
cargo test -p domovoi-rules --test smoke_index
```
- REAL-DEP-SMOKE: настоящий индекс temp-дерева → рекомендации с итогом по уровням (exit 0).
```
cargo test -p domovoi-rules --test tray_trash
```
- Корзина Домового перемещает через `trashItem`, журнал возвращает элемент на место (exit 0).
```
cargo clippy -p domovoi-rules --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).

### Phase 7: dups — конвейер дубликатов и замена клоном

<!-- deps: [rules] — потребление: действия над группой идут через Корзину Домового, запросы — через индекс (smoke_rules). -->

- [ ] Конвейер: кандидаты по размеру (порог по умолчанию 1 МиБ, настраиваемо) → хэш первых и последних 64 КиБ →
  полный BLAKE3 (`update_mmap_rayon` ≥ 128 КиБ, `set_nocache`, `set_thread_disk_policy_utility` — иначе проход
  вытесняет page cache; полный хэш только до порога 4 ГиБ, настраиваемо) → группы; прогресс и отмена; `SF_DATALESS`
  не хэшируется, EDEADLK = пропуск; `~/Library/Mobile Documents` и `~/Library/CloudStorage` — отдельная поздняя стадия
  с лимитом времени.
- [ ] Группа: члены с равным CLONEID или FILEID помечаются «уже делят блоки, выигрыша нет» (disabled); «оставить» =
  канонический элемент; выигрыш = Σ `reclaimable` копий.
- [ ] Действие по умолчанию «в Корзину» через Корзину Домового.
- [ ] `clone_replace` (процедура Hyperspace при выключенном «Free storage space immediately»): предусловия —
  регулярные файлы, один REALDEVID, `VOL_CAP_INT_CLONE` и `RENAME_SWAP` (`exchangedata` на APFS недоступен),
  `st_nlink == 1` у копии (иначе блоки держат другие ссылки), `st_uid == getuid()`, W_OK на файл и каталог, flags без
  `UF_IMMUTABLE | UF_APPEND | UF_DATAVAULT | SF_IMMUTABLE | SF_APPEND | SF_RESTRICTED | SF_NOUNLINK | SF_FIRMLINK |
  SF_DATALESS`, без `EF_IS_PURGEABLE`, busy-бит (`com.apple.FinderInfo` байты 24–25 & 0x0080) снят, не denylist,
  ни один предок не пакет (`isFilePackageAtPath`), не sync root (`EF_IS_SYNC_ROOT`, Mobile Documents,
  CloudStorage), `pids_with_open` пуст (UI: видны только свои процессы), CLONEID различаются.
- [ ] Машина состояний: `clone_at(оригинал → .domovoi-tmp-<uuid> в каталоге копии, CLONE_NOFOLLOW)` →
  `copy_metadata(копия → клон)` (даты, xattr quarantine / macl / теги без разрушения общих блоков) → JIT-валидация
  (`lstat` dev / ino / size / mtime, `gen_count`, побайтовое сравнение) → `rename_swap` внутри `NSFileCoordinator` →
  старое тело в Корзину Домового с исходным путём → откат mtime каталога; журнал незавершённых операций; уборка
  `.domovoi-tmp-*` при старте; предупреждение при < 5 % свободного места (ENOSPC при перезаписи общего блока).
- [ ] Тесты (`crates/dups/tests/`): `pipeline.rs` — группы и выигрыш на temp-дереве, порог полного хэша, поток
  хэширования имеет `thread_disk_policy() == IOPOL_UTILITY`; `smoke_rules.rs` — настоящие rules (Корзина Домового) и
  индекс на temp-дереве → группы и действие «в Корзину»; `clone_replace_image.rs` — на образе: пары с hard link
  (отказ), уже-клоны (нет выигрыша), sparse, busy-бит (отказ), открытый fd в дочернем процессе (отказ), успех
  (PRIVATESIZE копии = 0, старое тело в `.Trashes/<uid>` образа, журнал), уборка stranded.

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-dups
```
- Конвейер группирует и считает выигрыш по фикстурам (exit 0).
```
cargo test -p domovoi-dups --test smoke_rules
```
- REAL-DEP-SMOKE: настоящие rules и index на temp-дереве → группы и действие «в Корзину» (exit 0).
```
cargo test -p domovoi-dups --test clone_replace_image
```
- Замена клоном на APFS-образе: предусловия, атомарный swap, обратимость через Корзину (exit 0).
```
cargo clippy -p domovoi-dups --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).

### Phase 8: grammar — схемы ответа, конвертер JSON-схема→GBNF, шаблон ChatML

<!-- deps: [dups] — order-only (чистый крейт без потребления). -->

- [ ] Схемы: ответ модели — `{"kind":"tool_call","name":<enum>,"arguments":{…}}` либо
  `{"kind":"final","answer":<string>,"hypotheses":[{path, guess, confidence}]}`; инструменты `query_index`
  (фильтры индекса), `inspect_node(path)`, `rule_explain(id)`, `summarize()`; `additionalProperties: false`;
  вариант грамматики «только `tool_call`» для первого шага сценариев e2e.
- [ ] Конвертер JSON-схема → GBNF (конвертер llama.cpp живёт в C++ `common/`, публичный API принимает только GBNF)
  для безопасного подмножества: object / properties / required, string с `maxLength` < 2000 и `pattern` только
  `^…$`, integer / number с minimum / maximum, boolean, enum, array с minItems / maxItems, `$ref` только верхнего
  уровня; `anyOf` / `oneOf` рядом с `properties`, `patternProperties`, вложенные `$ref` — ошибка компиляции; правило
  `space` без пробельных цепочек; повторения `{0,N}`.
- [ ] Шаблон ChatML (`<|im_start|>role` … `<|im_end|>`, префикс `<think>\n\n</think>\n\n` у ассистента, инструменты
  и схема ответа с примером в system) с параметрами записи каталога; семейство — поле записи.
- [ ] Паритет: golden-промпты на каждую запись каталога `assets/chat-templates/<repo>@<rev>.golden.txt`, сгенерированные
  скриптом `research/render_templates.py` (jinja2 по официальному `chat_template.jinja` ревизии, `enable_thinking=false`,
  без `tools`) на фиксированных диалогах и закоммиченные; тест сравнивает Rust-шаблон побайтно с golden; смена
  ревизии = перегенерация golden.
- [ ] Тесты (`crates/grammar/tests/`): `converter.rs` — golden-грамматики `tests/golden/*.gbnf`, запрещённые
  конструкции отвергаются; `response.rs` — парсер ответа отвергает любое отклонение от схемы;
  `template_parity.rs` — побайтное равенство с golden-промптами.

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-grammar
```
- Конвертер даёт golden-грамматики, запрещённые конструкции отвергаются, парсер ответа строг (exit 0).
```
cargo test -p domovoi-grammar --test template_parity
```
- Rust-шаблон побайтно равен golden-промптам всех записей каталога (exit 0).
```
cargo clippy -p domovoi-grammar --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).

### Phase 9: llama-sys — статическая сборка llama.cpp на пине, биндинги, тестовые модели

<!-- deps: [grammar] — order-only (vocab_grammar использует собственные inline-GBNF, golden grammar не читает). -->

- [ ] Submodule `vendor/llama.cpp` на пине из Stack; `build.rs`: `cmake::Config` (`BUILD_SHARED_LIBS=OFF`,
  `GGML_METAL=ON`, `GGML_METAL_EMBED_LIBRARY=OFF`, `GGML_NATIVE=OFF` для воспроизводимости между машинами,
  `GGML_OPENMP=OFF`, `GGML_BLAS=ON` / `GGML_ACCELERATE=ON`, `LLAMA_BUILD_COMMON/APP/TOOLS/TESTS/EXAMPLES/SERVER=OFF` —
  `APP=OFF` обязателен при `TOOLS=OFF`, `LLAMA_CURL=OFF`); фичи `metal` (default) и `cpu-only` (`GGML_METAL=OFF`);
  линковка Foundation, Metal, MetalKit, Accelerate, c++; `CMAKE_*_COMPILER_LAUNCHER` из окружения (sccache).
- [ ] Предкомпиляция `default.metallib` и `ggml-tensor.metallib` (`xcrun -sdk macosx metal` / `metallib`) в OUT_DIR и
  экспорт пути для xtask; `GGML_METAL_PATH_RESOURCES` в тестах; замер холодной компиляции embedded-шейдеров со
  сброшенным кэшем (`#[ignore]`, nightly) — число фиксируется следующей правкой спеки.
- [ ] bindgen по `include/llama.h` и `ggml/include` (allowlist `llama_.*|ggml_.*|gguf_.*`, `allowlist_recursively
  false`, `derive_partialeq`, `prepend_enum_name false`); тест раскладки `llama_model_params` /
  `llama_context_params` на пин (`size_of`, поля `load_mode` — вместо `use_mmap` / `use_mlock` на этом пине —
  `devices`, `abort_callback`); `llama_model_load_from_file_ptr` в allowlist.
- [ ] Тестовые модели: `assets/test/tiny-random.gguf` (115 КБ; генератор `research/tiny_gguf.py`, SHA-256 в
  манифесте) и `stories260K.gguf` через `xtask fetch-test-models` (SHA-256, `target/test-models/`).
- [ ] Прогрев гейта: `cargo build -p domovoi-llama-sys` выполняется задачей до гейта (холодная cmake-сборка не
  обязана укладываться в 300 с; гейт инкрементален).
- [ ] Тесты (`crates/llama-sys/tests/`): `layout.rs` — раскладка структур; `vocab_grammar.rs` — `vocab_only` на обеих
  моделях; `llama_sampler_init_grammar` на собственных inline-GBNF (минимальная JSON-грамматика; NULL на заведомо
  битой); `llama_sampler_apply` на синтетических кандидатах: отвергнутые токены = −INF, EOG только при пустом стеке;
  окружение `GGML_METAL_DEVICES=0`.

**Done when (EXECUTABLE):**
```
cargo build -p domovoi-llama-sys
```
- llama.cpp собран статически на пине с предкомпилированными metallib (инкрементально, exit 0).
```
cargo test -p domovoi-llama-sys --test vocab_grammar
```
- Биндинги и сэмплер грамматики работают на словаре без forward-pass (exit 0).
```
cargo clippy -p domovoi-llama-sys --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).

### Phase 10: xpc — биндинги libxpc и протокол инференса

<!-- deps: [llama-sys] — order-only. -->

- [ ] `domovoi-xpc-sys`: bindgen над `<xpc/xpc.h>`, `<xpc/listener.h>`, `<xpc/session.h>`, `<dispatch/dispatch.h>`
  (`-fblocks`, без `--generate-block`), блоки через `block2::RcBlock`; обёртки продакшен-пути: сервер
  `xpc_listener_create` + `xpc_listener_set_peer_code_signing_requirement` (Team ID; фича `adhoc` отключает), клиент
  `xpc_session_create_xpc_service`, `xpc_fd_create`; обработчик сессии обязан вызвать
  `set_incoming_message_handler` или `cancel` (иначе API misuse crash). Обёртки тестового пути — connection-API:
  `xpc_connection_create(NULL)`, `xpc_endpoint_create`, `xpc_connection_create_from_endpoint`.
- [ ] `domovoi-inference-proto`: версионированные сообщения (handshake с pid и версией протокола; `load(fd, класс,
  n_ctx)`; `unload`; `generate(prompt, grammar, sampling, n_predict)`; `token`; `done(stats)`; `cancel`; `error`) ↔
  `xpc_dictionary` с round-trip тестами; таймауты из окружения.
- [ ] Тесты (`crates/xpc-sys/tests/`, `crates/inference-proto/tests/`): `anonymous.rs` — echo через анонимное
  соединение connection-API, `XPC_ERROR_CONNECTION_INTERRUPTED` после cancel, передача fd временного файла;
  `roundtrip.rs` (proto) — все сообщения без потерь, отказ на неизвестной версии.

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-xpc-sys
```
- Сообщения протокола проходят через анонимное соединение connection-API в одном процессе (exit 0).
```
cargo test -p domovoi-inference-proto
```
- Сообщения сериализуются и десериализуются без потерь, версии проверяются (exit 0).
```
cargo clippy -p domovoi-xpc-sys --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).
```
cargo clippy -p domovoi-inference-proto --all-targets -- -D warnings
```
- Typecheck-критерий proto (exit 0).

### Phase 11: inference — XPC-сервис с llama.cpp, фейковый бэкенд, песочница без сети

<!-- deps: [xpc] — потребление: сервис реализует proto и xpc-sys (xpc_bundle), биндинги llama-sys (tiny_gguf), грамматику grammar (smoke_grammar). -->

- [ ] Трейт `InferenceBackend` (load / unload / generate-stream / cancel) с `LlamaBackend` и `FakeBackend`
  (скриптованные потоки токенов, ошибки, задержки, зависание); `LlamaBackend`: модель из fd через `fdopen` →
  `llama_model_load_from_file_ptr`, `load_mode = MMAP`, `devices` по профилю, `n_ctx` по классу (8k / 16k / 32k), KV
  q8_0, грамматика `llama_sampler_init_grammar` с проверкой NULL, стрим токенов, отмена (`abort_callback` на CPU;
  Metal — по Knowledge [инференс]), `progress_callback` для отмены загрузки, таймер выгрузки (сразу / 5 / 15 / 60 /
  без таймера), `DISPATCH_SOURCE_TYPE_MEMORYPRESSURE` WARN → выгрузка, CRITICAL → аварийная выгрузка и отказ от
  загрузки до спада.
- [ ] Бинарник `domovoi-inference`: listener, QoS `USER_INITIATED` для генерации, `set_vfs_policies` первой строкой,
  режим `--backend fake` и тестовый режим с анонимным endpoint; `Info.plist` (`CFBundleIdentifier
  com.domovoi.inference`, `CFBundlePackageType` `XPC!`, `XPCService{ServiceType Application, RunLoopType
  dispatch_main}`, `EnvironmentVariables` для тестов — таймауты, `GGML_METAL_DEVICES`); entitlements
  `com.apple.security.app-sandbox` без `network.client`.
- [ ] Таблица памяти по классам (веса + KV при `n_ctx` + compute) против `recommendedMaxWorkingSetSize`. Факт:
  llama.cpp на hosted-раннерах не гоняет тесты с меткой `model` — отмену загрузки и выгрузку DOMOVOI проверяет сам.
- [ ] Тесты (`crates/inference/tests/`): `tiny_gguf.rs` (CPU, пустой `devices`, `GGML_METAL_DEVICES=0`) — загрузка из fd
  / выгрузка, отмена загрузки по `progress_callback` (> 0,5 → model == NULL), `abort_callback` во время decode, стрим,
  детерминизм при temp 0, валидный JSON под грамматикой при `n_predict ≥ 256`; `smoke_grammar.rs` — грамматика крейта
  grammar → сэмплер → валидный ответ по схеме; `xpc_bundle.rs` — временный `TestHost.app` с
  `Contents/XPCServices/com.domovoi.inference.xpc` (ad-hoc подпись с entitlements песочницы), запуск хоста из бандла,
  on-demand старт launchd, round-trip с `FakeBackend`, падение сервиса → переподключение, выгрузка по простою с
  таймаутом из plist (секунды); `sandbox_no_network.rs` — из сервиса в песочнице попытка TCP-соединения завершается
  ошибкой, чтение весов через переданный fd работает; `#[ignore]` nightly: `memory_budget.rs` (на 8 ГБ при открытом
  индексе 2 млн узлов загрузка не доводит до CRITICAL), `metal_cancel_latency.rs` (≤ 300 мс при `n_batch ≤ 256`),
  `tpot_in_service.rs` (разница с in-process ≤ 5 %), `metal_in_sandbox.rs` (tiny-GGUF с `devices=[MTL0]` из сервиса в
  песочнице).

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-inference --test tiny_gguf
```
- Биндинги на крошечной GGUF без GPU: загрузка из fd, отмена, стрим, грамматика (exit 0).
```
cargo test -p domovoi-inference --test smoke_grammar
```
- REAL-DEP-SMOKE: грамматика крейта grammar → сэмплер → валидный ответ по схеме (exit 0).
```
cargo test -p domovoi-inference --test xpc_bundle
```
- Настоящий XPC-сервис из временного бандла с entitlements: запуск, протокол, падение, простой (exit 0).
```
cargo test -p domovoi-inference --test sandbox_no_network
```
- Решение 9 обеспечено песочницей: сети у сервиса нет (exit 0).
```
cargo clippy -p domovoi-inference --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).

### Phase 12: downloader — каталог моделей и загрузчик с докачкой

<!-- deps: [inference] — order-only; потребление grammar (семейство записей) покрыто smoke_grammar, index (ModelEvent) — тестом catalog. -->

- [ ] `assets/catalog.json` и типы: запись (id, класс, семейство, repo, revision, file, size, sha256, license SPDX и
  текст LICENSE / NOTICE, min_memory_bytes, n_ctx, primary); состав по Knowledge [каталог]; отклонены и не
  включаются: по лицензии — YandexGPT-5-Lite (проприетарная, порог 10 млн токенов/мес), Gemma 1–3 (Terms of Use
  требуют EULA-слой), Llama 3.x / 4, xLAM (CC-BY-NC), EXAONE, LFM; по языку — Granite, OLMo, Nanbeige,
  Hermes-4-14B; по семейству шаблона — Phi-4-mini, Gemma 4, Mistral Nemo / Ministral.
- [ ] Модель по умолчанию по `hw.memsize` (< 16 ГБ → компактная, < 32 → стандартная, иначе расширенная) с проверкой
  `recommendedMaxWorkingSetSize`.
- [ ] Загрузчик на `NSURLSession` (objc2-foundation, делегат через `define_class!`): resolve
  `https://huggingface.co/<repo>/resolve/<revision>/<file>` → 302 на подписанный CDN-URL с истечением ≈ 1 ч → 206 с
  `accept-ranges: bytes`, `etag` = SHA-256 файла, `x-repo-commit` = ревизия; Range-докачка (`If-Range` по ETag),
  re-resolve при 403 или истечении, пауза / отмена / возобновление с диска, предварительная проверка места (размер
  + 5 %), потоковый SHA-256, атомарное переименование по завершении; источник настраивается (зеркало с тем же
  путём); файл, положенный вручную, принимается по хэшу; test-only записи каталога для e2e.
- [ ] Хранилище `~/Library/Application Support/Domovoi/models`: папка попадает в Time Machine — исключение
  `CSBackupSetItemExcluded` при создании; из Spotlight не исключается (у GGUF нет импортёра, индексируются только
  имена); детекция «осиротевших» файлов (нет в каталоге или хэш не совпал) с предложением удалить; эмиссия
  `ModelEvent` индекса после загрузки и удаления.
- [ ] Тесты (`crates/downloader/tests/`): `download.rs` — локальный `tiny_http`: 302 с истекающим токеном → 403 →
  re-resolve, Range и `If-Range`, пауза и возобновление, несовпадение SHA-256 → отказ, нехватка места → отказ до
  загрузки; `catalog.rs` — лицензии ∈ {Apache-2.0, MIT}, семейство только ChatML, размеры и SHA-256 заполнены, ровно
  один primary на класс, `ModelEvent` эмитится; `storage.rs` — `CSBackupIsItemExcluded` → true после создания
  каталога во временном месте, осиротевшие файлы (чужой хэш, не из каталога) помечаются, валидный — нет;
  `smoke_grammar.rs` — семейство каждой записи разрешается в шаблон grammar.

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-downloader
```
- Машина состояний загрузки и хранилище проходят все ветви на локальном сервере (exit 0).
```
cargo test -p domovoi-downloader --test catalog
```
- Каталог валиден: лицензии, семейство, хэши, ревизии, один primary на класс (exit 0).
```
cargo test -p domovoi-downloader --test smoke_grammar
```
- REAL-DEP-SMOKE: записи каталога разрешаются в шаблон grammar (exit 0).
```
cargo clippy -p domovoi-downloader --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).

### Phase 13: agent — агентный цикл, инструменты, защита от инъекций, эвал

<!-- deps: [downloader] — order-only; потребление index, rules, grammar, inference (LlamaBackend) покрыто smoke_real_modules. -->

- [ ] Агентный цикл над `InferenceBackend`: system-промпт с инструментами, схемой ответа и примером; контекст —
  агрегаты и результаты инструментов с бюджетом токенов; до N шагов; инструменты read-only над индексом и
  правилами; результаты и имена файлов — недоверенные данные.
- [ ] Инструменты `query_index`, `inspect_node`, `rule_explain`, `summarize` через запросы индекса; кнопки действий
  («Добавить в Корзину · X») формируются приложением по результатам инструментов, не по тексту.
- [ ] Задачи: классификация неизвестных каталогов → гипотезы «Похоже на…» с пометкой (вход для рекомендаций, всегда
  «Проверьте сами»); ответы на вопросы; сводка «сколько можно освободить и почему».
- [ ] Защита: схема без свободных полей там, где не нужно; вывод модели — недоверенный текст без ссылок и действий.
- [ ] `domovoi-eval`: ≥ 60 русскоязычных задач на фикстурном индексе; метрики — точность инструмента, валидность
  схемы, доля вызовов инструмента при явной необходимости, качество объяснения по шкале раздела VIII (LLM-judge),
  TPOT с грамматикой и без; пороги primary класса: точность инструмента ≥ 0,9, валидность 1,0, доля вызовов ≥ 0,9,
  объяснение ≥ 4/5, TPOT с грамматикой ≤ 2× без; `cargo xtask eval --class <класс>` (`#[ignore]`, nightly); первая
  ночная выгрузка фиксирует значения следующей правкой спеки.
- [ ] Тесты (`crates/agent/tests/`, `crates/eval/`): `loop.rs` — цикл, инструменты и формирование действий на
  FakeBackend; `smoke_real_modules.rs` — настоящие индекс, правила, грамматика и `LlamaBackend` на
  `assets/test/tiny-random.gguf` (CPU: пустой `devices`, `GGML_METAL_DEVICES=0`, `n_predict ≥ 256`) на фикстуре → ответ
  валиден по схеме, цикл завершается за ≤ N шагов; `injection.rs` — враждебные имена каталогов не меняют выбор
  инструмента и `rule_explain`; eval компилируется.

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-agent
```
- Цикл, инструменты и формирование действий на FakeBackend (exit 0).
```
cargo test -p domovoi-agent --test smoke_real_modules
```
- REAL-DEP-SMOKE: настоящие индекс, правила, грамматика и LlamaBackend на tiny-GGUF → ответ по схеме (exit 0).
```
cargo test -p domovoi-agent --test injection
```
- Враждебные имена не меняют выбор инструмента и пояснение правила (exit 0).
```
cargo test -p domovoi-eval --no-run
```
- Эвал-набор и метрики компилируются (exit 0); запуск — ночной на self-hosted.
```
cargo clippy -p domovoi-agent --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).
```
cargo clippy -p domovoi-eval --all-targets -- -D warnings
```
- Typecheck-критерий eval (exit 0).

### Phase 14: render — Metal-диаграмма: два режима, анимации, растровый текст, доступность

<!-- deps: [agent] — order-only; потребление theme и index покрыто smoke_theme_index и color_parity. -->

- [ ] `MTKView` (objc2-metal-kit, `define_class!` делегат `MainThreadOnly`) по запросу, до 120 Гц во время анимаций;
  `CAMetalLayer.colorspace` sRGB, drawable `bgra8Unorm_srgb`, premultiplied-альфа; собственные шейдеры компилируются
  в `.metallib` в xtask и грузятся `newLibraryWithURL_error`.
- [ ] Модель диаграммы: общий фокус, наведение, hit-test (полярный для sunburst, дерево прямоугольников для treemap),
  LOD (4 кольца / плитки ≥ 2 px); инстансы SDF-дуги и прямоугольника, один draw call на слой; фикстура
  `tests/fixtures/book_v.json` — дерево ROOT раздела V книги.
- [ ] Layout: sunburst (доли по размеру, зазоры по размеру, «Скрытое» пунктиром) и squarified treemap (рамки групп,
  вложенные плитки; пороги подписей книги: плитка 58×34, заголовок группы 50×46); сектор скана по фактическому
  прогрессу.
- [ ] Анимации: радиальный зум 220 мс (интерполяция углов и радиусов на GPU), приглушение 120 мс до 45 %, сжатие к
  центру 320 мс; `reduce_motion` → смена кадра.
- [ ] Текст: растровый атлас глифов — шейпинг и фолбэк через Core Text (`CTLine` → `CTRun` glyphs / positions / font:
  китайские имена получают системный каскад сами), растеризация в `CGBitmapContext` alpha-only 8 bpc на точный
  device-размер (subpixel positioning on, Apple quantization off, 4 субпиксельных варианта, паддинг +1 px,
  утолщение по `AppleFontSmoothing` для паритета с `NSTextField`), блендинг текста `linear-corrected` в фрагментном
  шейдере, атлас R8Unorm 2048² с ленивой дорисовкой; центр солнца — подгонка по хорде (уменьшение кегля до минимума,
  затем `CTLineCreateTruncatedLine` End — канон Core Text, не демо `truncMid` книги); подписи treemap только в плитках
  выше порога, обрезка в конце; при зуме — масштабирование битмапа до финального кадра (как Core Animation).
- [ ] Доступность: `NSAccessibilityElement` на сегмент уровня (frame, label «имя, размер, категория») и центр;
  клавиатурная навигация по сегментам стрелками.
- [ ] Offscreen-рендер в `MTLTexture` → PNG (масштаб 1.0 и 2.0); golden-тест пишет
  `target/snapshots/render/{sunburst,treemap}-{dark,light}@{1,2}x.png` на book_v.
- [ ] `benches/frame` (ignored, nightly): на фикстурах 1 млн и 5,65 млн узлов — время раскладки sunburst и squarify,
  число инстансов на кадр при глубине 3–4 с LOD, GPU-время кадра по `GPUStartTime` / `GPUEndTime` командного буфера,
  доля площади treemap с подписью; пороги фиксируются следующей правкой спеки.
- [ ] Тесты (`crates/render/tests/`): `layout.rs` — сумма углов 2π, плитки squarify без перекрытий, детерминизм,
  hit-test обратен layout, кольцо = глубина узла, подпись treemap только при плитке ≥ порога и усечена End, центр:
  уменьшение кегля до минимума, затем `…` — на строках TP / TN / TN3 книги при ширинах 140–560 px;
  `golden_offscreen.rs` — рендер book_v при 1.0 и 2.0 в пределах порога (точное сравнение при
  `DOMOVOI_GOLDEN_EXACT=1`), текст совпадает с `CTLineDraw`; `color_parity.rs` — квад с токеном `wax` / `c_apps` в
  `MTLTexture` и `NSView` с `ns_color()` того же токена через `bitmapImageRepForCachingDisplayInRect` дают равные
  пиксели в обеих темах, `*-soft` — премультипликация совпадает; `motion_budget.rs` — 220 / 120 / 320 / 1400 мс и
  reduce motion; `accessibility.rs` — число `accessibilityChildren` = видимые сегменты + центр, label и frame
  совпадают с hit-test, стрелки перемещают фокус; `smoke_theme_index.rs` — настоящая палитра theme и настоящий
  индекс фикстуры → кадр.

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-render --test layout
```
- Раскладка, hit-test, пороги подписей и подгонка центра соответствуют разделам V и X (exit 0).
```
cargo test -p domovoi-render --test golden_offscreen
```
- Рендер book_v при 1.0 и 2.0 совпадает с эталоном в пределах порога; текст совпадает с `CTLineDraw` (exit 0).
```
cargo test -p domovoi-render --test color_parity
```
- Токен в Metal и в NSView даёт равные пиксели в обеих темах (exit 0).
```
cargo test -p domovoi-render --test motion_budget
```
- Длительности 220 / 120 / 320 / 1400 мс и режим reduce motion закреплены (exit 0).
```
cargo test -p domovoi-render --test accessibility
```
- Дерево доступности диаграммы полно и навигируемо с клавиатуры (exit 0).
```
cargo test -p domovoi-render --test smoke_theme_index
```
- REAL-DEP-SMOKE: настоящая палитра theme и настоящий индекс фикстуры → кадр (exit 0).
```
cargo clippy -p domovoi-render --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).
**Done when (AGENT-JUDGED):** ревьюер читает PNG `target/snapshots/render/{sunburst,treemap}-{dark,light}@{1,2}x.png` (файлы на диске, не diff) и сверяет с разделом V книги: пропорции и зазоры сегментов, пунктир «Скрытого», палитра категорий обеих тем; читаемость подписей 11–13 pt при 1.0 без размытия рядом с эталоном `CTLineDraw`. Обоснование agent-judged: визуальное соответствие книге машиной не проверяется.

### Phase 15: ui-shell — приложение, сайдбар, экраны 01–03, Session, headless-режимы

<!-- deps: [render] — потребление: диаграмма из render, индекс и сканер в Session (smoke_render_scanner), палитра theme (theme_pixels). -->

- [ ] `domovoi` (objc2-app-kit): `NSApplication` и делегат, окно, `NSSplitViewController`, сайдбар source list на
  `NSOutlineView` (Диск: Macintosh HD; Порядок: Рекомендации, Дубликаты, Домовой; Настройки: Модель; индикатор
  «свободно X из Y»), `NSAppearance` → палитра и динамические цвета, `viewDidChangeEffectiveAppearance`; при
  `accessibilityDisplayShouldIncreaseContrast` рамки и разделители — системные `NSColor` (подписка на изменение);
  `ATSApplicationFontsPath`, `LSMinimumSystemVersion` 15.0, usage descriptions для Desktop / Documents / Downloads.
- [ ] Экран 01 (доступ к диску): проба FDA при запуске и перед сканом; тексты данными; «Открыть Настройки» (URL из
  Knowledge [FDA]), «Сканировать без доступа», «Перезапустить» (`open -n -b`); орнамент «засеянное поле» на Core
  Graphics, растеризуется один раз на масштаб.
- [ ] Экран 02 (сканирование): сектор роста, строка «getattrlistbulk · N потоков · путь», «Остановить»; кэш →
  мгновенное открытие и дельты FSEvents; строка «после обновления macOS первый скан полный» при сбросе кэша.
- [ ] Экран 03 (главное): диаграмма с режимами Sunburst / Treemap (запоминается), крошки `NSStackView` со
  сворачиванием средних уровней в «…» с меню, список уровня `NSTableView` view-based (имя в конце, размер не
  режется, полоса доли; усечённое имя несёт `toolTip` и `accessibilityLabel` с полным текстом), легенда категорий
  чипами с суммами, контекстное меню «Показать в Finder» (`activateFileViewerSelectingURLs`) у списка и крошек,
  «свободно», тултип-панель `NSPanel` над диаграммой (путь в середине), сегменты «Скрытое», «Другие тома», «macOS»,
  «в облаке» с источником цифр в тултипе.
- [ ] Композиционный корень `Session`: сканер → индекс → кэш → FSEvents → рекомендации → Корзина → дубликаты →
  агент; headless-режимы бинарника: `--snapshot` по Knowledge [UI] (состояния из `--state fixture.json`: FDA
  нет/есть, прогресс скана как в книге — 286,4 ГБ / 1,28 млн файлов, состояния модели) и `--e2e <scenario.json>
  --report <report.json>` — выполняет шаги Session по сценарию и вопрос модели через штатный XPC-клиент, пишет
  JSON-отчёт (ответ, вызовы инструментов, итоги, пути снимков).
- [ ] Модуль `strings` (ru-only) по разделу VIII; `lineBreakMode` по таблице раздела X для каждого элемента.
- [ ] Тесты (`crates/app/tests/`): `snapshot_shell.rs` — экраны 01–03 × 2 темы × 2 масштаба (+ 03 в режиме
  `--contrast high`) из фикстуры состояний → PNG существуют, размеры верны, детерминизм; `theme_pixels.rs` — пиксели
  фона окна, сайдбара и акцентной кнопки на снимках 03 равны `Palette::DARK/LIGHT` (`bg`, `bg_2`, `wax`) с допуском
  1/255; `smoke_render_scanner.rs` — Session на фикстуре → модель диаграммы непуста; `truncation.rs` —
  `lineBreakMode` элементов по таблице X, размеры никогда не режутся, усечённое имя несёт `toolTip` и
  `accessibilityLabel`.

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-app --test snapshot_shell
```
- Снимки экранов 01–03 из фикстуры состояний детерминированы в двух темах, масштабах и режиме контраста (exit 0).
```
cargo test -p domovoi-app --test theme_pixels
```
- Токены на снимках равны палитре theme (exit 0).
```
cargo test -p domovoi-app --test smoke_render_scanner
```
- REAL-DEP-SMOKE: Session связывает настоящие сканер, индекс и render на фикстуре (exit 0).
```
cargo test -p domovoi-app --test truncation
```
- Правила обрезки раздела X закреплены для каждого элемента, подсказка и VoiceOver несут полный текст (exit 0).
```
cargo clippy -p domovoi-app --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).
**Done when (AGENT-JUDGED):** ревьюер читает PNG `target/snapshots/{01,02,03}-{dark,light}@2x.png` и `03-dark@2x-contrast.png` (файлы на диске, не diff) и сверяет композицию с макетами раздела VII книги: 01 — заголовок, три шага, две кнопки, орнамент; 02 — сектор роста и строка потоков; 03 — крошки, список уровня, диаграмма, легенда, «свободно». Обоснование agent-judged: композиционное соответствие макетам машиной не проверяется.

### Phase 16: ui-care — экраны 04 Рекомендации, 06 Дубликаты, 08 Корзина Домового

<!-- deps: [ui-shell] — потребление: Session и оболочка ui-shell, модели rules и dups (smoke_rules_dups). -->

- [ ] Экран 04: `NSCollectionView` в `NSScrollView`, закреплённая шапка «Можно освободить X» с разбивкой по уровням,
  карточки правил (заголовок до 2 строк, путь в середине, пояснение переносится, бейдж уровня, `правило · id`,
  сумма и объяснение удержания), гипотезы пунктиром с `гипотеза модели` и давностью через `format_age`, предвыбор
  «Безопасно», «В Корзину Домового · X», пояснение «закройте приложение» для запущенных владельцев.
- [ ] Экран 06: переключатель «Искать дубликаты» запускает проход вручную с прогрессом и отменой, `NSOutlineView` с
  группами, «оставить», чекбоксы копий, бейдж «клон APFS» для уже-клонов (disabled), переключатель «В Корзину |
  Заменить клоном» с объяснением («имена остаются, место общее, изменение любой копии снова занимает место») и
  недоступностью при невыполненных предусловиях, строка «открытые файлы других пользователей не видны — закройте их
  сами», итог «освободится».
- [ ] Экран 08: лист подтверждения (строки с категорией и размером, итог воском, «Файлы можно вернуть из Корзины
  Finder…», Отмена / Переместить в Корзину), результат «Перемещено в Корзину: X», раздел возврата из журнала,
  орнамент завершения 1400 мс однократно (смена кадра при reduce motion).
- [ ] Тесты (`crates/app/tests/`): `snapshot_care.rs` — экраны 04, 06, 08 из фикстуры состояний;
  `smoke_rules_dups.rs` — Session → рекомендации и группы на фикстуре → view-модели экранов; `voice.rs` — строки
  раздела VIII присутствуют, запрещённые формулировки и «!» отсутствуют.

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-app --test snapshot_care
```
- Снимки экранов 04, 06, 08 детерминированы (exit 0).
```
cargo test -p domovoi-app --test smoke_rules_dups
```
- REAL-DEP-SMOKE: настоящие rules и dups через Session → view-модели экранов (exit 0).
```
cargo test -p domovoi-app --test voice
```
- Голос интерфейса соответствует таблице раздела VIII (exit 0).
```
cargo clippy -p domovoi-app --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).
**Done when (AGENT-JUDGED):** ревьюер читает PNG `target/snapshots/{04,06,08}-{dark,light}@2x.png` (файлы на диске, не diff) и сверяет композицию с макетами экранов 04, 06, 08 раздела VII и образцами раздела VI (бейджи безопасности и происхождения, карточка рекомендации, строка размера). Обоснование agent-judged: композиционное соответствие макетам машиной не проверяется.

### Phase 17: ui-ai — экраны 05 Домовой и 07 Модель, клиент сервиса

<!-- deps: [ui-care] — потребление: agent, downloader через Session (smoke_agent_downloader), xpc-sys и inference-proto клиентом (smoke_inference_client). -->

- [ ] Экран 05: чат в `NSScrollView` (сообщения, видимые вызовы инструментов пунктирными блоками `query_index … → N
  узлов`, гипотезы с бейджем, кнопки действий из результатов инструментов), ввод `NSTextView`, подпись «модель видит
  пути, размеры и даты · содержимое файлов не читается»; состояния: AI выключен (переключатель «Домовой (AI)» на
  экране 07), модель не скачана (переход к Модели), загружается, выгружена; отмена генерации; сторож зависшего
  сервиса.
- [ ] Экран 07: `NSGridView`, переключатель «Домовой (AI)», «Этот Mac: X ГБ общей памяти · подходит …», карточки
  классов (имя, размер, «от N ГБ памяти», Скачать / Пауза / Отменить / Удалить, прогресс с байтами, скоростью и
  «осталось ~N мин», подпись «докачка поддерживается», «SHA-256 совпадает», Активна), выгрузка модели (сразу / 5 /
  15 / 60 минут / без таймера — с пояснением, что система сама может завершить простаивающий сервис), источник
  загрузки (зеркало, файл вручную), осиротевшие файлы.
- [ ] Интеграция: XPC-клиент из приложения (handshake, pid для сторожа, fd весов), обработка падения сервиса,
  сторож по таймауту из настроек, `ModelEvent` в индекс после загрузки и удаления.
- [ ] Тесты (`crates/app/tests/`): `snapshot_ai.rs` — экраны 05 и 07 во всех состояниях из фикстуры;
  `smoke_agent_downloader.rs` — Session с FakeBackend над настоящими agent и каталогом → view-модель чата с вызовами
  инструментов и кнопкой действия; `smoke_inference_client.rs` — XPC-клиент приложения ↔ настоящий listener сервиса
  (крейт inference, `--backend fake`) через анонимный endpoint: handshake с pid и версией, `load` по fd временного
  файла, стрим токенов, `cancel`, скриптованное зависание → сторож по таймауту (≤ 5 с из окружения) делает
  `kill(pid, SIGKILL)`, клиент переподключается.

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-app --test snapshot_ai
```
- Снимки экранов 05 и 07 во всех состояниях детерминированы (exit 0).
```
cargo test -p domovoi-app --test smoke_agent_downloader
```
- REAL-DEP-SMOKE: настоящие agent и downloader через Session → view-модели (exit 0).
```
cargo test -p domovoi-app --test smoke_inference_client
```
- REAL-DEP-SMOKE: клиент приложения и настоящий сервис по протоколу, сторож убивает зависший сервис (exit 0).
```
cargo clippy -p domovoi-app --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).
**Done when (AGENT-JUDGED):** ревьюер читает PNG `target/snapshots/{05,07}-{dark,light}@2x.png` (файлы на диске, не diff) и сверяет композицию с макетами экранов 05 и 07 раздела VII и карточкой загрузки раздела VI. Обоснование agent-judged: композиционное соответствие макетам машиной не проверяется.

### Phase 18: distribution — бандл с XPC-сервисом, подпись, нотаризация, Sparkle, релиз

<!-- deps: [ui-ai] — потребление: бинарники app и inference, metallib из llama-sys, шрифты и версия из workspace. -->

- [ ] `xtask bundle --profile <dev|adhoc|release>` (cargo-bundle и cargo-packager не умеют вложенные `.xpc` и второй
  исполняемый файл): `cargo build --release --bin domovoi --bin domovoi-inference` → `target/bundle/DOMOVOI.app`
  (Contents/MacOS/domovoi, Info.plist, Resources/Fonts, Resources/catalog.json,
  XPCServices/com.domovoi.inference.xpc/Contents/{Info.plist, MacOS, Resources/default.metallib,
  Resources/ggml-tensor.metallib}, Frameworks/Sparkle.framework из `Sparkle-2.10.0.tar.xz` по SHA-256 без
  Installer.xpc и Downloader.xpc — не нужны вне песочницы), rpath `@executable_path/../Frameworks`;
  `CFBundleShortVersionString` и `CFBundleVersion` обоих Info.plist штампуются из `[workspace.package].version`.
- [ ] Подпись изнутри наружу без `--deep` («Consider Deep Harmful»): Sparkle (Autoupdate, Updater.app, фреймворк) →
  сервис с entitlements (sandbox без сети) → приложение; профили: dev = Developer ID с `--options runtime
  --timestamp`; adhoc = `-` без hardened runtime, фича `adhoc`; release = Developer ID + hardened runtime +
  `xcrun notarytool submit --keychain-profile --wait` + `stapler staple` + zip (`ditto -c -k --keepParent`) и DMG;
  `allow-jit` не запрашивается (Metal компилирует шейдеры в MTLCompilerService; прямого утверждения Apple нет —
  подтверждается первой нотаризацией release-профиля); `rcodesign` 0.29.0 — запасной путь без Apple-инструментов.
- [ ] `xtask check-bundle <app>`: структура, `lipo -info` только arm64, Info.plist (`CFBundleIdentifier`,
  `LSMinimumSystemVersion` 15.0, `SUFeedURL`, `SUPublicEDKey`, usage descriptions, `ATSApplicationFontsPath`, версии
  равны версии workspace), plist сервиса, entitlements, `codesign --verify --deep --strict`.
- [ ] Sparkle в приложении: `extern_class!(SPUStandardUpdaterController)`, `#[link(name = "Sparkle", kind =
  "framework")]`, пункт меню «Проверить обновления…», appcast через `generate_appcast` / `sign_update` (EdDSA в
  Keychain), `<sparkle:minimumSystemVersion>15.0</sparkle:minimumSystemVersion>`, версия в appcast — из workspace.
- [ ] `xtask release`: инкремент патча версии workspace, сборка release, appcast, тег, запись в CHANGELOG.
- [ ] Прогрев гейта: `cargo build --release --bin domovoi --bin domovoi-inference` выполняется задачей до гейта.

**Done when (EXECUTABLE):**
```
cargo xtask bundle --profile adhoc
```
- Бандл с двумя исполняемыми файлами, сервисом и Sparkle собирается и подписывается ad-hoc (инкрементально, exit 0).
```
cargo xtask check-bundle target/bundle/DOMOVOI.app
```
- Структура, arm64-only, plist-ключи, версии и entitlements соответствуют спеке (exit 0).
```
codesign --verify --deep --strict target/bundle/DOMOVOI.app
```
- Подпись бандла и вложений валидна (exit 0). Нотаризация и Developer ID — ручная приёмка.
```
cargo clippy --workspace --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).

### Phase 19: wire — сквозной главный путь на настоящих модулях

<!-- deps: [distribution] — потребление ВСЕХ code-фаз: wire-терминал по WIRE-PHASE-RULE. -->

- [ ] `crates/app/tests/wire_main_path.rs`: APFS-образ с фикстурой (DerivedData, `target/` при `Cargo.toml`,
  `node_modules` при `package.json`, дубликаты с клоном, sparse, папка `mode 000`) и локальный `tiny_http` с
  tiny-random.gguf (302 → 206 с Range, `If-Range`); запуск `target/bundle/DOMOVOI.app/Contents/MacOS/domovoi --e2e
  <scenario> --report <report>` дочерним процессом с `GGML_METAL_DEVICES=0`; сценарий: скан образа → индекс → кэш
  (запись и повторное открытие) → создание файла на образе → дельта FSEvents → узел в индексе → рекомендации (итог
  по уровням, denylist) → Корзина Домового (перемещение в `.Trashes/<uid>` образа и возврат по журналу) → дубликаты
  (группы, замена клоном одной пары) → загрузка модели по test-only записи каталога в models-dir фикстуры (SHA-256
  совпал, запись исключена из бэкапа) → сервис грузит скачанный файл по fd → вопрос Домовому (грамматика первого
  шага ограничена веткой `tool_call`; ответ валиден по схеме, вызов инструмента есть) → `--snapshot all` в обеих
  темах; тест проверяет report.json и 16 PNG непустые.
- [ ] Гейты конвенций по всему workspace: комментарии, токены, источники, отсутствие `unlink` / `removeItem` вне
  тестов.
- [ ] Прогрев гейта: `cargo xtask bundle --profile adhoc` выполняется задачей до гейта.
- [ ] Предел терминала (честно): окно в живом WindowServer, онбординг FDA на VM macOS 15, нотаризация — приёмка
  человеком после зелёного wire.

**Done when (EXECUTABLE):**
```
cargo xtask bundle --profile adhoc
```
- Бандл для e2e собран (инкрементально, exit 0).
```
cargo test -p domovoi-app --test wire_main_path
```
- Главный путь проходит на настоящих модулях из бандла: от образа и загрузки модели до ответа и снимков (exit 0).
```
cargo xtask check-comments
```
- Конвенция комментариев держится по всему workspace (exit 0).
```
cargo xtask check-tokens
```
- Цвета только из theme по всему workspace (exit 0).
```
cargo xtask check-sources
```
- Документ и книга остаются согласованы с принятыми решениями (exit 0).
```
cargo clippy --workspace --all-targets -- -D warnings
```
- Весь workspace чист (exit 0).
