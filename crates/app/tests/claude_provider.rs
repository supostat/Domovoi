mod fake_claude;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use domovoi_agent::ClaudeCode;
use domovoi_app::{
    AskJob, Assistant, AssistantProvider, AssistantSettings, Availability, ChatEntry, ChatModel, ClaudeCodeChoice,
    Question, Session, SessionPaths, claude_directories, find_claude, strings,
};
use domovoi_downloader::{Catalog, ModelStore};
use domovoi_scanner::CancelToken;
use domovoi_testkit::{ImageVolume, TempTree, Visibility, VolumeFormat};
use fake_claude::{ANSWER, ANSWERING, FakeClaude, HANGING, SESSION, is_running, value_after};
use objc2::AnyThread;
use objc2_foundation::{NSString, NSUserDefaults};

const SETTINGS_SUITE: &str = "domovoi.test.assistant-settings";
const QUESTION: &str = "Что можно убрать из сборок?";
const FOLLOW_UP: &str = "А внутри target что?";
const EXECUTABLE: u32 = 0o755;
const PLAIN_FILE: u32 = 0o644;
const POLL: Duration = Duration::from_millis(10);
const STOP_BUDGET: Duration = Duration::from_secs(5);

struct Volume {
    _image: ImageVolume,
    _state: TempTree,
    session: Session,
}

impl Volume {
    fn scanned() -> Self {
        let image = ImageVolume::create(VolumeFormat::Apfs, Visibility::Hidden).expect("the image attaches");
        let root = fs::canonicalize(image.mount_point()).expect("the mount point resolves");
        fs::create_dir_all(root.join("Projects/engine/target/debug")).expect("the project tree is created");
        fs::write(
            root.join("Projects/engine/Cargo.toml"),
            "[package]\nname = \"engine\"\n",
        )
        .expect("the manifest is written");
        fs::write(root.join("Projects/engine/target/debug/engine"), vec![7; 64 * 1024])
            .expect("the build output is written");
        let state = TempTree::new().expect("a temp tree is created");
        let mut session =
            Session::open(&root, &SessionPaths::in_directory(state.path()), "0.0.0-test").expect("the session opens");
        session.scan(None, CancelToken::default()).expect("the volume scans");
        Self {
            _image: image,
            _state: state,
            session,
        }
    }
}

fn ask_and_wait(session: Session, claude: &ClaudeCode, chat: &mut ChatModel, text: &str) -> Session {
    let question = Question {
        text: text.to_owned(),
        history: chat.past_turns(),
        ..Question::default()
    };
    chat.ask(text);
    let mut job = AskJob::start_with_claude(session, None, claude.clone(), question, chat.claude_session.clone());
    while !job.is_finished() {
        job.absorb(chat);
        thread::sleep(POLL);
    }
    job.absorb(chat);
    let outcome = job.finish();
    outcome.reply.record(chat);
    outcome.session
}

fn settings(provider: AssistantProvider) -> AssistantSettings {
    AssistantSettings {
        enabled: true,
        provider,
        ..AssistantSettings::default()
    }
}

fn assistant(store: &ModelStore, provider: AssistantProvider, claude: Option<ClaudeCode>) -> Assistant {
    Assistant::new(store.clone(), Catalog::bundled(), settings(provider)).with_claude_code(claude)
}

#[test]
fn the_chosen_provider_is_saved_and_read_back() {
    let suite = NSString::from_str(SETTINGS_SUITE);
    let defaults = NSUserDefaults::initWithSuiteName(NSUserDefaults::alloc(), Some(&suite)).expect("a suite opens");
    defaults.removePersistentDomainForName(&suite);
    assert_eq!(
        AssistantSettings::load_from(&defaults).provider,
        AssistantProvider::LocalModel
    );
    settings(AssistantProvider::ClaudeCode).save_to(&defaults);
    let read = AssistantSettings::load_from(&defaults);
    defaults.removePersistentDomainForName(&suite);
    assert_eq!(read.provider, AssistantProvider::ClaudeCode);
    assert!(read.enabled);
}

