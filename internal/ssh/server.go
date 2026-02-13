package ssh

import (
	"bytes"
	"fmt"
	"log"
	"net"
	"os"
	"os/exec"
	"path/filepath"
	"strings"

	"github.com/s-uei/agito/internal/git"
	gossh "golang.org/x/crypto/ssh"
)

type Server struct {
	port           string
	hostKeyPath    string
	authorizedKeys string
	reposDir       string
	listener       net.Listener
}

func NewServer(port, hostKeyPath, authorizedKeys, reposDir string) *Server {
	return &Server{
		port:           port,
		hostKeyPath:    hostKeyPath,
		authorizedKeys: authorizedKeys,
		reposDir:       reposDir,
	}
}

func (s *Server) Start() error {
	// Load or generate host key
	hostKey, err := s.getHostKey()
	if err != nil {
		return fmt.Errorf("failed to load host key: %w", err)
	}

	config := &gossh.ServerConfig{
		PublicKeyCallback: s.authCallback,
	}
	config.AddHostKey(hostKey)

	// Start listening
	listener, err := net.Listen("tcp", ":"+s.port)
	if err != nil {
		return fmt.Errorf("failed to listen on port %s: %w", s.port, err)
	}
	s.listener = listener

	log.Printf("SSH server listening on port %s", s.port)

	for {
		conn, err := listener.Accept()
		if err != nil {
			log.Printf("Failed to accept connection: %v", err)
			continue
		}

		go s.handleConnection(conn, config)
	}
}

func (s *Server) Stop() {
	if s.listener != nil {
		s.listener.Close()
	}
}

func (s *Server) getHostKey() (gossh.Signer, error) {
	// Check if host key exists
	if _, err := os.Stat(s.hostKeyPath); os.IsNotExist(err) {
		// Generate new host key
		log.Printf("Generating new SSH host key at %s", s.hostKeyPath)
		cmd := exec.Command("ssh-keygen", "-t", "rsa", "-b", "4096", "-f", s.hostKeyPath, "-N", "")
		if err := cmd.Run(); err != nil {
			return nil, fmt.Errorf("failed to generate host key: %w", err)
		}
	}

	// Load host key
	keyBytes, err := os.ReadFile(s.hostKeyPath)
	if err != nil {
		return nil, fmt.Errorf("failed to read host key: %w", err)
	}

	key, err := gossh.ParsePrivateKey(keyBytes)
	if err != nil {
		return nil, fmt.Errorf("failed to parse host key: %w", err)
	}

	return key, nil
}

func (s *Server) authCallback(conn gossh.ConnMetadata, key gossh.PublicKey) (*gossh.Permissions, error) {
	// Read authorized keys
	if _, err := os.Stat(s.authorizedKeys); os.IsNotExist(err) {
		log.Printf("Authorized keys file not found, creating empty file at %s", s.authorizedKeys)
		if err := os.WriteFile(s.authorizedKeys, []byte{}, 0600); err != nil {
			return nil, fmt.Errorf("failed to create authorized keys file: %w", err)
		}
	}

	authKeys, err := os.ReadFile(s.authorizedKeys)
	if err != nil {
		return nil, fmt.Errorf("failed to read authorized keys: %w", err)
	}

	// Parse authorized keys
	for len(authKeys) > 0 {
		pubKey, _, _, rest, err := gossh.ParseAuthorizedKey(authKeys)
		if err != nil {
			break
		}
		authKeys = rest

		if bytes.Equal(key.Marshal(), pubKey.Marshal()) {
			log.Printf("User %s authenticated successfully", conn.User())
			return nil, nil
		}
	}

	return nil, fmt.Errorf("public key not found in authorized keys")
}

func (s *Server) handleConnection(conn net.Conn, config *gossh.ServerConfig) {
	defer conn.Close()

	sshConn, chans, reqs, err := gossh.NewServerConn(conn, config)
	if err != nil {
		log.Printf("Failed to handshake: %v", err)
		return
	}
	defer sshConn.Close()

	log.Printf("New SSH connection from %s (%s)", sshConn.RemoteAddr(), sshConn.User())

	// Discard all global requests
	go gossh.DiscardRequests(reqs)

	// Handle channels
	for newChannel := range chans {
		if newChannel.ChannelType() != "session" {
			newChannel.Reject(gossh.UnknownChannelType, "unknown channel type")
			continue
		}

		channel, requests, err := newChannel.Accept()
		if err != nil {
			log.Printf("Failed to accept channel: %v", err)
			continue
		}

		go s.handleChannel(channel, requests, sshConn.User())
	}
}

