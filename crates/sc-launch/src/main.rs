use sc_launch::{launch, packs, profiles};
use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "\
sc-launch - Star Conflict launcher with switchable profiles

USAGE:
  sc-launch [OPTIONS] list              List profiles
  sc-launch [OPTIONS] info              Show the detected game, tools and Proton setup
  sc-launch [OPTIONS] run <profile> [-- <game args>]
                                        Start the game with a profile (game args
                                        replace the profile's own)
  sc-launch [OPTIONS] install <zip|folder|url> [--yes]
                                        Install a server pack, or update the
                                        profile that already has it
  sc-launch [OPTIONS] updates           Check installed packs for new versions
  sc-launch [OPTIONS] update <profile> [--yes]
                                        Install the newest version of a profile's pack

OPTIONS:
  --profiles-dir <dir>   Profile folder (default: the per-user data folder)
  --game-dir <dir>       Star Conflict install (default: found through Steam)
  --tools-dir <dir>      Folder with sc-inject.exe and sc_agent.dll (default: next to sc-launch)
  --proton <dir>         Proton build to use (Linux; default: the one that last ran the game)
  --yes                  Install without asking for confirmation

ENVIRONMENT (used when the matching option is not given):
  STAR_FORGE_GAME_DIR, STAR_FORGE_TOOLS_DIR, STAR_FORGE_PROTON, STAR_FORGE_DATA_DIR
  Development builds also read them from the repo's .env file.
";

struct Cli {
    opts: launch::Options,
    profiles_dir: Option<PathBuf>,
    command: Vec<String>,
    game_args: Vec<String>,
    yes: bool,
}

fn parse() -> Result<Cli, String> {
    let mut cli = Cli {
        opts: launch::Options::default(),
        profiles_dir: None,
        command: Vec::new(),
        game_args: Vec::new(),
        yes: false,
    };
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        let mut value = |name: &str| it.next().map(PathBuf::from).ok_or(format!("{name} needs a value"));
        match a.as_str() {
            "--profiles-dir" => cli.profiles_dir = Some(value(&a)?),
            "--game-dir" => cli.opts.game_dir = Some(value(&a)?),
            "--tools-dir" => cli.opts.tools_dir = Some(value(&a)?),
            "--proton" => cli.opts.proton = Some(value(&a)?),
            "-h" | "--help" => cli.command = vec!["help".into()],
            "--yes" | "-y" => cli.yes = true,
            "--" => {
                cli.game_args = it.by_ref().collect();
            }
            _ if a.starts_with("--") => return Err(format!("unknown option {a}")),
            _ => cli.command.push(a),
        }
    }
    Ok(cli)
}

