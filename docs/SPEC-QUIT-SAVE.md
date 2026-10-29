<!--
  SPEC-QUIT-SAVE.md — дельта к docs/SPEC-DOMOVOI.md: выход без подвисания — окно исчезает сразу, кэш скана
  дописывается в фоне, приложение завершается после записи. 1 фаза.

  ГРАНИЦА РЕШЕНИЯ (веер от 29.10.2026, выбран вариант A по рекомендации): applicationShouldTerminate отвечает
  NSTerminateLater, Session уходит в QuitJob, ответ replyToApplicationShouldTerminate(true) — по готовности записи.
  Отвергнуты: B — спрятать окно и писать синхронно (главный поток занят 0,7–1,15 с и больше с дельтами, растёт с
  размером диска, против соглашения spec-off-main-thread); C — ускорить запись кэша (подвисание короче, но остаётся;
  «индекс не менялся» редкость — FSEvents идут почти всегда).
-->

# Baseline

- Предыдущая спека: `docs/SPEC-DOMOVOI.md`; все дельты из `docs/` применены, последняя — `docs/SPEC-CLEANUP-RULES.md`
  (влита в `main`, `cfd9cd4`).
- Prior spec-hash: sha256:<placeholder — confirm>
- Проблема (замечено 29.10.2026). При закрытии окно ненадолго замирает: `applicationWillTerminate` →
  `ShellController::application_will_terminate` → `Session::persist` на главном потоке. Замер на машине разработчика
  (4 438 412 узлов, `scan.cache` 753 МБ): `persist` без сжатия 0,72–1,15 с, из них запись 0,25–0,4 с, `F_FULLFSYNC`
  0,01–0,07 с, остальное — сериализация rkyv в память. С применёнными дельтами `persist` ещё сжимает индекс и
  пересобирает рекомендации — эта часть не замерялась.
- Уже есть и не меняется: `Session::persist` и `write_cache` (временный файл, `sync_all`, `rename`), формат кэша,
  запись кэша после скана; остановка скана, поиска и удаления при выходе; когда `Session` у задачи (скан, поиск,
  применение дельты), при выходе кэш не пишется, как и сейчас.
- Версия: фазы повышения версии нет — `cargo xtask release` сам инкрементирует патч; `.github/workflows/ci.yml`
  версию не меняет.
- Обоснование выбора — staged decision note (id после accept).

# Stack

- `crates/app/src/quit_job.rs` (новый, `pub QuitJob`): `start(session, on_saved)` — поток с `session.persist()`,
  таймер готовности в `NSRunLoopCommonModes` главного цикла; по готовности таймер гасится, поток присоединяется,
  `on_saved(io::Result<()>)` вызывается один раз. `Drop` гасит таймер.
- `crates/app/src/app_delegate.rs`: `applicationShouldTerminate:` → `ShellController::should_terminate`; без
  контроллера — `TerminateNow`.
- `crates/app/src/shell_controller.rs`: `should_terminate() -> NSApplicationTerminateReply`; остановка задач вынесена
  в метод, общий для `should_terminate` и `application_will_terminate`; `application_will_terminate` больше не пишет
  кэш.
- `crates/app/src/shell_state.rs`: поле `quitting: Option<QuitJob>`.
- `crates/app/Cargo.toml`: `[[test]] quit_job`, `harness = false`.
- Тесты: новый `crates/app/tests/quit_job.rs`.
- `docs/DOMOVOI.md`: строка 2 таблицы модулей — про выход и кэш.

# Conventions

- Главный поток при выходе не делает работы размером с индекс (продолжение соглашения `docs/SPEC-OFF-MAIN-THREAD.md`).
- Таймер, который должен сработать, пока выход отложен, живёт в `NSRunLoopCommonModes`.
- Сбой записи кэша не держит выход: ошибка уходит в `SystemLog` (`com.domovoi.app`, категория `quit`), ответ
  `replyToApplicationShouldTerminate(true)` идёт всё равно.
