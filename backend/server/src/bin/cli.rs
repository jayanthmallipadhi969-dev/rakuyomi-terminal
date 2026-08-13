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
            "set-storage-path" => {
                if let Some(p) = parts.next() {
                    set_storage_path(&state, p.to_string()).await?;
                } else {
                    println!("Usage: set-storage-path <path>");
                }
            }
            "set-ram" => {
                if let Some(flag) = parts.next() {
                    set_ram(&state, flag.to_string()).await?;
                } else {
                    println!("Usage: set-ram <on|off>");
                }
            }
            "set-concurrent" => {
                if let Some(n) = parts.next() {
                    set_concurrent(&state, n.to_string()).await?;
                } else {
                    println!("Usage: set-concurrent <n>");
                }
            }
            "set-proxy" => {
                if let Some(val) = parts.next() {
                    set_proxy(&state, val.to_string()).await?;
                } else {
                    println!("Usage: set-proxy <url|none>");
                }
            }
            "install-sources" => {
                install_sources(&state).await?;
            }
            "install-source" => {
                if let Some(src_id) = parts.next() {
                    install_source(&state, src_id.to_string()).await?;
                } else {
                    println!("Usage: install-source <source_id>");
                }
            }
            "install-all-sources" => {
                install_all_sources(&state).await?;
            }
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
    println!("  set-storage-path <path>   Set storage path and persist settings");
    println!("  set-ram <on|off>          Enable/disable RAM-backed tmpfs storage");
    println!("  set-concurrent <n>        Set concurrent requests for pages");
    println!("  set-proxy <url|none>      Set or clear proxy URL");
    println!("  install-sources           List available sources from settings.source_lists");
    println!("  install-source <source_id>  Install a source by id (from source lists)");
    println!("  install-all-sources       Install all available sources from settings.source_lists if not already installed");
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

async fn set_storage_path(state: &crate::state::State, path: String) -> anyhow::Result<()> {
    let mut settings_guard = state.settings.lock().await;
    settings_guard.storage_path = Some(PathBuf::from(path.clone()));
    settings_guard
        .save_to_file(&state.settings_path)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;

    // Update source manager with new settings (reload sources if needed)
    let mut sm = state.source_manager.lock().await;
    sm.update_settings(settings_guard.clone(), &state.source_manager)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;

    println!("storage_path set to {} and saved", path);
    Ok(())
}

async fn set_ram(state: &crate::state::State, flag: String) -> anyhow::Result<()> {
    let mut settings_guard = state.settings.lock().await;
    match flag.as_str() {
        "on" | "true" | "1" => {
            settings_guard.ram_storage_enabled = true;
            settings_guard
                .save_to_file(&state.settings_path)
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;

            // Try to enable RAM on chapter storage
            let mut cs = state.chapter_storage.lock().await;
            match cs.enable_ram(settings_guard.ram_storage_size_mb) {
                Ok(_) => println!("RAM storage enabled (size {} MB)", settings_guard.ram_storage_size_mb),
                Err(e) => println!("Failed to enable RAM storage: {}", e),
            }
        }
        "off" | "false" | "0" => {
            settings_guard.ram_storage_enabled = false;
            settings_guard
                .save_to_file(&state.settings_path)
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;

            let mut cs = state.chapter_storage.lock().await;
            cs.disable_ram();
            println!("RAM storage disabled");
        }
        other => println!("Unknown value for set-ram: {} (use on|off)", other),
    }
    Ok(())
}

async fn set_concurrent(state: &crate::state::State, n: String) -> anyhow::Result<()> {
    let parsed = n.parse::<usize>().map_err(|_| anyhow::anyhow!("invalid number"))?;
    let mut settings_guard = state.settings.lock().await;
    settings_guard.concurrent_requests_pages = Some(parsed);
    settings_guard
        .save_to_file(&state.settings_path)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    println!("concurrent_requests_pages set to {}", parsed);
    Ok(())
}

async fn set_proxy(state: &crate::state::State, val: String) -> anyhow::Result<()> {
    let mut settings_guard = state.settings.lock().await;
    if val.eq_ignore_ascii_case("none") || val.eq_ignore_ascii_case("null") {
        settings_guard.proxy_url = None;
        println!("proxy cleared");
    } else {
        settings_guard.proxy_url = Some(val.clone());
        println!("proxy set to {}", val);
    }
    settings_guard
        .save_to_file(&state.settings_path)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;

    // Update tls proxy in shared if available
    shared::tls::set_proxy_url(settings_guard.proxy_url.clone());

    Ok(())
}

async fn install_sources(state: &crate::state::State) -> anyhow::Result<()> {
    let settings_guard = state.settings.lock().await.clone();
    if settings_guard.source_lists.is_empty() {
        println!("No source lists configured in settings.source_lists");
        return Ok(());
    }

    println!("Fetching available sources from configured source lists...");
    let available = shared::usecases::list_available_sources(settings_guard.source_lists.clone())
        .await
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;

    println!("Available sources:");
    for src in available.iter() {
        println!("{} - {}  (from {})", src.id.value(), src.name, src.source_of_source.clone().unwrap_or_default());
    }

    Ok(())
}

async fn install_source(state: &crate::state::State, source_id: String) -> anyhow::Result<()> {
    let settings_guard = state.settings.lock().await.clone();
    if settings_guard.source_lists.is_empty() {
        println!("No source lists configured in settings.source_lists");
        return Ok(());
    }

    let available = shared::usecases::list_available_sources(settings_guard.source_lists.clone())
        .await
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;

    let found = available.into_iter().find(|s| s.id.value() == &source_id);
    let found = match found {
        Some(s) => s,
        None => {
            println!("Source id {} not found in configured source lists", source_id);
            return Ok(());
        }
    };

    let domain = found.source_of_source.clone().unwrap_or_default();
    let src_id = shared::model::SourceId::new(found.id.value().clone());

    let mut sm = state.source_manager.lock().await;
    // call shared installer
    shared::usecases::install_source(&mut *sm, &state.source_manager, &settings_guard.source_lists, src_id, domain)
        .await
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;

    println!("Installed source {}", source_id);
    Ok(())
}

async fn install_all_sources(state: &crate::state::State) -> anyhow::Result<()> {
    let settings_guard = state.settings.lock().await.clone();
    if settings_guard.source_lists.is_empty() {
        println!("No source lists configured in settings.source_lists");
        return Ok(());
    }

    println!("Fetching available sources from configured source lists...");
    let available = shared::usecases::list_available_sources(settings_guard.source_lists.clone())
        .await
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;

    let mut sm = state.source_manager.lock().await;
    let existing: std::collections::HashSet<String> = sm
        .sources()
        .into_iter()
        .map(|s| s.manifest().info.id.clone())
        .collect();

    for src in available.into_iter() {
        let id = src.id.value().clone();
        if existing.contains(&id) {
            println!("Skipping already installed source: {}", id);
            continue;
        }
        println!("Installing {}...", id);
        let domain = src.source_of_source.clone().unwrap_or_default();
        let src_id = shared::model::SourceId::new(id.clone());
        // Install each source; don't hold the lock across await by cloning manager reference
        let arc_manager = state.source_manager.clone();
        shared::usecases::install_source(&mut *sm, &arc_manager, &settings_guard.source_lists, src_id, domain)
            .await
            .map_err(|e| anyhow::anyhow!(format!("failed to install {}: {}", id, e)))?;
        println!("Installed {}", id);
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
