package git

import (
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
)

// Clone clones a repository using git
func Clone(url string, args ...string) error {
	cloneArgs := append([]string{"clone", url}, args...)
	cmd := exec.Command("git", cloneArgs...)
	cmd.Stdout = os.Stdout
	cmd.Stderr = os.Stderr
	cmd.Stdin = os.Stdin
	return cmd.Run()
}

// CreateRemoteRepo creates a bare repository on a remote agito server via SSH
func CreateRemoteRepo(server, user, repoName string) error {
	// Ensure repo name ends with .git
	if !strings.HasSuffix(repoName, ".git") {
		repoName = repoName + ".git"
	}

	// SSH command to create repository on server
	sshCmd := fmt.Sprintf("agito-create-repo %s", repoName)
	cmd := exec.Command("ssh", fmt.Sprintf("%s@%s", user, server), sshCmd)
	cmd.Stdout = os.Stdout
	cmd.Stderr = os.Stderr
	return cmd.Run()
}

// InitBareRepo initializes a bare git repository
func InitBareRepo(path string) error {
	if err := os.MkdirAll(path, 0755); err != nil {
		return fmt.Errorf("failed to create directory: %w", err)
	}

	cmd := exec.Command("git", "init", "--bare", path)
	output, err := cmd.CombinedOutput()
	if err != nil {
		return fmt.Errorf("failed to init repository: %w\nOutput: %s", err, string(output))
	}

	// Set up default hooks
	if err := setupHooks(path); err != nil {
		return fmt.Errorf("failed to setup hooks: %w", err)
	}

	return nil
}

// setupHooks sets up the server-side git hooks
func setupHooks(repoPath string) error {
	hooksDir := filepath.Join(repoPath, "hooks")
	
	// Create post-receive hook for CI/CD
	postReceive := filepath.Join(hooksDir, "post-receive")
	postReceiveContent := `#!/bin/sh
# Agito post-receive hook
# This hook is called after a push is completed

echo "Running post-receive hook..."

# Read the pushed refs
while read oldrev newrev refname; do
    echo "Processing: $refname"
    echo "  Old: $oldrev"
    echo "  New: $newrev"
    
    # Extract branch name
    branch=$(echo $refname | sed 's/refs\/heads\///')
    
    # Run CI/CD if configured
    if [ -f "$GIT_DIR/agito-ci.sh" ]; then
        echo "Running CI/CD pipeline for branch: $branch"
        sh "$GIT_DIR/agito-ci.sh" "$branch" "$oldrev" "$newrev"
    fi
done

echo "Post-receive hook completed."
`
	if err := os.WriteFile(postReceive, []byte(postReceiveContent), 0755); err != nil {
		return err
	}

	// Create pre-receive hook for validation
	preReceive := filepath.Join(hooksDir, "pre-receive")
	preReceiveContent := `#!/bin/sh
# Agito pre-receive hook
# This hook is called before a push is accepted

echo "Running pre-receive hook..."

# Read the refs being pushed
while read oldrev newrev refname; do
    echo "Validating: $refname"
    
    # Add custom validation logic here
    # Return non-zero to reject the push
done

echo "Pre-receive validation passed."
exit 0
`
	if err := os.WriteFile(preReceive, []byte(preReceiveContent), 0755); err != nil {
		return err
	}

	// Create update hook
	update := filepath.Join(hooksDir, "update")
	updateContent := `#!/bin/sh
# Agito update hook
# This hook is called for each ref being updated

refname="$1"
oldrev="$2"
newrev="$3"

echo "Update hook: $refname"

# Add custom branch protection logic here
# Return non-zero to reject the update

exit 0
`
	if err := os.WriteFile(update, []byte(updateContent), 0755); err != nil {
		return err
	}

	return nil
}

// GetRepoInfo returns information about a git repository
func GetRepoInfo(repoPath string) (map[string]string, error) {
	info := make(map[string]string)

	// Get description
	descPath := filepath.Join(repoPath, "description")
	if desc, err := os.ReadFile(descPath); err == nil {
		info["description"] = strings.TrimSpace(string(desc))
	}

	// Check if it's a bare repo
	cmd := exec.Command("git", "-C", repoPath, "rev-parse", "--is-bare-repository")
	output, err := cmd.Output()
	if err == nil {
		info["bare"] = strings.TrimSpace(string(output))
	}

	return info, nil
}

// ListRefs lists all refs in a repository
func ListRefs(repoPath string) ([]string, error) {
	cmd := exec.Command("git", "-C", repoPath, "show-ref")
	output, err := cmd.Output()
	if err != nil {
		// Empty repository might not have refs yet
		return []string{}, nil
	}

	refs := strings.Split(strings.TrimSpace(string(output)), "\n")
	return refs, nil
}
