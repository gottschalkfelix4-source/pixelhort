//! `pixelhort` command line client: login, bulk upload, albums, server info.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::{Context, bail};
use chrono::{DateTime, Utc};
use clap::{Parser, Subcommand};
use futures::StreamExt;
use pixelhort_common::{
    AlbumDto, BulkUploadCheckItem, BulkUploadCheckRequest, BulkUploadCheckResponse, CreateAlbumRequest, IdsRequest,
    LoginRequest, LoginResponse, ServerInfo, UploadResponse, media_type_for,
};
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use tokio::io::AsyncReadExt;

#[derive(Parser)]
#[command(name = "pixelhort", version, about = "Command line client for Pixelhort")]
struct Cli {
    /// Server URL (overrides the stored login)
    #[arg(long, env = "PIXELHORT_SERVER", global = true)]
    server: Option<String>,
    /// API key (overrides the stored login)
    #[arg(long, env = "PIXELHORT_API_KEY", global = true)]
    api_key: Option<String>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Log in with e-mail/password (or store an API key) and remember the server
    Login {
        server: String,
        #[arg(long)]
        email: Option<String>,
        #[arg(long, env = "PIXELHORT_PASSWORD")]
        password: Option<String>,
        #[arg(long)]
        key: Option<String>,
    },
    /// Forget the stored credentials
    Logout,
    /// Upload files and folders (duplicates are skipped by checksum)
    Upload {
        /// Files or directories
        #[arg(required = true)]
        paths: Vec<PathBuf>,
        /// Recurse into sub directories
        #[arg(short, long, default_value_t = true)]
        recursive: bool,
        /// Add all uploaded files to this album (created if missing)
        #[arg(long)]
        album: Option<String>,
        /// Create one album per top-level folder name
        #[arg(long)]
        album_per_folder: bool,
        /// Parallel uploads
        #[arg(short = 'j', long, default_value_t = 4)]
        concurrency: usize,
        /// Only show what would be uploaded
        #[arg(long)]
        dry_run: bool,
        /// Delete local files after a successful upload (or when already on the server)
        #[arg(long)]
        delete: bool,
    },
    /// Show server information
    Info,
    /// List albums
    Albums,
}

#[derive(Serialize, Deserialize, Default)]
struct Stored {
    server: String,
    token: Option<String>,
    api_key: Option<String>,
}

fn config_path() -> PathBuf {
    dirs::config_dir().unwrap_or_else(|| PathBuf::from(".")).join("pixelhort").join("cli.json")
}

fn load_stored() -> Option<Stored> {
    serde_json::from_slice(&std::fs::read(config_path()).ok()?).ok()
}

struct Client {
    base: String,
    http: reqwest::Client,
    auth: (String, String),
}

impl Client {
    fn from(cli: &Cli) -> anyhow::Result<Self> {
        let stored = load_stored();
        let base = cli
            .server
            .clone()
            .or_else(|| stored.as_ref().map(|s| s.server.clone()))
            .context("no server configured – run `pixelhort login <url>` first")?;
        let auth = if let Some(k) = cli.api_key.clone().or_else(|| stored.as_ref().and_then(|s| s.api_key.clone())) {
            ("x-api-key".to_string(), k)
        } else if let Some(t) = stored.and_then(|s| s.token) {
            ("authorization".to_string(), format!("Bearer {t}"))
        } else {
            bail!("not logged in – run `pixelhort login`");
        };
        Ok(Self { base: normalize(&base), http: reqwest::Client::new(), auth })
    }

    fn req(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        self.http.request(method, format!("{}/api{path}", self.base)).header(&self.auth.0, &self.auth.1)
    }

    async fn json<T: for<'de> Deserialize<'de>>(&self, rb: reqwest::RequestBuilder) -> anyhow::Result<T> {
        let res = rb.send().await?;
        let status = res.status();
        if !status.is_success() {
            bail!("{status}: {}", res.text().await.unwrap_or_default());
        }
        Ok(res.json().await?)
    }
}

fn normalize(url: &str) -> String {
    let u = url.trim().trim_end_matches('/');
    let u = u.strip_suffix("/api").unwrap_or(u);
    if u.starts_with("http") { u.to_string() } else { format!("http://{u}") }
}

async fn sha1_file(path: &Path) -> anyhow::Result<String> {
    let mut f = tokio::fs::File::open(path).await?;
    let mut h = Sha1::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut buf).await?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(hex::encode(h.finalize()))
}

