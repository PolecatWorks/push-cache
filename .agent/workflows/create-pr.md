---
description: Create a GitHub Pull Request with a summary of branch changes
---

1. Before creating a pull request, ensure that the Docker builds succeed for both backend images. Use the appropriate make targets:
// turbo
2. `make backend-docker`
// turbo
3. `make backend-java-docker`
4. Ensure the current branch is pushed to the remote repository. If not, push it.
5. Create a descriptive summary of the changes by analyzing the differences between the current branch and `main`. Run `git log main..HEAD -p` to view the changes.
6. Write the summary to a temporary file inside the current workspace instead of `/tmp/` to avoid out-of-workspace permissions prompts, e.g., `.pr_body.md`.
// turbo
7. Create the pull request using the `gh` tool and passing the file body: `gh pr create --fill --body-file .pr_body.md`
// turbo
8. Clean up the temporary file: `rm .pr_body.md`
