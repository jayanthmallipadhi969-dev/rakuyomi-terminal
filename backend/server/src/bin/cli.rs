use std::io::{self, Write};
use std::path::PathBuf;

use clap::Parser;
use tokio::sync::CancellationToken;

use shared::model::{MangaId, SourceId};

use crate::app::{build_state, init_logging};

/// Minimal terminal-only Rakuyomi CLI. Provides searching and downloading
/// without the full UI, designed to be lightweight.
#[derive(Parser, Debug)]
#[command(name = "rakuyomi-cli")]
struct Args {
    /// Path to rakuyomi home folder (contains settings.json, database, sources...)
    #[arg(long)]
    home: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_logging();

    let args = Args::parse();

    let home = args
        .home
        .or_else(|| std::env::var("RAKUYOMI_HOME").ok().map(PathBuf::from))
        .or_else(|| dirs::home_dir().map(|h| h.join(".rakuyomi")))
        .unwrap_or_else(|| PathBuf::from("."));

    println!("Using home path: {}", home.display());

    let state = build_state(home).await?;

    repl_loop(state).await
}

async fn repl_loop(state: crate::state::State) -> anyhow::Result<()> {
    println!("Rakuyomi terminal (type 'help' for commands)");

    let mut input = String::new();

    loop {
        print!("> ");
        let _ = io::stdout().flush();
        input.clear();
        if io::stdin().read_line(&mut input)? == 0 {
            // EOF
            break;
        }
        let cmd = input.trim();
        if cmd.is_empty() {
            continue;
        }

        let mut parts = cmd.split_whitespace();
        match parts.next().unwrap() {
            "help" => print_help(),
            "exit" | "quit" => break,
            "list-sources" => list_sources(&state).await?,
            "show-settings" => show_settings(&state).await?,
            "search" => {
                let query = parts.collect::<Vec<_>>().join(" ");
                if query.is_empty() {
                    println!("Usage: search <query>");
                } else {
                    search_mangas(&state, query).await?;
                }
            }
            "download" => {
                let rest: Vec<_> = parts.collect();
                if rest.is_empty() {
                    println!("Usage: download <source_id:manga_id> [unread|all]");
                } else {
                    let id = rest[0];
                    let mode = rest.get(1).map(|s| *s).unwrap_or("unread");
                    download_command(&state, id.to_string(), mode.to_string()).await?;
                }
            }
            other => println!("Unknown command: {} (type 'help')", other),
        }
    }

    Ok(())
}

fn print_help() {
    println!("Commands:");
    println!("  help               Show this help");
    println!("  list-sources       List installed sources");
    println!("  show-settings      Print current settings");
    println!("  search <query>     Search across sources");
    println!("  download <src:manga> [unread|all]   Download manga chapters (default: unread)");
    println!("  exit, quit         Exit");
}

async fn list_sources(state: &crate::state::State) -> anyhow::Result<()> {
    let manager = state.source_manager.lock().await.clone();
    for source in manager.sources() {
        let manifest = source.manifest();
        println!("{} - {}", manifest.info.id, manifest.info.name);
    }
    Ok(())
}

async fn show_settings(state: &crate::state::State) -> anyhow::Result<()> {
    let settings = state.settings.lock().await.clone();
    println!("Settings ({}):", state.settings_path.display());
    println!("  storage_path: {:?}", settings.storage_path);
    println!("  storage_size_limit: {:?}", settings.storage_size_limit);
    println!("  optimize_image: {}", settings.optimize_image);
    println!("  ram_storage_enabled: {}", settings.ram_storage_enabled);
    println!("  proxy_url: {:?}", settings.proxy_url);
    Ok(())
}