- `Session` передаётся задаче владением, как в `ApplyJob`; общего доступа нет.

# Knowledge

- `NSTerminateLater`: до ответа AppKit крутит цикл в `NSModalPanelRunLoopMode`. Таймер из
  `scheduledTimerWithTimeInterval` (так ставятся все таймеры контроллера) живёт только в режиме по умолчанию и в этом
  режиме не сработает — приложение не завершится никогда. Поэтому таймер `QuitJob` добавляется в
  `NSRunLoopCommonModes`, а тест крутит цикл именно в `NSModalPanelRunLoopMode`.
- `should_terminate`: есть `Session` в состоянии → спрятать окно и панели (наведение, свойства), погасить Dock,
  остановить ассистента и задачи, забрать `Session`, запустить `QuitJob`, ответить `TerminateLater`. `Session` нет
  (экран доступа, ошибка, сессия у задачи) → `TerminateNow`, дальше прежний `application_will_terminate`.
- После `replyToApplicationShouldTerminate(true)` AppKit присылает `applicationWillTerminate`: `Session` уже нет,
  второй записи нет.
- Повторный запрос выхода во время записи (`quitting` занят) отвечает `TerminateCancel`: второй записи нет, ожидающий
  выход завершает `QuitJob`.
- Красная кнопка идёт тем же путём: `applicationShouldTerminateAfterLastWindowClosed` → `true`.
- Оборванная запись безопасна: `write_cache` пишет во временный файл и переименовывает.
- После исчезновения окна процесс живёт ещё около секунды, значок в Dock — до выхода.
- Тест держит `TempTree` целиком, не отбрасывает через `..` в деструктуризации (иначе каталог с кэшем удаляется до
  проверок).
- Предел проверки: `ShellController` закрыт для тестов, `--e2e` не проходит через `NSApp terminate`. Cmd-Q и красная
  кнопка на бандле (`cargo xtask bundle --profile adhoc`) — приёмка человеком на границе фазы: окно исчезает сразу,
  процесс завершается примерно за секунду, следующий запуск открывает кэш без скана. Первый запуск после
  `FORMAT_VERSION` 4 сканирует в любом случае — приёмка после одного полного цикла запуск → выход.

# Gameplan

### Phase 1: quit — выход без подвисания

- [ ] `quit_job.rs`: `QuitJob::start(session, on_saved)`, поток с `persist`, таймер в `NSRunLoopCommonModes`, один
  вызов `on_saved`, `Drop` гасит таймер; экспорт из `lib.rs`.
- [ ] `app_delegate.rs`: `applicationShouldTerminate:` → `should_terminate`.
- [ ] `shell_controller.rs`: `should_terminate` по Knowledge; общая остановка задач; `application_will_terminate`
  без `persist`; ошибка записи — в `SystemLog`, затем `replyToApplicationShouldTerminate(true)`.
- [ ] `shell_state.rs`: `quitting: Option<QuitJob>`.
- [ ] `crates/app/tests/quit_job.rs` (`harness = false`): скан `TempTree`, файл кэша удалён после скана,
  `QuitJob::start`, цикл в `NSModalPanelRunLoopMode` до вызова `on_saved` с пределом времени; проверки — `on_saved`
  получил `Ok`, вызван один раз (цикл крутится ещё после), кэш открывается `CacheOpening::Opened` с тем же числом
  узлов.
- [ ] `docs/DOMOVOI.md`: строка 2 таблицы модулей — при выходе окно закрывается сразу, кэш дописывается в фоне.

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-app --test quit_job
```
- Запись в фоне, готовность приходит в режиме отложенного выхода, кэш открывается (exit 0).
```
grep -q applicationShouldTerminate: crates/app/src/app_delegate.rs
```
- Делегат отвечает на запрос выхода (exit 0).
```
cargo xtask check-comments
```
- Комментарии без ссылок на книгу, фазы и спеки (exit 0).
```
cargo xtask check-sources
```
- Книга и документ без устаревших формулировок (exit 0).
```
cargo clippy -p domovoi-app --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).
