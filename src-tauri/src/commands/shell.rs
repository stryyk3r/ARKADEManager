/// Hosts the app is allowed to open in the browser. Anything else is refused so this command
/// can't be used to launch arbitrary URLs/protocol handlers.
const ALLOWED_URL_HOSTS: &[&str] = &["github.com"];

/// Open a URL in the system default browser.
#[tauri::command]
pub async fn open_external_url(url: String) -> Result<(), String> {
    let url = validate_external_url(&url)?;
    // Launched directly (never through `cmd /C start`), so characters like `&` in the URL are not
    // interpreted by a shell.
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(&url)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&url)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        std::process::Command::new("xdg-open")
            .arg(&url)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Parse and re-serialize the URL (which percent-encodes anything unusual) and require https on an
/// allowed host.
fn validate_external_url(raw: &str) -> Result<String, String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err("URL is empty".to_string());
    }
    let parsed = url::Url::parse(raw).map_err(|_| "Invalid URL".to_string())?;
    if parsed.scheme() != "https" {
        return Err("Only https URLs can be opened".to_string());
    }
    let host = parsed.host_str().unwrap_or("");
    if !ALLOWED_URL_HOSTS.contains(&host) {
        return Err(format!("Opening links to {} is not allowed", host));
    }
    Ok(parsed.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_github_https() {
        assert_eq!(
            validate_external_url("https://github.com/stryyk3r/ARKADEManager/releases").unwrap(),
            "https://github.com/stryyk3r/ARKADEManager/releases"
        );
    }

    #[test]
    fn rejects_other_hosts_schemes_and_lookalikes() {
        assert!(validate_external_url("https://evil.example/").is_err());
        assert!(validate_external_url("http://github.com/").is_err());
        assert!(validate_external_url("file:///C:/Windows/System32/calc.exe").is_err());
        assert!(validate_external_url("https://github.com.evil.example/").is_err());
        assert!(validate_external_url("calc.exe").is_err());
    }

    #[test]
    fn shell_metacharacters_are_encoded() {
        let url = validate_external_url("https://github.com/a b\"&calc").unwrap();
        assert!(!url.contains(' ') && !url.contains('"'), "{}", url);
    }
}
