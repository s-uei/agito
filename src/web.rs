use anyhow::Result;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    routing::get,
    Router,
};
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use tower_http::services::ServeDir;

#[derive(Clone)]
pub struct WebServer {
    repos_dir: PathBuf,
}

pub struct Repository {
    name: String,
    path: PathBuf,
    description: String,
    last_commit: String,
    branches: Vec<String>,
}

impl WebServer {
    pub fn new(repos_dir: PathBuf) -> Self {
        Self { repos_dir }
    }

    pub async fn start(self, port: &str) -> Result<()> {
        let app = Router::new()
            .route("/", get(handle_index))
            .route("/repo/:name", get(handle_repo))
            .route("/repo/:name/*path", get(handle_repo))
            .nest_service("/static", ServeDir::new("web/static"))
            .with_state(Arc::new(self));

        let addr = format!("0.0.0.0:{}", port);
        tracing::info!("Web server listening on {}", addr);
        tracing::info!("Visit http://localhost:{} to view repositories", port);

        let listener = tokio::net::TcpListener::bind(&addr).await?;
        axum::serve(listener, app).await?;

        Ok(())
    }

    fn list_repositories(&self) -> Result<Vec<Repository>> {
        let mut repos = Vec::new();

        let entries = fs::read_dir(&self.repos_dir)?;

        for entry in entries {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }

            let repo_path = entry.path();

            // Check if it's a git repository
            if !repo_path.join("HEAD").exists() {
                continue;
            }

            let mut repo = Repository {
                name: entry.file_name().to_string_lossy().to_string(),
                path: repo_path.clone(),
                description: String::new(),
                last_commit: String::new(),
                branches: Vec::new(),
            };

            // Get description
            let desc_path = repo_path.join("description");
            if let Ok(desc) = fs::read_to_string(&desc_path) {
                let desc = desc.trim().to_string();
                if desc != "Unnamed repository; edit this file 'description' to name the repository."
                {
                    repo.description = desc;
                }
            }

            // Get last commit info
            let output = Command::new("git")
                .arg("-C")
                .arg(&repo_path)
                .arg("log")
                .arg("-1")
                .arg("--format=%h - %s (%cr)")
                .output();

            if let Ok(output) = output {
                if output.status.success() {
                    repo.last_commit = String::from_utf8_lossy(&output.stdout).trim().to_string();
                }
            }

            repos.push(repo);
        }

