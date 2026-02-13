package main

import (
	"fmt"
	"os"
	"os/exec"

	"github.com/s-uei/agito/internal/git"
)

func main() {
	if len(os.Args) < 2 {
		printUsage()
		os.Exit(1)
	}

	command := os.Args[1]

	switch command {
	case "clone":
		handleClone(os.Args[2:])
	case "create":
		handleCreate(os.Args[2:])
	case "help", "--help", "-h":
		printUsage()
	default:
		// Pass through to git for standard git commands
		passToGit(os.Args[1:])
	}
}

func printUsage() {
	usage := `agito - A simple git alternative with integrated hosting

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
`
	fmt.Println(usage)
}

func handleClone(args []string) {
	if len(args) < 1 {
		fmt.Println("Error: clone requires a repository URL")
		os.Exit(1)
	}

	url := args[0]
	if err := git.Clone(url, args[1:]...); err != nil {
		fmt.Fprintf(os.Stderr, "Error cloning repository: %v\n", err)
		os.Exit(1)
	}
}

func handleCreate(args []string) {
	if len(args) < 1 {
		fmt.Println("Error: create requires a repository name")
		os.Exit(1)
	}

	repoName := args[0]
	
	// Get server from environment or use default
	server := os.Getenv("AGITO_SERVER")
	if server == "" {
		server = "localhost:2222"
	}

	user := os.Getenv("AGITO_USER")
	if user == "" {
		user = "git"
	}

	if err := git.CreateRemoteRepo(server, user, repoName); err != nil {
		fmt.Fprintf(os.Stderr, "Error creating repository: %v\n", err)
		os.Exit(1)
	}

	fmt.Printf("Repository '%s' created successfully on %s\n", repoName, server)
	fmt.Printf("Clone it with: agito clone ssh://%s@%s/%s\n", user, server, repoName)
}

func passToGit(args []string) {
	cmd := exec.Command("git", args...)
	cmd.Stdout = os.Stdout
	cmd.Stderr = os.Stderr
	cmd.Stdin = os.Stdin

	if err := cmd.Run(); err != nil {
		if exitErr, ok := err.(*exec.ExitError); ok {
			os.Exit(exitErr.ExitCode())
		}
		fmt.Fprintf(os.Stderr, "Error executing git command: %v\n", err)
		os.Exit(1)
	}
}
