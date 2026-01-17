use std::{
    env, fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

use reqwest::blocking::Client;
use walkdir::WalkDir;
use zip::{write::FileOptions, ZipWriter};

fn main() -> io::Result<()> {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let pack_dir = manifest_dir.join("assets").join("resource_pack");
    let cache_path = manifest_dir.join("resource_pack_cache.txt"); // survives rebuilds
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    let zip_path = out_dir.join("resource_pack.zip");
    zip_dir(&pack_dir, &zip_path)?;

    // Compute sha1 (useful for MC resource-pack prompt)
    let zip_bytes = fs::read(&zip_path)?;
    let sha1_hex = {
        use sha1::{Digest, Sha1};
        let mut h = Sha1::new();
        h.update(&zip_bytes);
        format!("{:x}", h.finalize())
    };
    println!("cargo:warning=Resource pack SHA1: {sha1_hex}");

    let cached = fs::read_to_string(&cache_path).ok().and_then(|s| {
        let mut lines = s.lines();
        let url = lines.next()?.trim().to_string();
        let sha = lines.next()?.trim().to_string();
        Some((url, sha))
    });

    // Get or create URL
    let force_upload = env::var("RESOURCE_PACK_UPLOAD").ok() == Some("1".to_string());

    let (url, cached_sha) = match (force_upload, cached) {
        // Forced reupload
        (true, _) => {
            println!("cargo:warning=RESOURCE_PACK_UPLOAD set; re-uploading resource pack...");
            let url = upload_to_paste_cnet(&zip_bytes).expect("upload failed");
            fs::write(&cache_path, format!("{url}\n{sha1_hex}\n"))?;
            println!("cargo:warning=Uploaded resource pack: {url}");
            (url, sha1_hex.clone())
        }

        // Cached with same content
        (false, Some((url, sha))) if sha == sha1_hex => {
            println!("cargo:warning=Using cached resource pack (SHA unchanged): {url}");
            (url, sha)
        }

        // Cached but content changed
        (false, Some((_old_url, old_sha))) => {
            println!(
                "cargo:warning=Resource pack changed ({} -> {}), re-uploading...",
                old_sha, sha1_hex
            );
            let url = upload_to_paste_cnet(&zip_bytes).expect("upload failed");
            fs::write(&cache_path, format!("{url}\n{sha1_hex}\n"))?;
            println!("cargo:warning=Uploaded resource pack: {url}");
            (url, sha1_hex.clone())
        }

        // No cache at all
        (false, None) => {
            println!("cargo:warning=No cached resource pack found; uploading...");
            let url = upload_to_paste_cnet(&zip_bytes).expect("upload failed");
            fs::write(&cache_path, format!("{url}\n{sha1_hex}\n"))?;
            println!("cargo:warning=Uploaded resource pack: {url}");
            (url, sha1_hex.clone())
        }
    };

    // Generate consts
    let gen = format!(
        "pub const SERVER_RESOURCE_PACK_URL: &str = {url:?};\n\
         pub const SERVER_RESOURCE_PACK_SHA1_HEX: &str = {cached_sha:?};\n"
    );
    fs::write(out_dir.join("resource_pack_consts.rs"), gen)?;

    // Rebuild if pack changes
    println!("cargo:rerun-if-changed={}", pack_dir.display());
    // If you edit/clear the cached URL, rebuild too:
    println!("cargo:rerun-if-changed={}", cache_path.display());

    Ok(())
}

// Use https://paste.c-net.org/ to upload the resource pack
fn upload_to_paste_cnet(bytes: &[u8]) -> Result<String, reqwest::Error> {
    let client = Client::new();
    let resp = client
        .put("https://paste.c-net.org/") // PUT or POST both fine per their docs
        .header("X-FileName", "resource_pack.zip")
        .body(bytes.to_vec())
        .send()?
        .error_for_status()?;

    Ok(resp.text()?.trim().to_string())
}

fn zip_dir(src_dir: &Path, dst_file: &Path) -> io::Result<()> {
    use zip::DateTime;

    let file = fs::File::create(dst_file)?;
    let mut zip = ZipWriter::new(file);

    // Fixed timestamp so builds are reproducible
    let fixed_time =
        DateTime::from_date_and_time(1993, 12, 10, 0, 0, 0).expect("Invalid zip datetime");

    let options = FileOptions::<()>::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .last_modified_time(fixed_time);

    let src_dir = src_dir.canonicalize()?;

    // Collect and sort for stable ordering
    let mut files = Vec::new();
    for entry in WalkDir::new(&src_dir).into_iter().filter_map(Result::ok) {
        let path = entry.path();
        if path.is_file() {
            let rel = path.strip_prefix(&src_dir).unwrap();
            let name = rel.to_string_lossy().replace('\\', "/");
            files.push((name, path.to_path_buf()));
        }
    }
    files.sort_by(|a, b| a.0.cmp(&b.0));

    for (name, path) in files {
        zip.start_file(name, options)?;
        zip.write_all(&fs::read(path)?)?;
    }

    zip.finish()?;
    Ok(())
}
