# Actions Commit

Action to commit files with verification through the workflow context, write permissions are required.

**Example:**
```yaml
- name: Commit Files
  uses: dev-bio/actions-commit@v1.0.0
  with:
    message: 'Commit message is required!'
    flatten: false # optional setting to discard directory structure
    include: |
      **/*.png
      **/*.md
    exclude: |
      **/image.png
      **/file.md
```