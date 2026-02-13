use agito::git;
use std::env;
use std::process::{Command, exit};

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        print_usage();
        exit(1);
    }

    let command = &args[1];

    match command.as_str() {
        "clone" => handle_clone(&args[2..]),
        "create" => handle_create(&args[2..]),
        "help" | "--help" | "-h" => print_usage(),
        _ => {
            // Pass through to git for standard git commands
            pass_to_git(&args[1..]);
        }
    }
}

fn print_usage() {
    let usage = r#"agito - A simple git alternative with integrated hosting

Usage:
  agito <command> [arguments]

Agito Commands:
  clone <url>              Clone a repository from agito server
  create <name>            Create a new bare repository on agito server
  help                     Show this help message

Git Commands:
  Any standard git command will be passed through to git
  Examples: agito status, agito commit -m "message", agito push, etc.

Examples:
  agito clone ssh://user@server/repo.git
  agito create myrepo
  agito status
  agito commit -m "Initial commit"
"#;
    println!("{}", usage);
}

fn handle_clone(args: &[String]) {
    if args.is_empty() {
        eprintln!("Error: clone requires a repository URL");
        exit(1);
    }

    let url = &args[0];
    let extra_args: Vec<String> = args[1..].to_vec();

    if let Err(e) = git::clone(url, &extra_args) {
        eprintln!("Error cloning repository: {}", e);
        exit(1);
    }
}

fn handle_create(args: &[String]) {
    if args.is_empty() {
        eprintln!("Error: create requires a repository name");
        exit(1);
    }

    let repo_name = &args[0];

    // Get server from environment or use default
    let server = env::var("AGITO_SERVER").unwrap_or_else(|_| "localhost:2222".to_string());
    let user = env::var("AGITO_USER").unwrap_or_else(|_| "git".to_string());

    if let Err(e) = git::create_remote_repo(&server, &user, repo_name) {
        eprintln!("Error creating repository: {}", e);
        exit(1);
    }

    println!("Repository '{}' created successfully on {}", repo_name, server);
    println!("Clone it with: agito clone ssh://{}@{}/{}", user, server, repo_name);
}

fn pass_to_git(args: &[String]) {
    let status = Command::new("git")
        .args(args)
        .status()
        .expect("Failed to execute git command");

    if !status.success() {
        exit(status.code().unwrap_or(1));
    }
}