fn run() -> Result<u8, String> {
    let cli = parse()?;
    let profiles_dir = cli
        .profiles_dir
        .clone()
        .or_else(profiles::default_profiles_dir)
        .ok_or("cannot work out a profiles folder; pass --profiles-dir")?;

    match cli.command.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        [] | ["help"] => {
            print!("{USAGE}");
            Ok(0)
        }
        ["list"] => {
            let entries = profiles::list(&profiles_dir);
            if entries.is_empty() {
                println!("no profiles in {}", profiles_dir.display());
            }
            for e in entries {
                match e.profile {
                    Ok(p) => println!(
                        "{:<20} {:<24} {:<13} {} patch(es)  {}",
                        e.name,
                        p.server.as_deref().unwrap_or("-"),
                        if p.local_server { "local server" } else { "client only" },
                        p.patches.len(),
                        p.name
                    ),
                    Err(err) => println!("{:<20} INVALID: {err}", e.name),
                }
            }
            Ok(0)
        }
        ["info"] => {
            println!("profiles: {}", profiles_dir.display());
            let setup = launch::resolve(&cli.opts)?;
            println!("game:     {}", setup.game_dir.display());
            println!("tools:    {}", setup.tools_dir.display());
            #[cfg(unix)]
            {
                let p = &setup.proton;
                println!("proton:   {}", p.dir.display());
                println!(
                    "runtime:  {}",
                    p.runtime.as_ref().map_or("none (running Proton directly)".into(), |r| r.display().to_string())
                );
                println!("prefix:   {}", p.compat_data.display());
            }
            Ok(0)
        }
        ["run", name] => {
            if !profiles::valid_name(name) {
                return Err(format!("{name:?} is not a valid profile name"));
            }
            let dir = profiles_dir.join(name);
            let profile = profiles::load(&dir)?;
            let setup = launch::resolve(&cli.opts)?;
            if let Some(warning) = profiles::read_pack(&dir).ok().and_then(|p| packs::game_build_warning(&p, &setup.game_exe())) {
                eprintln!("warning: {warning}");
            }
            let mut cmd = launch::command(&setup, &dir, &cli.game_args)?;
            println!(
                "starting {} with profile {name}{}",
                setup.game_exe().display(),
                profile.server.as_deref().map(|s| format!(" (server {s})")).unwrap_or_default()
            );
            let status = cmd.status().map_err(|e| format!("could not start the injector: {e}"))?;
            println!("agent log: {}", dir.join(sc_profile::AGENT_LOG).display());
            Ok(status.code().unwrap_or(1).clamp(0, 255) as u8)
        }
        ["install", source] => {
            let staged = packs::stage(&packs::Source::parse(source)?)?;
            install_reviewed(staged, &profiles_dir, cli.yes)
        }
        ["updates"] => {
            let mut any = false;
            for e in profiles::list(&profiles_dir) {
                let Some(pack) = e.pack else { continue };
                match packs::check_update(&pack) {
                    Ok(Some(a)) => {
                        any = true;
                        println!("{:<20} {} -> {}", e.name, pack.version, a.version);
                    }
                    Ok(None) => {}
                    Err(err) => eprintln!("{}: could not check for updates: {err}", e.name),
                }
            }
            if !any {
                println!("all packs are up to date");
            }
            Ok(0)
        }
        ["update", name] => {
            if !profiles::valid_name(name) {
                return Err(format!("{name:?} is not a valid profile name"));
            }
            let pack = profiles::read_pack(&profiles_dir.join(name))?;
            match packs::check_update(&pack)? {
                None if pack.update.is_none() => Err(format!("{name} has no update address")),
                None => {
                    println!("{name} is up to date ({})", pack.version);
                    Ok(0)
                }
                Some(a) => install_reviewed(packs::stage(&packs::Source::Url(a.url))?, &profiles_dir, cli.yes),
            }
        }
        _ => Err(format!("unknown command; see sc-launch --help")),
    }
}

/// Shows what a pack will do and installs it once the user agrees.
fn install_reviewed(staged: packs::Staged, profiles_dir: &std::path::Path, yes: bool) -> Result<u8, String> {
    let pack = &staged.pack;
    let existing = profiles::find_pack(profiles_dir, &pack.id).and_then(|d| profiles::read_pack(&d).ok());
    println!("{} {} ({})", pack.name, pack.version, pack.id);
    if let Some(old) = &existing {
        println!("  updates the installed version {}", old.version);
    }
    if !pack.authors.is_empty() {
        println!("  by {}", pack.authors.join(", "));
    }
    println!("  from {}{}", staged.origin, if staged.verified { " (checksum verified)" } else { "" });
    println!("  server:  {}", pack.server.address.as_deref().unwrap_or("game default"));
    if pack.server.local {
        println!("  starts the local server with the game");
    }
    println!("  replaces {} game file(s)", staged.files.len());
    for f in staged.files.iter().take(20) {
        println!("    {f}");
    }
    if staged.files.len() > 20 {
        println!("    ... and {} more", staged.files.len() - 20);
    }
    if !pack.patches.is_empty() {
        println!("  applies {} game patch(es)", pack.patches.len());
    }
    if !pack.game.args.is_empty() {
        println!("  game arguments: {}", pack.game.args.join(" "));
    }
    for w in &staged.warnings {
        println!("  note: {w}");
    }
    if !yes {
        print!("Install? [y/N] ");
        std::io::Write::flush(&mut std::io::stdout()).ok();
        let mut answer = String::new();
        std::io::stdin().read_line(&mut answer).map_err(|e| e.to_string())?;
        if !matches!(answer.trim(), "y" | "Y" | "yes") {
            packs::discard(staged);
            println!("not installed");
            return Ok(1);
        }
    }
    let name = packs::install(staged, profiles_dir)?;
    println!("installed as profile {name}");
    Ok(0)
}

fn main() -> ExitCode {
    sc_launch::env::load_dotenv();
    match run() {
        Ok(code) => ExitCode::from(code),
        Err(e) => {
            eprintln!("sc-launch: {e}");
            ExitCode::FAILURE
        }
    }
}
