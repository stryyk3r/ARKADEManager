#[tauri::command]
pub async fn get_app_version() -> Result<String, String> {
    Ok(env!("CARGO_PKG_VERSION").to_string())
}

#[tauri::command]
pub async fn check_for_updates() -> Result<serde_json::Value, String> {
    use serde_json::Value;

    log::info!("Checking for updates via GitHub API...");
    let current_version = env!("CARGO_PKG_VERSION");

    let client = reqwest::Client::new();
    let response = match client
        .get("https://api.github.com/repos/stryyk3r/ARKADEManager/releases/latest")
        .header("User-Agent", "ARKADE-Manager")
        .send()
        .await
    {
        Ok(resp) => resp,
        Err(e) => {
            log::error!("Failed to fetch GitHub releases: {}", e);
            return Err(format!("Failed to fetch releases: {}", e));
        }
    };

    if !response.status().is_success() {
        log::error!("GitHub API returned error: {}", response.status());
        return Err(format!("GitHub API error: {}", response.status()));
    }

    let release: Value = match response.json().await {
        Ok(r) => r,
        Err(e) => {
            log::error!("Failed to parse GitHub response: {}", e);
            return Err(format!("Failed to parse response: {}", e));
        }
    };

    let latest_version = match release.get("tag_name") {
        Some(tag) => {
            let tag_str = tag.as_str().unwrap_or("");
            tag_str.strip_prefix('v').unwrap_or(tag_str)
        }
        None => {
            log::error!("No tag_name in GitHub response");
            return Ok(serde_json::json!({ "available": false }));
        }
    };

    log::info!(
        "Current version: {}, Latest version: {}",
        current_version,
        latest_version
    );

    let current_ver = match semver::Version::parse(current_version) {
        Ok(v) => v,
        Err(e) => {
            log::error!("Failed to parse current version: {}", e);
            return Ok(serde_json::json!({ "available": false }));
        }
    };

    let latest_ver = match semver::Version::parse(latest_version) {
        Ok(v) => v,
        Err(e) => {
            log::error!("Failed to parse latest version: {}", e);
            return Ok(serde_json::json!({ "available": false }));
        }
    };

    if latest_ver > current_ver {
        log::info!("Update available: {}", latest_version);
        Ok(serde_json::json!({
            "available": true,
            "version": latest_version,
            "published_at": release.get("published_at").and_then(|b| b.as_str()).unwrap_or(""),
            "body": release.get("body").and_then(|b| b.as_str()).unwrap_or("")
        }))
    } else {
        log::info!(
            "No update available (current: {}, latest: {})",
            current_version,
            latest_version
        );
        Ok(serde_json::json!({
            "available": false,
            "version": latest_version,
            "published_at": release.get("published_at").and_then(|b| b.as_str()).unwrap_or("")
        }))
    }
}

#[tauri::command]
pub async fn install_update() -> Result<String, String> {
    use serde_json::Value;
    use std::fs;
    use std::io::Write;
    use std::process::Command;

    log::info!("Starting update installation...");

    let client = reqwest::Client::new();
    let response = client
        .get("https://api.github.com/repos/stryyk3r/ARKADEManager/releases/latest")
        .header("User-Agent", "ARKADE-Manager")
        .send()
        .await
        .map_err(|e| format!("Failed to fetch release info: {}", e))?;

    let release: Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse release info: {}", e))?;

    let assets = release
        .get("assets")
        .and_then(|a| a.as_array())
        .ok_or_else(|| "No assets found in release".to_string())?;

    let installer_url = assets
        .iter()
        .find_map(|asset| {
            let name = asset.get("browser_download_url")?.as_str()?;
            if name.ends_with("-setup.exe") {
                Some(name)
            } else {
                None
            }
        })
        .ok_or_else(|| "No installer found in release assets".to_string())?;

    log::info!("Downloading installer from: {}", installer_url);

    let installer_response = client
        .get(installer_url)
        .header("User-Agent", "ARKADE-Manager")
        .send()
        .await
        .map_err(|e| format!("Failed to download installer: {}", e))?;

    let temp_dir = std::env::temp_dir();
    let installer_path = temp_dir.join("ARKADE_Manager_Update.exe");

    let mut file = fs::File::create(&installer_path)
        .map_err(|e| format!("Failed to create temp file: {}", e))?;

    let bytes = installer_response
        .bytes()
        .await
        .map_err(|e| format!("Failed to read installer data: {}", e))?;

    file.write_all(&bytes)
        .map_err(|e| format!("Failed to write installer: {}", e))?;

    drop(file);

    log::info!("Installer downloaded to: {}", installer_path.display());

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    Command::new(&installer_path)
        .spawn()
        .map_err(|e| format!("Failed to launch installer: {}", e))?;

    log::info!("Installer launched, exiting application...");

    std::process::exit(0);
}
