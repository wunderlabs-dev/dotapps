//! Framework detection and build execution
//!
//! Reads `package.json` to detect the JavaScript framework, derives the
//! appropriate build command and output directory, then runs the build
//! inside a temporary container via the `Runtime` trait.

use std::path::Path;

use crate::error::AppError;
use crate::infrastructure::runtime::{BuildResult, Runtime};
use crate::publish::types::{BuildConfig, Framework};
use crate::publish::PublishError;

const ENSURE_DEPS: &str = "\
if [ ! -d node_modules ] || [ package.json -nt node_modules/.package-lock.json ] || \
{ [ -f package-lock.json ] && [ package-lock.json -nt node_modules/.package-lock.json ]; }; \
then npm install; fi";

/// Detect framework from package.json dependencies.
pub fn detect_framework(repo_path: &Path) -> Result<BuildConfig, AppError> {
    let pkg_path = repo_path.join("package.json");
    if !pkg_path.exists() {
        return Err(PublishError::NoPackageJson.into());
    }

    let content = std::fs::read_to_string(&pkg_path)?;
    let pkg: serde_json::Value = serde_json::from_str(&content)?;

    let has_dep = |name: &str| -> bool {
        let in_deps = pkg.get("dependencies").and_then(|d| d.get(name)).is_some();
        let in_dev = pkg
            .get("devDependencies")
            .and_then(|d| d.get(name))
            .is_some();
        in_deps || in_dev
    };

    if has_dep("next") {
        return detect_nextjs(repo_path);
    }

    if has_dep("vite") {
        return Ok(BuildConfig {
            framework: Framework::Vite,
            build_cmd: format!("{ENSURE_DEPS} && npx vite build"),
            output_dir: "dist".into(),
        });
    }

    if has_dep("react-scripts") {
        return Ok(BuildConfig {
            framework: Framework::Cra,
            build_cmd: format!("{ENSURE_DEPS} && npx react-scripts build"),
            output_dir: "build".into(),
        });
    }

    // Fallback: check if there's a "build" script in package.json
    let has_build_script = pkg.get("scripts").and_then(|s| s.get("build")).is_some();

    if !has_build_script {
        return Err(PublishError::NoBuildScript.into());
    }

    Ok(BuildConfig {
        framework: Framework::Unknown,
        build_cmd: format!("{ENSURE_DEPS} && npm run build"),
        output_dir: "dist".into(), // probe for build/ in deployer if dist/ missing
    })
}

/// Detect Next.js configuration and validate static export support.
fn detect_nextjs(repo_path: &Path) -> Result<BuildConfig, AppError> {
    // Check next.config for output: 'export'
    let config_files = ["next.config.js", "next.config.mjs", "next.config.ts"];
    let mut has_export_config = false;

    for config_file in &config_files {
        let config_path = repo_path.join(config_file);
        if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)?;
            // Regex-free: parsing next.config is fragile across JS/TS/MJS variants,
            // but the output+export keyword pair is sufficient for static export detection
            if content.contains("output") && content.contains("export") {
                has_export_config = true;
                break;
            }
        }
    }

    if !has_export_config {
        return Err(PublishError::NextjsStaticExportRequired.into());
    }

    Ok(BuildConfig {
        framework: Framework::NextJs,
        build_cmd: format!("{ENSURE_DEPS} && npx next build"),
        output_dir: "out".into(),
    })
}

/// Script injected into index.html to handle SPA routing on GitHub Pages.
///
/// Strips the base path from location.pathname before React Router initializes
/// (so routes like `/` match), then patches History.prototype so all future
/// pushState/replaceState calls re-add the base (keeping the URL correct).
/// Also generates 404.html (copy of index.html) so GitHub Pages serves the
/// SPA shell for all sub-routes.
const SPA_ROUTING_SCRIPT: &str = r"<script>(function(){var B='__BASE__';var p=location.pathname;if(!p.startsWith(B))return;var R=History.prototype.replaceState,P=History.prototype.pushState;function a(u){return typeof u==='string'&&u.charAt(0)==='/'&&!u.startsWith(B)?B+u:u}History.prototype.pushState=function(s,t,u){return P.call(this,s,t,a(u))};History.prototype.replaceState=function(s,t,u){return R.call(this,s,t,a(u))};R.call(history,history.state,'',p.slice(B.length)||'/')})()</script>";

