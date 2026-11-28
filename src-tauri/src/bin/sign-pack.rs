//! Creator tool for .promptpack files. Uses the same signing code as the app's
//! `verify_pack`, so anything it signs verifies in the app.
//!
//!   sign-pack keygen [--key PATH]                       new creator key (refuses to overwrite)
//!   sign-pack pubkey [--key PATH]                       print the public key
//!   sign-pack sign   [--key PATH] FILE...               sign packs in place
//!   sign-pack verify FILE...                            exit 1 if any pack fails
//!   sign-pack index  --out FILE --base-url URL FILE...  write registry index.json
//!
//! Default key: <OS config dir>/promptpack/creator.key (hex). Keep it out of git.

use ed25519_dalek::SigningKey;
use promptpack_marketplace::pack::{self, PromptPack};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// Splits `--flag value` options from positional args.
struct Args {
    flags: Vec<(String, String)>,
    files: Vec<PathBuf>,
}

impl Args {
    fn parse(raw: &[String]) -> Result<Self> {
        let (mut flags, mut files) = (Vec::new(), Vec::new());
        let mut it = raw.iter();
        while let Some(a) = it.next() {
            if let Some(name) = a.strip_prefix("--") {
                let value = it.next().ok_or_else(|| format!("--{name} needs a value"))?;
                flags.push((name.to_string(), value.clone()));
            } else {
                files.push(PathBuf::from(a));
            }
        }
        Ok(Self { flags, files })
    }

    fn get(&self, name: &str) -> Option<&str> {
        self.flags.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str())
    }

    fn key_path(&self) -> Result<PathBuf> {
        if let Some(p) = self.get("key") {
            return Ok(PathBuf::from(p));
        }
        let base = std::env::var_os("APPDATA")
            .or_else(|| std::env::var_os("XDG_CONFIG_HOME"))
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
            .ok_or("cannot find a config dir; pass --key")?;
        Ok(base.join("promptpack").join("creator.key"))
    }

    fn files(&self) -> Result<&[PathBuf]> {
        if self.files.is_empty() {
            return Err("no .promptpack files given".into());
        }
        Ok(&self.files)
    }
}

fn run(raw: &[String]) -> Result<ExitCode> {
    let (cmd, rest) = raw.split_first().ok_or(USAGE)?;
    let args = Args::parse(rest)?;
    match cmd.as_str() {
        "keygen" => {
            let path = args.key_path()?;
            if path.exists() {
                return Err(format!("{} already exists; refusing to overwrite a creator key", path.display()).into());
            }
            let mut seed = [0u8; 32];
            getrandom(&mut seed)?;
            let key = SigningKey::from_bytes(&seed);
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir)?;
            }
            std::fs::write(&path, hex::encode(key.to_bytes()))?;
            println!("wrote {}\npublic key: {}", path.display(), hex::encode(key.verifying_key().as_bytes()));
            println!("Back this file up somewhere safe: anyone with it can sign packs as you.");
        }
        "pubkey" => println!("{}", hex::encode(load_key(&args.key_path()?)?.verifying_key().as_bytes())),
        "sign" => {
            let key = load_key(&args.key_path()?)?;
            for path in args.files()? {
                let signed = pack::sign_pack(read_pack(path)?, &key)?;
                pack::verify(&signed)?;
                std::fs::write(path, serde_json::to_string_pretty(&signed)? + "\n")?;
                println!("signed  {}  id {}", path.display(), signed.metadata.id);
            }
        }
        "verify" => {
            let mut failed = 0;
            for path in args.files()? {
                match read_pack(path).and_then(|p| pack::verify(&p).map(|_| p).map_err(Into::into)) {
                    Ok(p) => println!("ok      {}  id {}", path.display(), p.metadata.id),
                    Err(e) => {
                        failed += 1;
                        println!("FAILED  {}  {e}", path.display());
                    }
                }
            }
            if failed > 0 {
                return Ok(ExitCode::FAILURE);
            }
        }
        "index" => {
            let out = PathBuf::from(args.get("out").ok_or("--out is required")?);
            let base = args.get("base-url").ok_or("--base-url is required")?.trim_end_matches('/');
            write_index(&out, base, args.files()?)?;
        }
        _ => return Err(USAGE.into()),
    }
    Ok(ExitCode::SUCCESS)
}

const USAGE: &str = "usage: sign-pack <keygen|pubkey|sign|verify|index> [--key PATH] [FILE...]";

fn load_key(path: &Path) -> Result<SigningKey> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read key {}: {e} (run `sign-pack keygen` first)", path.display()))?;
    let bytes: [u8; 32] = hex::decode(text.trim())?.try_into().map_err(|_| "key must be 32 bytes of hex")?;
    Ok(SigningKey::from_bytes(&bytes))
}

fn read_pack(path: &Path) -> Result<PromptPack> {
    let text = std::fs::read_to_string(path)?;
    serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()).into())
}

/// Regenerate the registry index from verified packs, keeping each pack's
/// existing price (matched by name) if `out` already exists.
fn write_index(out: &Path, base_url: &str, files: &[PathBuf]) -> Result<()> {
    let old: serde_json::Value = std::fs::read_to_string(out)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default();
    let price_of = |name: &str| {
        old["packs"]
            .as_array()
            .and_then(|ps| ps.iter().find(|p| p["name"] == name))
            .and_then(|p| p["price_usd"].as_f64())
            .unwrap_or(0.0)
    };

    let mut packs = Vec::new();
    for path in files {
        let p = read_pack(path)?;
        pack::verify(&p).map_err(|e| format!("{}: {e}", path.display()))?;
        let file = path.file_name().and_then(|f| f.to_str()).ok_or("bad file name")?;
        packs.push(serde_json::json!({
            "id": p.metadata.id,
            "name": p.metadata.name,
            "version": p.metadata.version,
            "author": p.metadata.author,
            "author_pubkey": p.metadata.author_pubkey,
            "description": p.metadata.description,
            "tags": p.metadata.tags,
            "download_url": format!("{base_url}/{file}"),
            "signature": "ed25519",
            "price_usd": price_of(&p.metadata.name),
            "created_at": p.metadata.created_at,
            "updated_at": p.metadata.updated_at,
        }));
    }
    let index = serde_json::json!({ "version": 1, "updated_at": chrono::Utc::now(), "packs": packs });
    std::fs::write(out, serde_json::to_string_pretty(&index)? + "\n")?;
    println!("wrote {} ({} packs)", out.display(), files.len());
    Ok(())
}

/// Key material straight from the OS CSPRNG.
fn getrandom(buf: &mut [u8]) -> Result<()> {
    getrandom::getrandom(buf).map_err(|e| format!("OS randomness unavailable: {e}").into())
}