fn collect(paths: &[PathBuf], recursive: bool) -> Vec<(PathBuf, Option<String>)> {
    let mut out = Vec::new();
    for root in paths {
        if root.is_file() {
            out.push((root.clone(), None));
            continue;
        }
        let folder = root.file_name().map(|n| n.to_string_lossy().to_string());
        let walker = walkdir::WalkDir::new(root).max_depth(if recursive { usize::MAX } else { 1 });
        for e in walker.into_iter().filter_map(Result::ok) {
            let name = e.file_name().to_string_lossy();
            if e.file_type().is_file() && !name.starts_with('.') && media_type_for(&name).is_some() {
                out.push((e.path().to_path_buf(), folder.clone()));
            }
        }
    }
    out
}

async fn upload_one(client: &Client, path: &Path, checksum: &str) -> anyhow::Result<UploadResponse> {
    let meta = tokio::fs::metadata(path).await?;
    let modified: DateTime<Utc> = meta.modified().map(Into::into).unwrap_or_else(|_| Utc::now());
    let created: DateTime<Utc> = meta.created().map(Into::into).unwrap_or(modified);
    let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "file".into());
    let file = tokio::fs::File::open(path).await?;
    let body = reqwest::Body::wrap_stream(tokio_util::io::ReaderStream::new(file));
    let part = reqwest::multipart::Part::stream_with_length(body, meta.len()).file_name(name);
    let mut form = reqwest::multipart::Form::new()
        .text("deviceAssetId", path.to_string_lossy().to_string())
        .text("deviceId", "pixelhort-cli")
        .text("fileCreatedAt", created.min(modified).to_rfc3339())
        .text("fileModifiedAt", modified.to_rfc3339())
        .part("assetData", part);
    for sidecar in [PathBuf::from(format!("{}.xmp", path.display())), path.with_extension("xmp")] {
        if sidecar.exists() {
            let bytes = tokio::fs::read(&sidecar).await?;
            form = form.part("sidecarData", reqwest::multipart::Part::bytes(bytes).file_name("sidecar.xmp"));
            break;
        }
    }
    client
        .json(client.req(reqwest::Method::POST, "/assets").header("x-checksum", checksum).multipart(form))
        .await
}

async fn find_or_create_album(client: &Client, name: &str) -> anyhow::Result<uuid::Uuid> {
    let albums: Vec<AlbumDto> = client.json(client.req(reqwest::Method::GET, "/albums?shared=false")).await?;
    if let Some(a) = albums.iter().find(|a| a.album_name == name) {
        return Ok(a.id);
    }
    let req = CreateAlbumRequest { album_name: name.to_string(), description: None, asset_ids: vec![], album_users: vec![] };
    let a: AlbumDto = client.json(client.req(reqwest::Method::POST, "/albums").json(&req)).await?;
    Ok(a.id)
}

