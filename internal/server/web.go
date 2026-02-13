package server

import (
	"context"
	"fmt"
	"html/template"
	"log"
	"net/http"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"time"
)

type WebServer struct {
	port     string
	reposDir string
	server   *http.Server
	tmpl     *template.Template
}

type Repository struct {
	Name        string
	Path        string
	Description string
	LastCommit  string
	Branches    []string
}

type RepoView struct {
	Repo     Repository
	Branches []string
	Files    []FileInfo
	Commits  []CommitInfo
	Content  string
	Readme   string
}

type FileInfo struct {
	Name  string
	Type  string
	Path  string
	Size  int64
}

type CommitInfo struct {
	Hash    string
	Author  string
	Date    string
	Message string
}

func NewWebServer(port, reposDir string) *WebServer {
	return &WebServer{
		port:     port,
		reposDir: reposDir,
	}
}

func (ws *WebServer) Start() error {
	// Parse templates
	ws.tmpl = template.Must(template.ParseGlob("web/templates/*.html"))

	// Set up routes
	mux := http.NewServeMux()
	mux.HandleFunc("/", ws.handleIndex)
	mux.HandleFunc("/repo/", ws.handleRepo)
	
	// Static files
	mux.Handle("/static/", http.StripPrefix("/static/", http.FileServer(http.Dir("web/static"))))

	ws.server = &http.Server{
		Addr:    ":" + ws.port,
		Handler: mux,
	}

	log.Printf("Web server listening on port %s", ws.port)
	log.Printf("Visit http://localhost:%s to view repositories", ws.port)

	return ws.server.ListenAndServe()
}

func (ws *WebServer) Stop() {
	if ws.server != nil {
		ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
		defer cancel()
		ws.server.Shutdown(ctx)
	}
}

func (ws *WebServer) handleIndex(w http.ResponseWriter, r *http.Request) {
	if r.URL.Path != "/" {
		http.NotFound(w, r)
		return
	}

	// List all repositories
	repos, err := ws.listRepositories()
	if err != nil {
		http.Error(w, fmt.Sprintf("Error listing repositories: %v", err), http.StatusInternalServerError)
		return
	}

	// Render template
	data := map[string]interface{}{
		"Title": "Agito - Git Repositories",
		"Repos": repos,
	}

	w.Header().Set("Content-Type", "text/html; charset=utf-8")
	if err := ws.tmpl.ExecuteTemplate(w, "index.html", data); err != nil {
		log.Printf("Template error: %v", err)
	}
}

func (ws *WebServer) handleRepo(w http.ResponseWriter, r *http.Request) {
	// Extract repo name from path
	path := strings.TrimPrefix(r.URL.Path, "/repo/")
	parts := strings.SplitN(path, "/", 2)
	
	if len(parts) == 0 || parts[0] == "" {
		http.NotFound(w, r)
		return
	}

	repoName := parts[0]
	repoPath := filepath.Join(ws.reposDir, repoName)

	// Check if repository exists
	if _, err := os.Stat(repoPath); os.IsNotExist(err) {
		http.NotFound(w, r)
		return
	}

	// Get repository info
	repo := Repository{
		Name: repoName,
		Path: repoPath,
	}

	// Get branches
	branches, _ := ws.getBranches(repoPath)
	repo.Branches = branches

	// Get description
	descPath := filepath.Join(repoPath, "description")
	if desc, err := os.ReadFile(descPath); err == nil {
		repo.Description = strings.TrimSpace(string(desc))
		if repo.Description == "Unnamed repository; edit this file 'description' to name the repository." {
			repo.Description = ""
		}
	}

	// Get commits
	commits, _ := ws.getCommits(repoPath, 10)

	// Get files (if we're viewing a specific path)
	var files []FileInfo
	var content string
	
	branch := "master"
	if len(branches) > 0 {
		branch = branches[0]
	}

	if len(parts) > 1 {
		// Viewing a specific file or directory
		filePath := parts[1]
		content, _ = ws.getFileContent(repoPath, branch, filePath)
	} else {
		// List root directory
		files, _ = ws.listFiles(repoPath, branch, "")
	}

	// Try to get README
	readme, _ := ws.getReadme(repoPath, branch)

	data := map[string]interface{}{
		"Title":   "Agito - " + repoName,
		"Repo":    repo,
		"Commits": commits,
		"Files":   files,
		"Content": content,
		"Readme":  readme,
		"Branch":  branch,
	}

	w.Header().Set("Content-Type", "text/html; charset=utf-8")
	if err := ws.tmpl.ExecuteTemplate(w, "repo.html", data); err != nil {
		log.Printf("Template error: %v", err)
	}
}

