use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use stellaris_tech::build::{BuildInput, build};
use stellaris_tech::source::GameSource;
use stellaris_tech::{discover, mods};

/// Extract the Stellaris technology tree (vanilla and mods) to JSON.
#[derive(Parser)]
#[command(version)]
struct Cli {
    /// Stellaris install directory (auto-detected from Steam when omitted).
    #[arg(long, global = true)]
    game: Option<PathBuf>,
    /// Paradox user directory holding launcher-v2.sqlite and mod/ (auto-detected).
    #[arg(long, global = true)]
    user_dir: Option<PathBuf>,
    /// Localisation language, e.g. english, german, braz_por.
    #[arg(long, default_value = "english")]
    lang: String,
    /// Load mods from a launcher playset; with no name, the active playset.
    #[arg(long, num_args = 0..=1, default_missing_value = "")]
    playset: Option<String>,
    /// Extra mod (directory, .mod file or workshop id); repeatable, loaded after the playset in order.
    #[arg(long = "mod", value_name = "PATH")]
    mods: Vec<PathBuf>,
    /// Output file.
    #[arg(long, short, default_value = "techs.json")]
    out: PathBuf,
    /// Pretty-print the JSON.
    #[arg(long)]
    pretty: bool,
    /// Verbose logging (-v info, -vv debug).
    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    verbose: u8,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// List launcher playsets.
    ListPlaysets,
    /// List the enabled mods of a playset (the active one by default).
    ListMods { name: Option<String> },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let level = match cli.verbose {
        0 => "warn",
        1 => "info",
        _ => "debug",
    };
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(level)).init();

    let user_dir = cli.user_dir.clone().or_else(discover::user_dir);
    match &cli.command {
        Some(Command::ListPlaysets) => {
            let ud = user_dir.context("Paradox user directory not found; pass --user-dir")?;
            for p in mods::list_playsets(&ud)? {
                println!("{}{}", if p.active { "* " } else { "  " }, p.name);
            }
            return Ok(());
        }
        Some(Command::ListMods { name }) => {
            let ud = user_dir.context("Paradox user directory not found; pass --user-dir")?;
            let (pname, list) = mods::playset_mods(&ud, name.as_deref())?;
            println!("{pname}:");
            for m in list {
                println!("  {:>3}  {}  ({})", m.position, m.name, m.dir.map(|d| d.display().to_string()).unwrap_or_default());
            }
            return Ok(());
        }
        None => {}
    }

    let game = match cli.game.clone().or_else(discover::find_game) {
        Some(g) if discover::is_game_dir(&g) => g,
        Some(g) => bail!("{} does not look like a Stellaris install (no common/technology)", g.display()),
        None => bail!("could not find Stellaris in any Steam library; pass --game"),
    };
    log::info!("game: {}", game.display());

    let mut source = GameSource::new(game.clone());
    let mut warnings = Vec::new();
    let mut playset_name = None;
    if let Some(name) = &cli.playset {
        let ud = user_dir.as_deref().context("Paradox user directory not found; pass --user-dir")?;
        let (pname, layers) = mods::playset_layers(ud, (!name.is_empty()).then_some(name.as_str()), &mut warnings)?;
        log::info!("playset {pname:?}: {} mods", layers.len());
        playset_name = Some(pname);
        layers.into_iter().for_each(|l| source.push(l));
    }
    let workshop = discover::workshop_dirs();
    for m in &cli.mods {
        source.push(mods::layer_from_arg(m, user_dir.as_deref(), &workshop)?);
    }
    for w in &warnings {
        log::warn!("{w}");
    }

    let output = build(BuildInput {
        source: &source,
        lang: &cli.lang,
        game_version: discover::game_version(&game),
        playset: playset_name,
        warnings,
    });

    if let Some(dir) = cli.out.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir)?;
    }
    let json = if cli.pretty { serde_json::to_vec_pretty(&output)? } else { serde_json::to_vec(&output)? };
    std::fs::write(&cli.out, json).with_context(|| format!("writing {}", cli.out.display()))?;
    eprintln!(
        "wrote {} techs ({} warnings) to {}",
        output.meta.tech_count,
        output.meta.warnings.len(),
        cli.out.display()
    );
    Ok(())
}