/// Rewrite the build command for GitHub Pages project sites.
///
/// Handles three concerns: (1) asset base paths via framework-specific flags,
/// (2) SPA routing via an injected script that patches History.prototype,
/// (3) 404.html fallback so GitHub Pages serves the SPA for all sub-routes.
pub fn with_base_path(build_config: &mut BuildConfig, repo_name: &str) {
    let base = format!("/{repo_name}");
    let output_dir = &build_config.output_dir;

    let clean = format!("rm -rf {output_dir}");

    // Inject SPA routing script into <head> of index.html, then copy to 404.html.
    // Write script to a temp file to avoid shell quoting issues (the script
    // contains single quotes from JavaScript === operators that break sed).
    // The heredoc terminator must be alone on its line, so we use \n instead of &&
    // between the heredoc and the subsequent node command.
    let script = SPA_ROUTING_SCRIPT.replace("__BASE__", &base);
    let node_inject = format!(
        "node -e \"var f=require('fs');var s=f.readFileSync('/tmp/_spa_inject.txt','utf8').trim();\
         var p='{output_dir}/index.html';var h=f.readFileSync(p,'utf8');\
         f.writeFileSync(p,h.replace('<head>','<head>'+s));\
         f.copyFileSync(p,'{output_dir}/404.html')\" && rm -f /tmp/_spa_inject.txt"
    );
    let inject = format!("cat > /tmp/_spa_inject.txt << 'SPAEOF'\n{script}\nSPAEOF\n{node_inject}");

    let base_slash = format!("{base}/");
    match build_config.framework {
        Framework::Vite => {
            build_config.build_cmd = build_config.build_cmd.replace(
                "npx vite build",
                &format!("npx vite build --base={base_slash}"),
            );
        }
        Framework::Cra => {
            let cmd = build_config.build_cmd.clone();
            build_config.build_cmd = format!("PUBLIC_URL={base_slash} {cmd}");
        }
        Framework::NextJs | Framework::Unknown => {}
    }

    let cmd = &build_config.build_cmd;
    build_config.build_cmd = format!("{clean} && {cmd} && {inject}");
}

