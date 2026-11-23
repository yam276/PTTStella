//! Headless, explicit synthetic-store driver for cross-version acceptance.
//! The same example can be compiled against the Dioxus baseline's desktop
//! feature. It invokes that revision's production stores, never system paths,
//! connections, windows or credential providers.
use pttstella::{bookmarks, drafts, filters, recent_boards, settings, sites};
use serde_json::{Value, json};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

fn value(bytes: &[u8]) -> Result<Value, String> {
    serde_json::from_slice(bytes).map_err(|e| e.to_string())
}
fn read(path: &Path, maximum: usize) -> Result<Vec<u8>, String> {
    pttstella::storage::read_bounded(path, maximum)
        .map_err(|_| format!("cannot read {}", path.display()))?
        .ok_or_else(|| format!("missing {}", path.display()))
}
fn root(argument: &str) -> Result<PathBuf, String> {
    let root = fs::canonicalize(argument).map_err(|e| e.to_string())?;
    if !root.is_dir() {
        return Err("fixture root is not a directory".into());
    }
    let marker = value(&read(&root.join(".store-compatibility.json"), 1024)?)?;
    if marker != json!({"format":"PTTStella.StoreCompatibility","version":1,"synthetic":true}) {
        return Err("explicit synthetic fixture marker required".into());
    }
    Ok(root)
}
fn snapshot(root: &Path) -> Result<Value, String> {
    let app = settings::store::Store::at(root.join("settings.json")).load()?;
    let site = sites::store::Store::at(root.join("sites.json")).load()?;
    let rules = filters::store::Store::at(root.join("filters.json")).load()?;
    let book = bookmarks::store::Store::at(root.join("bookmarks.json")).load()?;
    let draft = drafts::store::Store::at(root.join("drafts.json")).load()?;
    let recent = recent_boards::store::Store::at(root.join("recent-boards.json")).load()?;
    let profile = settings::PortableProfile::decode(&read(
        &root.join("display-profile.json"),
        settings::MAX_SETTINGS_BYTES,
    )?)?;
    Ok(json!({
        "settings":app,"sites":value(&site.encode()?)?,"filters":rules,
        "bookmarks":value(&book.encode()?)?,"drafts":value(&draft.encode()?)?,
        "recent":value(&recent.encode()?)?,"profile":profile,
    }))
}
struct Stage {
    name: &'static str,
    number: u8,
    account: &'static str,
    board: &'static str,
}
impl Stage {
    fn parse(name: &str) -> Result<Self, String> {
        let (name, number, account, board) = match name {
            "candidate" => ("candidate", 1, "FromSlint", "C_Chat"),
            "rollback" => ("rollback", 2, "BackDioxus", "Gossiping"),
            "final" => ("final", 3, "FinalSlint", "Tech_Job"),
            _ => return Err("unknown compatibility edit stage".into()),
        };
        Ok(Self {
            name,
            number,
            account,
            board,
        })
    }
}
fn edit(root: &Path, stage: Stage) -> Result<Value, String> {
    let store = settings::store::Store::at(root.join("settings.json"));
    let base = store.load()?;
    let mut next = base.clone();
    next.tools_opacity = settings::ToolOpacity::try_from(21 + 22 * stage.number)?;
    store.save_from(&base, next)?;

    let store = sites::store::Store::at(root.join("sites.json"));
    let base = store.load()?;
    let mut next = base.clone();
    next.save_favourite(
        format!("新增末字 {}", stage.name),
        sites::Target::new(
            format!("telnet://{}.example:2323", stage.name),
            pttstella::encoding::TextEncoding::Gbk,
        )?,
    )?;
    store.save_from(&base, next)?;

    let store = filters::store::Store::at(root.join("filters.json"));
    let base = store.load()?;
    let next = base.edit(stage.account, &format!("名單末字 {}", stage.name), false)?;
    store.save_from(&base, next)?;

    let store = bookmarks::store::Store::at(root.join("bookmarks.json"));
    let base = store.load()?;
    let next = base.upsert(
        pttstella::article_target::ArticleTarget::new(
            "Stella",
            &format!("0000010{}", stage.number + 1),
        )
        .map_err(|e| e.to_string())?,
        &format!("收藏末字 {}", stage.name),
        100 + u64::from(stage.number),
    )?;
    store.save_from(&base, next)?;

    let store = drafts::store::Store::at(root.join("drafts.json"));
    let base = store
        .load()?
        .entries
        .into_iter()
        .find(|e| e.id == "edited")
        .ok_or("edited fixture draft missing")?;
    let mut next = base.clone();
    next.body.push_str(&format!("\n修改末字:{}", stage.name));
    next.saved_at = 200 + u64::from(stage.number);
    let saved = store.save(Some(base), next, format!("unexpected-{}", stage.name))?;
    if saved.conflicted {
        return Err("sequential edit unexpectedly conflicted".into());
    }

    let store = recent_boards::store::Store::at(root.join("recent-boards.json"));
    let mut visits = recent_boards::RecentBoards::default();
    visits.visit(
        pttstella::board_navigation::BoardName::parse(stage.board).map_err(|e| e.to_string())?,
    );
    store.record(&visits, false)?;

    let path = root.join("display-profile.json");
    let mut profile =
        settings::PortableProfile::decode(&read(&path, settings::MAX_SETTINGS_BYTES)?)?;
    profile.name.push_str(&format!(" · {}", stage.name));
    pttstella::storage::replace(&path, &profile.encode()?)?;
    snapshot(root)
}
fn barrier(root: &Path, actor: &str) -> Result<(), String> {
    if !matches!(actor, "a" | "b") {
        return Err("unknown fixture actor".into());
    }
    let directory = root.join("barrier");
    let mut ready = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(directory.join(format!("ready-{actor}")))
        .map_err(|e| e.to_string())?;
    ready.write_all(b"ready").map_err(|e| e.to_string())?;
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if directory
            .join("continue")
            .try_exists()
            .map_err(|e| e.to_string())?
        {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err("fixture barrier timed out".into());
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}
fn concurrent(root: &Path, actor: &str, field: &str, number: &str) -> Result<Value, String> {
    if field == "draft" {
        let store = drafts::store::Store::at(root.join("drafts.json"));
        let base = store
            .load()?
            .entries
            .into_iter()
            .find(|e| e.id == "edited")
            .ok_or("edited fixture draft missing")?;
        let mut next = base.clone();
        next.body.push_str(&format!("\n並行末字:{actor}"));
        barrier(root, actor)?;
        return serde_json::to_value(store.save(Some(base), next, format!("conflict-{actor}"))?)
            .map_err(|e| e.to_string());
    }
    let store = settings::store::Store::at(root.join("settings.json"));
    let base = store.load()?;
    let mut next = base.clone();
    match field {
        "opacity" => {
            next.tools_opacity =
                settings::ToolOpacity::try_from(number.parse::<u8>().map_err(|e| e.to_string())?)?
        }
        "hover" if number == "off" => next.hover = settings::HoverPosition::Off,
        _ => return Err("unknown fixture field/value".into()),
    }
    barrier(root, actor)?;
    serde_json::to_value(store.save_from(&base, next)?).map_err(|e| e.to_string())
}
fn run() -> Result<Value, String> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [command] if command == "--describe" => {
            let mut setting = serde_json::to_value(settings::AppSettings::default()).map_err(|e| e.to_string())?;
            setting["auto_expand_images"] = Value::Bool(true);
            let auto_images = settings::AppSettings::decode(&serde_json::to_vec(&setting).map_err(|e| e.to_string())?).is_ok();
            Ok(json!({
                "package_version":env!("CARGO_PKG_VERSION"),
                "source_ref":option_env!("PTTSTELLA_COMPAT_REF").unwrap_or("unspecified"),
                "accepts_auto_expand_images":auto_images,
                "system_stores":false,"vault":false,"network":false,"GUI":false,
            }))
        },
        [command, directory] if command == "inspect" => snapshot(&root(directory)?),
        [command, directory, stage] if command == "edit" => edit(&root(directory)?, Stage::parse(stage)?),
        [command, directory, actor, field, number] if command == "concurrent" => concurrent(&root(directory)?, actor, field, number),
        _ => Err("usage: store_compatibility --describe | inspect DIR | edit DIR candidate|rollback|final | concurrent DIR a|b opacity|hover|draft VALUE".into()),
    }
}
fn main() {
    match run() {
        Ok(value) => println!("{}", json!({"status":"ok","value":value})),
        Err(error) => {
            eprintln!("{}", json!({"status":"error","error":error}));
            std::process::exit(2);
        }
    }
}
