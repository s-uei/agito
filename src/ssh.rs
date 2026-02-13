use anyhow::{Context, Result};
use async_trait::async_trait;
use russh::server::{Auth, Msg, Session};
use russh::{Channel, ChannelId};
use russh_keys::key;
use std::fs;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::AsyncReadExt;
use tokio::process::Command;

pub struct Server {
    port: String,
    host_key_path: PathBuf,
    authorized_keys_path: PathBuf,
    repos_dir: PathBuf,
}

impl Server {
    pub fn new(
        port: String,
        host_key_path: PathBuf,
        authorized_keys_path: PathBuf,
        repos_dir: PathBuf,
    ) -> Self {
        Self {
            port,
            host_key_path,
            authorized_keys_path,
            repos_dir,
        }
    }

    pub async fn start(self) -> Result<()> {
        let host_key = self.get_host_key().await?;

        let config = russh::server::Config {
            inactivity_timeout: Some(std::time::Duration::from_secs(3600)),
            auth_rejection_time: std::time::Duration::from_secs(3),
            auth_rejection_time_initial: Some(std::time::Duration::from_secs(0)),
            keys: vec![host_key],
            ..Default::default()
        };

        let config = Arc::new(config);

        let addr = format!("0.0.0.0:{}", self.port);
        tracing::info!("SSH server listening on {}", addr);

        // Start listening manually
        let listener = tokio::net::TcpListener::bind(&addr).await?;
        
        let repos_dir = Arc::new(self.repos_dir);
        let authorized_keys_path = Arc::new(self.authorized_keys_path);
        
        loop {
            let (stream, _addr) = listener.accept().await?;
            let config = config.clone();
            let repos_dir = repos_dir.clone();
            let authorized_keys_path = authorized_keys_path.clone();
            
            tokio::spawn(async move {
                let handler = SessionHandler {
                    repos_dir: (*repos_dir).clone(),
                    authorized_keys_path: (*authorized_keys_path).clone(),
                };
                let session = russh::server::run_stream(config, stream, handler).await;
                if let Err(e) = session {
                    tracing::error!("Session error: {}", e);
                }
            });
        }
    }

    async fn get_host_key(&self) -> Result<key::KeyPair> {
        // Check if host key exists
        if !self.host_key_path.exists() {
            // Generate new host key
            tracing::info!("Generating new SSH host key at {:?}", self.host_key_path);

            let status = Command::new("ssh-keygen")
                .arg("-t")
                .arg("rsa")
                .arg("-b")
                .arg("4096")
                .arg("-f")
                .arg(&self.host_key_path)
                .arg("-N")
                .arg("")
                .status()
                .await
                .context("Failed to generate host key")?;

            if !status.success() {
                anyhow::bail!("Failed to generate host key");
            }
        }

        // Load host key
        let key_data = fs::read(&self.host_key_path).context("Failed to read host key")?;
        let key = russh_keys::decode_secret_key(&String::from_utf8_lossy(&key_data), None)
            .context("Failed to parse host key")?;

        Ok(key)
    }
}

struct SessionHandler {
    repos_dir: PathBuf,
    authorized_keys_path: PathBuf,
}

#[async_trait]
impl russh::server::Handler for SessionHandler {
    type Error = anyhow::Error;

    async fn auth_publickey(
        &mut self,
        user: &str,
        public_key: &key::PublicKey,
    ) -> Result<Auth, Self::Error> {
        tracing::info!("Public key auth attempt for user: {}", user);

        // Read authorized keys
        if !self.authorized_keys_path.exists() {
            return Ok(Auth::Reject {
                proceed_with_methods: None,
            });
        }

        let auth_keys = fs::read_to_string(&self.authorized_keys_path)?;

        for line in auth_keys.lines() {
            if line.trim().is_empty() || line.starts_with('#') {
                continue;
            }

            if let Ok(auth_key) = russh_keys::parse_public_key_base64(line) {
                if &auth_key == public_key {
                    tracing::info!("User {} authenticated successfully", user);
                    return Ok(Auth::Accept);
                }
            }
        }

        Ok(Auth::Reject {
            proceed_with_methods: None,
        })
    }

    async fn channel_open_session(
        &mut self,
        _channel: Channel<Msg>,
        _session: &mut Session,
    ) -> Result<bool, Self::Error> {
        Ok(true)
    }

    async fn exec_request(
        &mut self,
        channel: ChannelId,
        data: &[u8],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        let command = String::from_utf8_lossy(data);
        tracing::info!("Executing command: {}", command);

        if command.starts_with("git-upload-pack") || command.starts_with("git-receive-pack") {
            self.handle_git_command(channel, &command, session).await?;
        } else if command.starts_with("agito-create-repo") {
            self.handle_create_repo(channel, &command, session).await?;
        } else {
            let msg = format!("Unknown command: {}\n", command);
            session.data(channel, msg.into_bytes().into());
            session.exit_status_request(channel, 1);
            session.eof(channel);
            session.close(channel);
        }

        Ok(())
    }
}

