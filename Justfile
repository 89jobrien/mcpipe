pre-commit:
    cargo fmt --all --check
    cargo clippy --workspace -- -D warnings

prepush:
    cargo nextest run --workspace

ci:
    cargo fmt --all --check
    cargo clippy --workspace -- -D warnings
    cargo nextest run --workspace

install-hooks:
    #!/usr/bin/env sh
    printf '#!/bin/sh\njust pre-commit\n' > .git/hooks/pre-commit
    chmod +x .git/hooks/pre-commit
    printf '#!/bin/sh\njust prepush\n' > .git/hooks/pre-push
    chmod +x .git/hooks/pre-push
    printf '#!/bin/sh\ncommit_regex="^(feat|fix|docs|style|refactor|perf|test|chore|ci|build|revert)((.+))?: .+"\nif ! grep -qE "$commit_regex" "$1"; then\n  echo "warning: commit message does not follow conventional commits (non-blocking)"\nfi\n' > .git/hooks/commit-msg
    chmod +x .git/hooks/commit-msg