/// Run the build command in a temporary container and return the result.
pub async fn run_build(
    runtime: &dyn Runtime,
    project_id: &str,
    repo_path: &Path,
    build_config: &BuildConfig,
) -> Result<BuildResult, AppError> {
    // Check runtime is reachable (VM running on macOS, Podman available on Linux)
    if !runtime.is_reachable().await {
        return Err(PublishError::RuntimeNotReady.into());
    }

    let build_id = format!("build-{project_id}");
    let result = runtime
        .run_to_completion(&build_id, repo_path, &build_config.build_cmd)
        .await?;

    if result.exit_code != 0 {
        // Extract the last few lines of logs for the error message
        let tail: String = result
            .logs
            .lines()
            .rev()
            .take(10)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join("\n");

        return Err(PublishError::BuildFailed {
            tail: format!("(exit code {}):\n{tail}", result.exit_code),
        }
        .into());
    }

    // Verify build output exists
    let output_path = repo_path.join(&build_config.output_dir);
    if !output_path.is_dir() {
        // For Unknown framework, try alternative output dirs
        if build_config.framework == Framework::Unknown {
            for alt in &["build", "out", "dist", "public"] {
                let alt_path = repo_path.join(alt);
                if alt_path.is_dir() {
                    tracing::info!("build output found in alternative directory: {alt}");
                    // Return success, deployer will use this path
                    return Ok(result);
                }
            }
        }

        return Err(PublishError::BuildOutputMissing {
            expected: build_config.output_dir.clone(),
        }
        .into());
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn detect_vite_framework() {
        let dir = tempfile::tempdir().expect("cannot create temp dir");
        fs::write(
            dir.path().join("package.json"),
            r#"{"dependencies": {"vite": "^5.0.0"}}"#,
        )
        .expect("cannot write");
        let config = detect_framework(dir.path()).expect("should detect vite");
        assert_eq!(config.framework, Framework::Vite);
        assert_eq!(config.output_dir, "dist");
    }

    #[test]
    fn detect_cra_framework() {
        let dir = tempfile::tempdir().expect("cannot create temp dir");
        fs::write(
            dir.path().join("package.json"),
            r#"{"dependencies": {"react-scripts": "5.0.0"}}"#,
        )
        .expect("cannot write");
        let config = detect_framework(dir.path()).expect("should detect cra");
        assert_eq!(config.framework, Framework::Cra);
        assert_eq!(config.output_dir, "build");
    }

    #[test]
    fn detect_nextjs_without_export_fails() {
        let dir = tempfile::tempdir().expect("cannot create temp dir");
        fs::write(
            dir.path().join("package.json"),
            r#"{"dependencies": {"next": "14.0.0"}}"#,
        )
        .expect("cannot write");
        fs::write(dir.path().join("next.config.js"), "module.exports = {}").expect("cannot write");
        let result = detect_framework(dir.path());
        assert!(result.is_err());
        let err = result.expect_err("should be error");
        assert!(err.to_string().contains("static export"));
    }

    #[test]
    fn detect_nextjs_with_export() {
        let dir = tempfile::tempdir().expect("cannot create temp dir");
        fs::write(
            dir.path().join("package.json"),
            r#"{"dependencies": {"next": "14.0.0"}}"#,
        )
        .expect("cannot write");
        fs::write(
            dir.path().join("next.config.js"),
            "module.exports = { output: 'export' }",
        )
        .expect("cannot write");
        let config = detect_framework(dir.path()).expect("should detect nextjs");
        assert_eq!(config.framework, Framework::NextJs);
        assert_eq!(config.output_dir, "out");
    }

    #[test]
    fn detect_unknown_with_build_script() {
        let dir = tempfile::tempdir().expect("cannot create temp dir");
        fs::write(
            dir.path().join("package.json"),
            r#"{"scripts": {"build": "tsc && rollup"}}"#,
        )
        .expect("cannot write");
        let config = detect_framework(dir.path()).expect("should detect unknown");
        assert_eq!(config.framework, Framework::Unknown);
    }

    #[test]
    fn detect_no_package_json_fails() {
        let dir = tempfile::tempdir().expect("cannot create temp dir");
        let result = detect_framework(dir.path());
        assert!(result.is_err());
        let err = result.expect_err("should be error");
        assert!(err.to_string().contains("package.json"));
    }

    #[test]
    fn detect_no_build_script_fails() {
        let dir = tempfile::tempdir().expect("cannot create temp dir");
        fs::write(
            dir.path().join("package.json"),
            r#"{"dependencies": {"lodash": "4.0.0"}}"#,
        )
        .expect("cannot write");
        let result = detect_framework(dir.path());
        assert!(result.is_err());
    }

    #[test]
    fn with_base_path_heredoc_terminator_on_own_line() {
        let mut config = BuildConfig {
            framework: Framework::Vite,
            build_cmd: "npx vite build".into(),
            output_dir: "dist".into(),
        };
        with_base_path(&mut config, "my-repo");
        // The heredoc terminator SPAEOF must appear on its own line,
        // not followed by && or any other command on the same line.
        // Lines starting with "cat" or containing "<< 'SPAEOF'" are the heredoc
        // opener, not the terminator.
        for line in config.build_cmd.lines() {
            let trimmed = line.trim();
            let is_terminator_line = trimmed.starts_with("SPAEOF");
            if is_terminator_line {
                assert_eq!(
                    trimmed, "SPAEOF",
                    "heredoc terminator must be alone on its line, got: {line}"
                );
            }
        }
    }
}