async fn search_mangas(state: &crate::state::State, query: String) -> anyhow::Result<()> {
    use shared::usecases::search_mangas;
    use tokio_util::sync::CancellationToken;

    let manager_clone = state.source_manager.lock().await.clone();
    let chapter_storage = state.chapter_storage.lock().await.clone();
    let settings = state.settings.lock().await.clone();

    let cancellation = CancellationToken::new();

    println!("Searching for: '{}'...", query);
    let (mangas, errors, has_next) = search_mangas(
        &manager_clone,
        &*state.database,
        &chapter_storage,
        &settings,
        cancellation,
        query,
        &None,
        1,
        10,
    )
    .await
    .map_err(|e| anyhow::anyhow!(e.to_string()))?;

    for m in mangas.iter().enumerate() {
        let (idx, manga) = m;
        let title = manga.information.title.clone().unwrap_or_else(|| "<no title>".into());
        println!("[{}] {}  -- {}:{}", idx, title, manga.information.id.source_id().value(), manga.information.id.value());
    }

    if !errors.is_empty() {
        println!("Some sources failed to search:");
        for e in errors {
            println!("  {}: {}", e.source_id, e.reason);
        }
    }
    println!("Has next page: {}", has_next);

    Ok(())
}

async fn download_command(state: &crate::state::State, id: String, mode: String) -> anyhow::Result<()> {
    use futures::StreamExt;
    use tokio_util::sync::CancellationToken;

    use shared::usecases::{fetch_manga_chapters_in_batch, refresh_manga_details};
    use shared::usecases::fetch_manga_chapters_in_batch::Filter;

    // Accept either "source:manga" or "source/manga"
    let (source_id_str, manga_id_str) = if id.contains(':') {
        let mut parts = id.splitn(2, ':');
        (parts.next().unwrap().to_string(), parts.next().unwrap().to_string())
    } else if id.contains('/') {
        let mut parts = id.splitn(2, '/');
        (parts.next().unwrap().to_string(), parts.next().unwrap().to_string())
    } else {
        println!("Invalid manga id format. Use source:manga_id");
        return Ok(());
    };

    let source_id = SourceId::new(source_id_str.clone());
    let manga_id = MangaId::from_strings(source_id_str.clone(), manga_id_str.clone());

    // Find source
    let manager_guard = state.source_manager.lock().await;
    let source = match manager_guard.get_by_id(&source_id) {
        Some(s) => s.clone(),
        None => {
            println!("Source not found: {}", source_id.value());
            return Ok(());
        }
    };
    drop(manager_guard);

    // Prepare chapter storage and DB clones
    let chapter_storage = state.chapter_storage.lock().await.clone();

    let token = CancellationToken::new();

    // Ensure manga details cached
    println!("Refreshing manga details...");
    match refresh_manga_details(&token, &*state.database, &chapter_storage, &source, &manga_id, 15).await {
        Ok(_) => {}
        Err(e) => println!("Warning: could not refresh manga details: {}", e),
    }

    // Determine filter
    let filter = match mode.as_str() {
        "all" => Filter::AllUnreadChapters,
        "unread" | _ => Filter::AllUnreadChapters,
    };

    println!("Starting download (this may take a while)...");

    let settings_guard = state.settings.lock().await.clone();
    let concurrent = settings_guard.concurrent_requests_pages.unwrap_or(5);
    let optimize = settings_guard.optimize_image;
    let title_format = settings_guard.chapter_title_format;

    let mut stream = fetch_manga_chapters_in_batch(
        token.clone(),
        &source,
        &*state.database,
        &chapter_storage,
        manga_id,
        filter,
        &[],
        concurrent,
        optimize,
        title_format,
    );

    while let Some(report) = stream.next().await {
        match report {
            ProgressReport::Progressing { downloaded, total } => {
                println!("Downloaded {}/{}", downloaded, total);
            }
            ProgressReport::Finished => {
                println!("Download finished");
            }
            ProgressReport::Cancelled => {
                println!("Download cancelled");
            }
            ProgressReport::Errored(e) => {
                println!("Download errored: {}", e);
            }
        }
    }

    Ok(())
}