#[allow(clippy::too_many_arguments)]
async fn upload(
    client: Client,
    paths: Vec<PathBuf>,
    recursive: bool,
    album: Option<String>,
    album_per_folder: bool,
    concurrency: usize,
    dry_run: bool,
    delete: bool,
) -> anyhow::Result<()> {
    let files = collect(&paths, recursive);
    println!("{} media files found, hashing...", files.len());
    let mut hashed = Vec::new();
    for (i, (p, folder)) in files.into_iter().enumerate() {
        match sha1_file(&p).await {
            Ok(h) => hashed.push((i.to_string(), p, folder, h)),
            Err(e) => eprintln!("skip {}: {e}", p.display()),
        }
    }
    let mut to_upload = Vec::new();
    let mut existing = Vec::new();
    for chunk in hashed.chunks(500) {
        let req = BulkUploadCheckRequest {
            assets: chunk.iter().map(|(id, _, _, h)| BulkUploadCheckItem { id: id.clone(), checksum: h.clone() }).collect(),
        };
        let res: BulkUploadCheckResponse =
            client.json(client.req(reqwest::Method::POST, "/assets/bulk-upload-check").json(&req)).await?;
        for r in res.results {
            let item = chunk.iter().find(|(id, ..)| *id == r.id).expect("known id").clone();
            if r.action == "accept" {
                to_upload.push(item);
            } else {
                existing.push((item, r.asset_id));
            }
        }
    }
    println!("{} new, {} already on the server", to_upload.len(), existing.len());
    if dry_run {
        for (_, p, ..) in &to_upload {
            println!("would upload {}", p.display());
        }
        return Ok(());
    }

    let client = Arc::new(client);
    let done = Arc::new(AtomicUsize::new(0));
    let total = to_upload.len();
    let results: Vec<_> = futures::stream::iter(to_upload.into_iter().map(|(_, path, folder, sum)| {
        let client = client.clone();
        let done = done.clone();
        async move {
            let r = upload_one(&client, &path, &sum).await;
            let n = done.fetch_add(1, Ordering::Relaxed) + 1;
            match &r {
                Ok(u) => println!("[{n}/{total}] {} {}", u.status, path.display()),
                Err(e) => eprintln!("[{n}/{total}] FAILED {}: {e:#}", path.display()),
            }
            (path, folder, r)
        }
    }))
    .buffer_unordered(concurrency.max(1))
    .collect()
    .await;

    let mut by_album: std::collections::BTreeMap<String, Vec<uuid::Uuid>> = Default::default();
    let mut ok_paths = Vec::new();
    for (path, folder, r) in results {
        if let Ok(u) = r {
            let target = album.clone().or(if album_per_folder { folder } else { None });
            if let Some(name) = target {
                by_album.entry(name).or_default().push(u.id);
            }
            ok_paths.push(path);
        }
    }
    for ((_, path, folder, _), id) in &existing {
        let target = album.clone().or(if album_per_folder { folder.clone() } else { None });
        if let (Some(name), Some(id)) = (target, id) {
            by_album.entry(name).or_default().push(*id);
        }
        ok_paths.push(path.clone());
    }
    for (name, ids) in by_album {
        let album_id = find_or_create_album(&client, &name).await?;
        let _: serde_json::Value = client
            .json(client.req(reqwest::Method::PUT, &format!("/albums/{album_id}/assets")).json(&IdsRequest { ids: ids.clone() }))
            .await?;
        println!("album \"{name}\": {} assets", ids.len());
    }
    if delete {
        for p in ok_paths {
            if let Err(e) = tokio::fs::remove_file(&p).await {
                eprintln!("could not delete {}: {e}", p.display());
            }
        }
    }
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match &cli.command {
        Command::Login { server, email, password, key } => {
            let base = normalize(server);
            let http = reqwest::Client::new();
            let stored = if let Some(k) = key {
                let res = http.get(format!("{base}/api/users/me")).header("x-api-key", k).send().await?;
                if !res.status().is_success() {
                    bail!("API key rejected: {}", res.status());
                }
                Stored { server: base, token: None, api_key: Some(k.clone()) }
            } else {
                let email = email.clone().context("--email or --key is required")?;
                let password = password.clone().context("--password (or PIXELHORT_PASSWORD) is required")?;
                let req = LoginRequest { email, password, device_type: Some("cli".into()), device_name: Some("pixelhort-cli".into()) };
                let res = http.post(format!("{base}/api/auth/login")).json(&req).send().await?;
                if !res.status().is_success() {
                    bail!("login failed: {}", res.text().await.unwrap_or_default());
                }
                let login: LoginResponse = res.json().await?;
                println!("logged in as {}", login.user.email);
                Stored { server: base, token: Some(login.access_token), api_key: None }
            };
            let path = config_path();
            std::fs::create_dir_all(path.parent().expect("parent"))?;
            std::fs::write(&path, serde_json::to_vec_pretty(&stored)?)?;
            println!("credentials stored in {}", path.display());
        }
        Command::Logout => {
            let _ = std::fs::remove_file(config_path());
            println!("logged out");
        }
        Command::Info => {
            let client = Client::from(&cli)?;
            let info: ServerInfo = client.json(client.req(reqwest::Method::GET, "/server/info")).await?;
            println!("{}", serde_json::to_string_pretty(&info)?);
        }
        Command::Albums => {
            let client = Client::from(&cli)?;
            let albums: Vec<AlbumDto> = client.json(client.req(reqwest::Method::GET, "/albums")).await?;
            for a in albums {
                println!("{}  {:<40} {:>6} assets{}", a.id, a.album_name, a.asset_count, if a.shared { "  (shared)" } else { "" });
            }
        }
        Command::Upload { paths, recursive, album, album_per_folder, concurrency, dry_run, delete } => {
            let client = Client::from(&cli)?;
            upload(client, paths.clone(), *recursive, album.clone(), *album_per_folder, *concurrency, *dry_run, *delete).await?;
        }
    }
    Ok(())
}
