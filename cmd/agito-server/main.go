package main

import (
	"flag"
	"fmt"
	"log"
	"os"
	"os/signal"
	"path/filepath"
	"syscall"

	"github.com/s-uei/agito/internal/server"
	"github.com/s-uei/agito/internal/ssh"
)

func main() {
	// Command line flags
	reposDir := flag.String("repos", "/var/lib/agito/repos", "Directory to store repositories")
	httpPort := flag.String("http-port", "3000", "HTTP port for web viewer")
	sshPort := flag.String("ssh-port", "2222", "SSH port for git operations")
	sshHostKey := flag.String("ssh-key", "/var/lib/agito/ssh/host_key", "SSH host key file")
	authorizedKeys := flag.String("authorized-keys", "/var/lib/agito/ssh/authorized_keys", "Authorized keys file")
	flag.Parse()

	// Create directories if they don't exist
	if err := os.MkdirAll(*reposDir, 0755); err != nil {
		log.Fatalf("Failed to create repos directory: %v", err)
	}

	keyDir := filepath.Dir(*sshHostKey)
	if err := os.MkdirAll(keyDir, 0700); err != nil {
		log.Fatalf("Failed to create SSH key directory: %v", err)
	}

	log.Printf("Agito Server Starting...")
	log.Printf("Repositories: %s", *reposDir)
	log.Printf("HTTP Port: %s", *httpPort)
	log.Printf("SSH Port: %s", *sshPort)

	// Start SSH server in a goroutine
	sshServer := ssh.NewServer(*sshPort, *sshHostKey, *authorizedKeys, *reposDir)
	go func() {
		if err := sshServer.Start(); err != nil {
			log.Fatalf("SSH server error: %v", err)
		}
	}()

	// Start HTTP server in a goroutine
	webServer := server.NewWebServer(*httpPort, *reposDir)
	go func() {
		if err := webServer.Start(); err != nil {
			log.Fatalf("Web server error: %v", err)
		}
	}()

	// Wait for interrupt signal
	sigChan := make(chan os.Signal, 1)
	signal.Notify(sigChan, os.Interrupt, syscall.SIGTERM)
	<-sigChan

	fmt.Println("\nShutting down...")
	sshServer.Stop()
	webServer.Stop()
}