func (ws *WebServer) listRepositories() ([]Repository, error) {
	var repos []Repository

	entries, err := os.ReadDir(ws.reposDir)
	if err != nil {
		return nil, err
	}

	for _, entry := range entries {
		if !entry.IsDir() {
			continue
		}

		repoPath := filepath.Join(ws.reposDir, entry.Name())
		
		// Check if it's a git repository
		if _, err := os.Stat(filepath.Join(repoPath, "HEAD")); os.IsNotExist(err) {
			continue
		}

		repo := Repository{
			Name: entry.Name(),
			Path: repoPath,
		}

		// Get description
		descPath := filepath.Join(repoPath, "description")
		if desc, err := os.ReadFile(descPath); err == nil {
			repo.Description = strings.TrimSpace(string(desc))
			if repo.Description == "Unnamed repository; edit this file 'description' to name the repository." {
				repo.Description = ""
			}
		}

		// Get last commit info
		cmd := exec.Command("git", "-C", repoPath, "log", "-1", "--format=%h - %s (%cr)")
		if output, err := cmd.Output(); err == nil {
			repo.LastCommit = strings.TrimSpace(string(output))
		}

		repos = append(repos, repo)
	}

	return repos, nil
}

func (ws *WebServer) getBranches(repoPath string) ([]string, error) {
	cmd := exec.Command("git", "-C", repoPath, "branch", "-a")
	output, err := cmd.Output()
	if err != nil {
		return nil, err
	}

	var branches []string
	lines := strings.Split(string(output), "\n")
	for _, line := range lines {
		line = strings.TrimSpace(line)
		line = strings.TrimPrefix(line, "* ")
		if line != "" && !strings.Contains(line, "->") {
			branches = append(branches, line)
		}
	}

	return branches, nil
}

func (ws *WebServer) getCommits(repoPath string, limit int) ([]CommitInfo, error) {
	cmd := exec.Command("git", "-C", repoPath, "log", 
		fmt.Sprintf("--max-count=%d", limit),
		"--format=%H|%an|%ar|%s")
	output, err := cmd.Output()
	if err != nil {
		return nil, err
	}

	var commits []CommitInfo
	lines := strings.Split(strings.TrimSpace(string(output)), "\n")
	for _, line := range lines {
		if line == "" {
			continue
		}
		parts := strings.SplitN(line, "|", 4)
		if len(parts) == 4 {
			commits = append(commits, CommitInfo{
				Hash:    parts[0][:8],
				Author:  parts[1],
				Date:    parts[2],
				Message: parts[3],
			})
		}
	}

	return commits, nil
}

func (ws *WebServer) listFiles(repoPath, branch, path string) ([]FileInfo, error) {
	treePath := branch + ":" + path
	cmd := exec.Command("git", "-C", repoPath, "ls-tree", treePath)
	output, err := cmd.Output()
	if err != nil {
		return nil, err
	}

	var files []FileInfo
	lines := strings.Split(strings.TrimSpace(string(output)), "\n")
	for _, line := range lines {
		if line == "" {
			continue
		}
		
		// Format: <mode> <type> <hash>\t<name>
		parts := strings.Fields(line)
		if len(parts) >= 4 {
			fileType := parts[1]
			name := strings.Join(parts[3:], " ")
			
			files = append(files, FileInfo{
				Name: name,
				Type: fileType,
				Path: filepath.Join(path, name),
			})
		}
	}

	return files, nil
}

func (ws *WebServer) getFileContent(repoPath, branch, path string) (string, error) {
	blobPath := branch + ":" + path
	cmd := exec.Command("git", "-C", repoPath, "show", blobPath)
	output, err := cmd.Output()
	if err != nil {
		return "", err
	}

	return string(output), nil
}

func (ws *WebServer) getReadme(repoPath, branch string) (string, error) {
	readmeNames := []string{"README.md", "README", "Readme.md", "readme.md"}
	
	for _, name := range readmeNames {
		content, err := ws.getFileContent(repoPath, branch, name)
		if err == nil {
			return content, nil
		}
	}

	return "", nil
}