impl SessionHandler {
    async fn handle_git_command(
        &mut self,
        channel: ChannelId,
        command: &str,
        session: &mut Session,
    ) -> Result<()> {
        let parts: Vec<&str> = command.split_whitespace().collect();
        if parts.len() < 2 {
            session.data(channel, b"Invalid git command\n".to_vec().into());
            session.exit_status_request(channel, 1);
            session.eof(channel);
            session.close(channel);
            return Ok(());
        }

        let git_cmd = parts[0];
        let repo_path = parts[1].trim_matches('\'').trim_matches('"');

        // Clean and validate repo path
        let repo_path = repo_path.trim_start_matches('/');
        let full_path = self.repos_dir.join(repo_path);

        // Security check: ensure path is within repos_dir
        if !full_path.starts_with(&self.repos_dir) {
            session.data(channel, b"Invalid repository path\n".to_vec().into());
            session.exit_status_request(channel, 1);
            session.eof(channel);
            session.close(channel);
            return Ok(());
        }

        // Check if repository exists
        if !full_path.exists() {
            let msg = format!("Repository not found: {}\n", repo_path);
            session.data(channel, msg.into_bytes().into());
            session.exit_status_request(channel, 1);
            session.eof(channel);
            session.close(channel);
            return Ok(());
        }

        // Execute git command
        let mut child = Command::new(git_cmd)
            .arg(&full_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let _stdin = child.stdin.take().unwrap();
        let mut stdout = child.stdout.take().unwrap();
        let mut stderr = child.stderr.take().unwrap();

        // NOTE: This is a simplified implementation. In a production system,
        // stdin should be bidirectionally connected to the SSH channel to support
        // git push operations. The original Go implementation had a similar limitation.

        // Forward stdout from git process to SSH channel
        let mut buf = vec![0u8; 8192];
        loop {
            match stdout.read(&mut buf).await {
                Ok(0) => break,
                Ok(n) => {
                    session.data(channel, buf[..n].to_vec().into());
                }
                Err(_) => break,
            }
        }

        // Forward stderr
        let mut buf = vec![0u8; 8192];
        loop {
            match stderr.read(&mut buf).await {
                Ok(0) => break,
                Ok(n) => {
                    session.data(channel, buf[..n].to_vec().into());
                }
                Err(_) => break,
            }
        }

        let status = child.wait().await?;
        let exit_code = status.code().unwrap_or(1);
        session.exit_status_request(channel, exit_code as u32);
        session.eof(channel);
        session.close(channel);

        Ok(())
    }

    async fn handle_create_repo(
        &mut self,
        channel: ChannelId,
        command: &str,
        session: &mut Session,
    ) -> Result<()> {
        let parts: Vec<&str> = command.split_whitespace().collect();
        if parts.len() < 2 {
            session.data(channel, b"Usage: agito-create-repo <repo-name>\n".to_vec().into());
            session.exit_status_request(channel, 1);
            session.eof(channel);
            session.close(channel);
            return Ok(());
        }

        let mut repo_name = parts[1].to_string();

        // Ensure repo name ends with .git
        if !repo_name.ends_with(".git") {
            repo_name.push_str(".git");
        }

        // Validate repo name
        if repo_name.contains("..") || repo_name.contains('/') {
            session.data(channel, b"Invalid repository name\n".to_vec().into());
            session.exit_status_request(channel, 1);
            session.eof(channel);
            session.close(channel);
            return Ok(());
        }

        let repo_path = self.repos_dir.join(&repo_name);

        // Check if repository already exists
        if repo_path.exists() {
            let msg = format!("Repository already exists: {}\n", repo_name);
            session.data(channel, msg.into_bytes().into());
            session.exit_status_request(channel, 1);
            session.eof(channel);
            session.close(channel);
            return Ok(());
        }

        // Create the repository
        if let Err(e) = crate::git::init_bare_repo(&repo_path) {
            let msg = format!("Failed to create repository: {}\n", e);
            session.data(channel, msg.into_bytes().into());
            session.exit_status_request(channel, 1);
            session.eof(channel);
            session.close(channel);
            return Ok(());
        }

        let msg = format!("Repository created: {}\n", repo_name);
        tracing::info!("Created repository: {:?}", repo_path);
        session.data(channel, msg.into_bytes().into());
        session.exit_status_request(channel, 0);
        session.eof(channel);
        session.close(channel);

        Ok(())
    }
}