#[test]
fn the_search_finds_claude_in_the_given_directories_and_stays_quiet_without_it() {
    let tree = TempTree::new().expect("a temp tree is created");
    let plain = tree.path().join("plain");
    let installed = tree.path().join("installed");
    for (directory, mode) in [(&plain, PLAIN_FILE), (&installed, EXECUTABLE)] {
        fs::create_dir(directory).expect("a directory is created");
        let program = directory.join("claude");
        fs::write(&program, "#!/bin/sh\n").expect("a program is written");
        fs::set_permissions(&program, fs::Permissions::from_mode(mode)).expect("the mode is set");
    }
    let empty = tree.path().join("empty");
    fs::create_dir(&empty).expect("a directory is created");

    assert_eq!(
        find_claude(&[empty.clone(), plain.clone(), installed.clone()]),
        Some(installed.join("claude"))
    );
    assert_eq!(
        find_claude(&[empty, plain]),
        None,
        "a file without the executable bit is not claude"
    );
    let home = Path::new("/Users/somebody");
    assert_eq!(
        claude_directories(home),
        [
            home.join(".local/bin"),
            PathBuf::from("/opt/homebrew/bin"),
            PathBuf::from("/usr/local/bin")
        ]
    );
}

#[test]
fn with_claude_code_chosen_the_assistant_is_ready_without_a_model() {
    let tree = TempTree::new().expect("a temp tree is created");
    let store = ModelStore::open(&tree.path().join("models")).expect("the model store opens");
    let claude = FakeClaude::behaving(ANSWERING).claude_code();

    let chosen = assistant(&store, AssistantProvider::ClaudeCode, Some(claude.clone()));
    assert_eq!(chosen.availability(), Availability::Ready);
    assert_eq!(
        chosen.models().model.claude_code,
        Some(ClaudeCodeChoice {
            found: true,
            chosen: true
        })
    );

    let local = assistant(&store, AssistantProvider::LocalModel, Some(claude));
    assert_eq!(local.availability(), Availability::NoModel);

    let missing = assistant(&store, AssistantProvider::ClaudeCode, None);
    assert_eq!(
        missing.availability(),
        Availability::NoModel,
        "without claude the local model answers"
    );
    assert_eq!(
        missing.models().model.claude_code,
        Some(ClaudeCodeChoice {
            found: false,
            chosen: true
        })
    );
}

#[test]
fn a_question_goes_through_claude_code_and_the_next_one_resumes_its_session() {
    let volume = Volume::scanned();
    let claude = FakeClaude::behaving(ANSWERING);
    let mut chat = ChatModel::default();

    let session = ask_and_wait(volume.session, &claude.claude_code(), &mut chat, QUESTION);
    assert!(
        !claude.arguments().iter().any(|argument| argument == "--resume"),
        "the first question opens a session"
    );
    assert!(
        matches!(&chat.entries[1], ChatEntry::ToolCall { tool, .. } if tool == "summarize"),
        "{:?}",
        chat.entries
    );
    assert!(
        matches!(&chat.entries[2], ChatEntry::Answer { text, .. } if text == ANSWER),
        "{:?}",
        chat.entries
    );
    assert_eq!(chat.claude_session.as_deref(), Some(SESSION));

    ask_and_wait(session, &claude.claude_code(), &mut chat, FOLLOW_UP);
    assert_eq!(value_after(&claude.arguments(), "--resume"), SESSION);
    assert!(claude.written("prompt").contains(FOLLOW_UP));
}

#[test]
fn stop_ends_claude_and_says_the_answer_was_stopped() {
    let volume = Volume::scanned();
    let claude = FakeClaude::behaving(HANGING);
    let mut chat = ChatModel::default();
    chat.ask(QUESTION);
    let question = Question {
        text: QUESTION.to_owned(),
        ..Question::default()
    };
    let mut job = AskJob::start_with_claude(volume.session, None, claude.claude_code(), question, None);
    while !claude.pid_file().exists() {
        thread::sleep(POLL);
    }
    let stopped = Instant::now();
    job.stop();
    while !job.is_finished() {
        job.absorb(&mut chat);
        assert!(stopped.elapsed() < STOP_BUDGET, "claude did not stop");
        thread::sleep(POLL);
    }
    job.finish().reply.record(&mut chat);

    assert!(
        matches!(chat.entries.last(), Some(ChatEntry::Failure { text }) if text == strings::ANSWER_CANCELLED),
        "{:?}",
        chat.entries
    );
    let pid = fs::read_to_string(claude.pid_file()).expect("the pid is written");
    assert!(!is_running(pid.trim()), "claude {pid} still runs");
}