func (s *Server) handleChannel(channel gossh.Channel, requests <-chan *gossh.Request, user string) {
	defer channel.Close()

	for req := range requests {
		switch req.Type {
		case "exec":
			s.handleExec(channel, req, user)
		case "shell":
			// Reject shell requests
			if req.WantReply {
				req.Reply(false, nil)
			}
		default:
			if req.WantReply {
				req.Reply(false, nil)
			}
		}
	}
}

func (s *Server) handleExec(channel gossh.Channel, req *gossh.Request, user string) {
	if len(req.Payload) < 4 {
		if req.WantReply {
			req.Reply(false, nil)
		}
		return
	}

	// Extract command from payload
	cmdLen := int(req.Payload[3])
	if len(req.Payload) < 4+cmdLen {
		if req.WantReply {
			req.Reply(false, nil)
		}
		return
	}

	command := string(req.Payload[4 : 4+cmdLen])
	log.Printf("Executing command: %s", command)

	if req.WantReply {
		req.Reply(true, nil)
	}

	// Handle git commands
	if strings.HasPrefix(command, "git-upload-pack") || strings.HasPrefix(command, "git-receive-pack") {
		s.handleGitCommand(channel, command)
	} else if strings.HasPrefix(command, "agito-create-repo") {
		s.handleCreateRepo(channel, command)
	} else {
		fmt.Fprintf(channel, "Unknown command: %s\n", command)
		channel.SendRequest("exit-status", false, []byte{0, 0, 0, 1})
	}
}

func (s *Server) handleGitCommand(channel gossh.Channel, command string) {
	// Parse the command to extract repository path
	parts := strings.Fields(command)
	if len(parts) < 2 {
		fmt.Fprintf(channel, "Invalid git command\n")
		channel.SendRequest("exit-status", false, []byte{0, 0, 0, 1})
		return
	}

	gitCmd := parts[0]
	repoPath := strings.Trim(parts[1], "'\"")
	
	// Clean and validate repo path
	repoPath = filepath.Clean(repoPath)
	if strings.HasPrefix(repoPath, "/") {
		repoPath = strings.TrimPrefix(repoPath, "/")
	}
	
	fullPath := filepath.Join(s.reposDir, repoPath)
	fullPath = filepath.Clean(fullPath)

	// Ensure the path is within reposDir using filepath.Rel for security
	cleanReposDir := filepath.Clean(s.reposDir)
	relPath, err := filepath.Rel(cleanReposDir, fullPath)
	if err != nil || strings.HasPrefix(relPath, "..") {
		fmt.Fprintf(channel, "Invalid repository path\n")
		channel.SendRequest("exit-status", false, []byte{0, 0, 0, 1})
		return
	}

	// Check if repository exists
	if _, err := os.Stat(fullPath); os.IsNotExist(err) {
		fmt.Fprintf(channel, "Repository not found: %s\n", repoPath)
		channel.SendRequest("exit-status", false, []byte{0, 0, 0, 1})
		return
	}

	// Execute git command
	cmd := exec.Command(gitCmd, fullPath)
	cmd.Stdin = channel
	cmd.Stdout = channel
	cmd.Stderr = channel

	if err := cmd.Run(); err != nil {
		log.Printf("Git command failed: %v", err)
		channel.SendRequest("exit-status", false, []byte{0, 0, 0, 1})
		return
	}

	channel.SendRequest("exit-status", false, []byte{0, 0, 0, 0})
}

func (s *Server) handleCreateRepo(channel gossh.Channel, command string) {
	parts := strings.Fields(command)
	if len(parts) < 2 {
		fmt.Fprintf(channel, "Usage: agito-create-repo <repo-name>\n")
		channel.SendRequest("exit-status", false, []byte{0, 0, 0, 1})
		return
	}

	repoName := parts[1]
	
	// Ensure repo name ends with .git
	if !strings.HasSuffix(repoName, ".git") {
		repoName = repoName + ".git"
	}

	// Validate repo name
	if strings.Contains(repoName, "..") || strings.Contains(repoName, "/") {
		fmt.Fprintf(channel, "Invalid repository name\n")
		channel.SendRequest("exit-status", false, []byte{0, 0, 0, 1})
		return
	}

	repoPath := filepath.Join(s.reposDir, repoName)

	// Check if repository already exists
	if _, err := os.Stat(repoPath); err == nil {
		fmt.Fprintf(channel, "Repository already exists: %s\n", repoName)
		channel.SendRequest("exit-status", false, []byte{0, 0, 0, 1})
		return
	}

	// Create the repository
	if err := git.InitBareRepo(repoPath); err != nil {
		fmt.Fprintf(channel, "Failed to create repository: %v\n", err)
		channel.SendRequest("exit-status", false, []byte{0, 0, 0, 1})
		return
	}

	fmt.Fprintf(channel, "Repository created: %s\n", repoName)
	log.Printf("Created repository: %s", repoPath)
	channel.SendRequest("exit-status", false, []byte{0, 0, 0, 0})
}