        Ok(repos)
    }

    fn get_branches(&self, repo_path: &PathBuf) -> Result<Vec<String>> {
        let output = Command::new("git")
            .arg("-C")
            .arg(repo_path)
            .arg("branch")
            .arg("-a")
            .output()?;

        if !output.status.success() {
            return Ok(Vec::new());
        }

        let branches: Vec<String> = String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(|line| {
                line.trim()
                    .trim_start_matches("* ")
                    .to_string()
            })
            .filter(|line| !line.is_empty() && !line.contains("->"))
            .collect();

        Ok(branches)
    }

    fn get_commits(&self, repo_path: &PathBuf, limit: usize) -> Result<Vec<CommitInfo>> {
        let output = Command::new("git")
            .arg("-C")
            .arg(repo_path)
            .arg("log")
            .arg(format!("--max-count={}", limit))
            .arg("--format=%H|%an|%ar|%s")
            .output()?;

        if !output.status.success() {
            return Ok(Vec::new());
        }

        let commits: Vec<CommitInfo> = String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter_map(|line| {
                let parts: Vec<&str> = line.splitn(4, '|').collect();
                if parts.len() == 4 {
                    Some(CommitInfo {
                        hash: parts[0][..8.min(parts[0].len())].to_string(),
                        author: parts[1].to_string(),
                        date: parts[2].to_string(),
                        message: parts[3].to_string(),
                    })
                } else {
                    None
                }
            })
            .collect();

        Ok(commits)
    }

    fn list_files(&self, repo_path: &PathBuf, branch: &str, path: &str) -> Result<Vec<FileInfo>> {
        let tree_path = format!("{}:{}", branch, path);
        let output = Command::new("git")
            .arg("-C")
            .arg(repo_path)
            .arg("ls-tree")
            .arg(&tree_path)
            .output()?;

        if !output.status.success() {
            return Ok(Vec::new());
        }

        let files: Vec<FileInfo> = String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter_map(|line| {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 4 {
                    let file_type = parts[1];
                    let name = parts[3..].join(" ");
                    let file_path = if path.is_empty() {
                        name.clone()
                    } else {
                        format!("{}/{}", path, name)
                    };
                    Some(FileInfo {
                        name,
                        file_type: file_type.to_string(),
                        path: file_path,
                        size: 0,
                    })
                } else {
                    None
                }
            })
            .collect();

        Ok(files)
    }

    fn get_file_content(&self, repo_path: &PathBuf, branch: &str, path: &str) -> Result<String> {
        let blob_path = format!("{}:{}", branch, path);
        let output = Command::new("git")
            .arg("-C")
            .arg(repo_path)
            .arg("show")
            .arg(&blob_path)
            .output()?;

        if !output.status.success() {
            anyhow::bail!("Failed to get file content");
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    fn get_readme(&self, repo_path: &PathBuf, branch: &str) -> Option<String> {
        let readme_names = ["README.md", "README", "Readme.md", "readme.md"];

        for name in &readme_names {
            if let Ok(content) = self.get_file_content(repo_path, branch, name) {
                return Some(content);
            }
        }

        None
    }
}

struct CommitInfo {
    hash: String,
    author: String,
    date: String,
    message: String,
}

struct FileInfo {
    name: String,
    file_type: String,
    path: String,
    size: i64,
}

async fn handle_index(State(server): State<Arc<WebServer>>) -> Response {
    match server.list_repositories() {
        Ok(repos) => {
            let mut html = String::from(r#"<!DOCTYPE html>
<html>
<head>
    <title>Agito - Git Repositories</title>
    <style>
        body { font-family: Arial, sans-serif; margin: 40px; }
        h1 { color: #333; }
        .repo-list { margin-top: 30px; }
        .repo-item { 
            border: 1px solid #ddd; 
            padding: 20px; 
            margin-bottom: 15px; 
            border-radius: 5px;
            background: #f9f9f9;
        }
        .repo-item h2 { margin: 0 0 10px 0; color: #0066cc; }
        .repo-item a { text-decoration: none; }
        .repo-desc { color: #666; margin: 10px 0; }
        .repo-meta { color: #888; font-size: 0.9em; }
    </style>
</head>
<body>
    <h1>Agito - Git Repositories</h1>
    <div class="repo-list">
"#);

            for repo in repos {
                html.push_str(&format!(
                    r#"
        <div class="repo-item">
            <h2><a href="/repo/{}">{}</a></h2>
            <div class="repo-desc">{}</div>
            <div class="repo-meta">{}</div>
        </div>
"#,
                    repo.name, repo.name, repo.description, repo.last_commit
                ));
            }

            html.push_str(
                r#"
    </div>
</body>
</html>
"#,
            );

            Html(html).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Error listing repositories: {}", e),
        )
            .into_response(),
    }
}

async fn handle_repo(
    State(server): State<Arc<WebServer>>,
    Path(params): Path<String>,
) -> Response {
    let parts: Vec<&str> = params.split('/').collect();
    let repo_name = parts[0];
    let repo_path = server.repos_dir.join(repo_name);

    if !repo_path.exists() {
        return (StatusCode::NOT_FOUND, "Repository not found").into_response();
    }

    // Get branches
    let branches = server.get_branches(&repo_path).unwrap_or_default();
    let branch = branches.first().unwrap_or(&"master".to_string()).clone();

    // Get description
    let desc_path = repo_path.join("description");
    let description = fs::read_to_string(&desc_path)
        .unwrap_or_default()
        .trim()
        .to_string();
    let description = if description
        == "Unnamed repository; edit this file 'description' to name the repository."
    {
        String::new()
    } else {
        description
    };

    // Get commits
    let commits = server.get_commits(&repo_path, 10).unwrap_or_default();

    // Get files
    let files = if parts.len() > 1 {
        Vec::new()
    } else {
        server.list_files(&repo_path, &branch, "").unwrap_or_default()
    };

    // Try to get README
    let readme = server.get_readme(&repo_path, &branch).unwrap_or_default();

    let mut html = format!(
        r#"<!DOCTYPE html>
<html>
<head>
    <title>Agito - {}</title>
    <style>
        body {{ font-family: Arial, sans-serif; margin: 40px; }}
        h1 {{ color: #333; }}
        .section {{ margin: 30px 0; }}
        .section h2 {{ color: #0066cc; border-bottom: 2px solid #0066cc; padding-bottom: 5px; }}
        .file-list, .commit-list {{ list-style: none; padding: 0; }}
        .file-item, .commit-item {{ 
            padding: 10px; 
            border-bottom: 1px solid #eee; 
        }}
        .file-item:hover, .commit-item:hover {{ background: #f5f5f5; }}
        .breadcrumb {{ color: #666; margin-bottom: 20px; }}
        pre {{ background: #f5f5f5; padding: 15px; border-radius: 5px; overflow-x: auto; }}
    </style>
</head>
<body>
    <div class="breadcrumb">
        <a href="/">Home</a> / {}
    </div>
    <h1>{}</h1>
    <p>{}</p>
"#,
        repo_name, repo_name, repo_name, description
    );

    if !files.is_empty() {
        html.push_str(r#"<div class="section"><h2>Files</h2><ul class="file-list">"#);
        for file in files {
            html.push_str(&format!(
                r#"<li class="file-item">{} - {}</li>"#,
                file.name, file.file_type
            ));
        }
        html.push_str("</ul></div>");
    }

    if !readme.is_empty() {
        html.push_str(&format!(
            r#"<div class="section"><h2>README</h2><pre>{}</pre></div>"#,
            html_escape(&readme)
        ));
    }

    if !commits.is_empty() {
        html.push_str(r#"<div class="section"><h2>Recent Commits</h2><ul class="commit-list">"#);
        for commit in commits {
            html.push_str(&format!(
                r#"<li class="commit-item"><strong>{}</strong> - {} <br/><small>{} by {}</small></li>"#,
                commit.hash, html_escape(&commit.message), commit.date, html_escape(&commit.author)
            ));
        }
        html.push_str("</ul></div>");
    }

    html.push_str("</body></html>");

    Html(html).into_response()
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
