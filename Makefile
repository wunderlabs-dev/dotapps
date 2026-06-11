.PHONY: all install kill-dev vm-doctor dev-frontend dev dev-all build prepare-resources clean check test test-frontend test-all setup-macos setup-linux setup-dirs vm-image wsl-image help lint-frontend fmt-frontend lint-rust fmt-check lint skills-check parity worker-check worker-deploy

# Default target
all: install check

# Install all dependencies
install: setup-dirs
	cd src/web && pnpm install
	cd src/tauri && cargo fetch

# Lockfile prevents concurrent dev sessions from fighting over VM/ports.
# Checked before starting, removed on kill.
LOCKFILE := $(HOME)/.vibox/dev.lock

define check-lock
	@if [ -f $(LOCKFILE) ]; then \
		LOCK_PID=$$(cat $(LOCKFILE)); \
		if kill -0 $$LOCK_PID 2>/dev/null; then \
			echo "ERROR: another dev session is running (PID $$LOCK_PID)"; \
			echo "Run 'make kill-dev' first, or 'make status' to inspect."; \
			exit 1; \
		else \
			echo "Removing stale lockfile (PID $$LOCK_PID is dead)"; \
			rm -f $(LOCKFILE); \
		fi; \
	fi
endef

# Lock is written AFTER cargo tauri dev starts, using the actual opnble PID.
# See dev/dev-all/restart targets for the background write pattern.
define write-lock-when-ready
	(for i in $$(seq 1 30); do \
		PID=$$(pgrep -f "target/debug/opnble" | head -1); \
		if [ -n "$$PID" ]; then \
			echo "$$PID" > $(LOCKFILE); \
			break; \
		fi; \
		sleep 1; \
	done) &
endef

# Kill any running dev processes. Safe for VM image because vfkit
# writes to the APFS clone (alpine.img), never the base (alpine-base.img).
# The clone is disposable and recreated on next startup.
kill-dev:
	@-lsof -ti :1420 | xargs kill 2>/dev/null || true
	@-pkill -f "cargo[- ]tauri.*tauri dev" 2>/dev/null || true
	@-pkill -9 -f "target/debug/opnble" 2>/dev/null || true
	@sleep 1
	@-pkill -9 -f vfkit 2>/dev/null || true
	@-pkill -9 -f gvproxy 2>/dev/null || true
	@-pkill -9 -f "node.*vite" 2>/dev/null || true
	@-pkill -9 -f "pnpm dev" 2>/dev/null || true
	@sleep 1
	@-rm -f ~/.vibox/vm/alpine.img ~/.vibox/vm/vfkit-efi-store 2>/dev/null || true
	@-rm -f ~/.vibox/vm/agent.sock ~/.vibox/vm/gvproxy-*.sock ~/.vibox/vm/vfkit-*.sock 2>/dev/null || true
	@-rm -f $(LOCKFILE) 2>/dev/null || true

# Escalation for vfkit/gvproxy processes that survive kill-dev. Targets the
# macOS Hypervisor.framework uninterruptible-wait deadlock. Rerun with sudo
# if anything remains after the unprivileged pass.
vm-doctor:
	@bash $(CURDIR)/scripts/vm-doctor.sh

# Reset project state to stopped (clears stale running status)
reset-state:
	@python3 -c "import json; f=open('$(HOME)/.vibox/state.json'); s=json.load(f); f.close(); [p.update({'status':'stopped','intent':'stop','port':None}) for p in s['projects']]; s['nextPort']=3001; f=open('$(HOME)/.vibox/state.json','w'); json.dump(s,f,indent=2); f.close(); print('State reset: all projects stopped')"

# Run Vite dev server independently (must be running before `make dev`)
dev-frontend:
	$(check-lock)
	cd src/web && pnpm dev

# Run in development mode (Vite must be running separately via `make dev-frontend`)
dev: kill-dev
	$(write-lock-when-ready)
	cd src/tauri && cargo tauri dev; \
	rm -f $(LOCKFILE) 2>/dev/null || true

# Start both Vite and Tauri (Vite in background, then Tauri)
dev-all: kill-dev
	@cd src/web && pnpm dev &
	@echo "Waiting for Vite..."
	@for i in $$(seq 1 15); do \
		curl -s -o /dev/null http://localhost:1420 && break; \
		sleep 1; \
	done
	$(write-lock-when-ready)
	cd src/tauri && cargo tauri dev; \
	kill %1 2>/dev/null || true; \
	rm -f $(LOCKFILE) 2>/dev/null || true

# Full restart: kill, reset state, start fresh
restart: kill-dev reset-state
	@cd src/web && pnpm dev &
	@echo "Waiting for Vite..."
	@for i in $$(seq 1 15); do \
		curl -s -o /dev/null http://localhost:1420 && break; \
		sleep 1; \
	done
	$(write-lock-when-ready)
	cd src/tauri && cargo tauri dev; \
	kill %1 2>/dev/null || true; \
	rm -f $(LOCKFILE) 2>/dev/null || true

# Show VM and process status
status:
	@echo "=== Lock ==="
	@if [ -f $(LOCKFILE) ]; then \
		LOCK_PID=$$(cat $(LOCKFILE)); \
		if kill -0 $$LOCK_PID 2>/dev/null; then \
			echo "  LOCKED (PID $$LOCK_PID)"; \
		else \
			echo "  STALE (PID $$LOCK_PID is dead)"; \
		fi; \
	else \
		echo "  UNLOCKED"; \
	fi
	@echo "=== VM Processes ==="
	@ps aux | grep -E 'vfkit|gvproxy' | grep -v grep | wc -l | xargs echo "  count:"
	@echo "=== App ==="
	@ps aux | grep "target/debug/opnble" | grep -v grep | awk '{print "  PID:", $$2}' || echo "  NOT RUNNING"
	@echo "=== Agent ==="
	@test -S ~/.vibox/vm/agent.sock && echo "  UP" || echo "  DOWN"
	@echo "=== Vite ==="
	@curl -s -o /dev/null -w "  HTTP %{http_code}\n" http://localhost:1420/ 2>/dev/null || echo "  DOWN"
	@echo "=== Projects ==="
	@python3 -c "import json; f=open('$(HOME)/.vibox/state.json'); s=json.load(f); [print(f'  {p[\"name\"]}: {p[\"status\"]} intent={p.get(\"intent\")} port={p.get(\"port\")}') for p in s['projects']]" 2>/dev/null || echo "  no state file"


# Stage helper binaries (cloudflared, plus vfkit + gvproxy once Phase 1 lands)
# into src/tauri so the Tauri bundler can pick them up.
prepare-resources:
	./scripts/prepare-resources.sh

# Build for production. Matches CI: prepare-resources first, then bundle.
build: prepare-resources
	cd src/tauri && cargo tauri build

# Build frontend only
build-frontend:
	cd src/web && pnpm build

# Full frontend quality gate (biome + eslint + tsc + tests)
check-frontend:
	cd src/web && pnpm check

# Lint frontend (Biome + ESLint)
lint-frontend:
	cd src/web && pnpm lint

# Format frontend
fmt-frontend:
	cd src/web && pnpm format

# Rust quality gate (clippy with deny on all warnings)
lint-rust:
	cargo clippy -p opnble --all-targets -- -D warnings
	cargo clippy -p opnble-agent --all-targets -- -D warnings

# Check Rust formatting
fmt-check:
	cargo fmt -p opnble --check
	cargo fmt -p opnble-agent --check

# Full lint (format + clippy for both frontend and Rust)
lint: fmt-check lint-frontend lint-rust

# Check Rust code (quick compile check)
check-rust:
	cargo check --workspace

# Verify Tauri command registration matches the parity allowlist
parity:
	@./scripts/check-tauri-command-parity.sh

# Run all checks
check: check-frontend lint-rust fmt-check parity

# Validate Cursor project skill metadata
skills-check:
	@./scripts/check-cursor-skills.sh

# Run Rust tests for both workspace crates (host + agent)
test:
	cargo test -p opnble
	cargo test -p opnble-agent

# Run frontend tests
test-frontend:
	cd src/web && pnpm test

# Run all tests
test-all: test test-frontend

# Typecheck the Cloudflare tunnel worker
worker-check:
	cd src/worker && npm ci && npm run check

# Deploy the Cloudflare tunnel worker (requires CLOUDFLARE_API_TOKEN and CLOUDFLARE_ACCOUNT_ID)
worker-deploy:
	cd src/worker && npm run deploy

# Create required directories
setup-dirs:
	mkdir -p ~/.vibox/repos
	mkdir -p ~/.vibox/vm

# macOS setup (installs vfkit for Apple Silicon VM support)
setup-macos: setup-dirs
	@echo "Installing macOS dependencies..."
	brew install vfkit || true
	@echo "Done. Run 'make vm-image' to create the VM image."

# Linux setup (installs Podman)
setup-linux: setup-dirs
	@echo "Installing Linux dependencies..."
	@if command -v apt-get >/dev/null 2>&1; then \
		sudo apt-get update && sudo apt-get install -y podman; \
	elif command -v dnf >/dev/null 2>&1; then \
		sudo dnf install -y podman; \
	elif command -v pacman >/dev/null 2>&1; then \
		sudo pacman -S --noconfirm podman; \
	else \
		echo "Please install Podman manually"; \
	fi

# Create Alpine+Node VM image for macOS
vm-image: setup-dirs
	@echo "Creating Alpine Linux VM image with Node.js..."
	@./scripts/build-vm-image.sh

# Create Alpine WSL distro image for Windows
wsl-image:
	@echo "Creating Alpine WSL distro image with Podman..."
	@./src/tauri/resources/build-alpine-wsl.sh

# Clean build artifacts
clean:
	rm -rf src/web/dist
	rm -rf target
	rm -rf src/web/node_modules

# Clean everything including VM and repos (careful!)
clean-all: clean
	rm -rf ~/.vibox

# Format all code
fmt: fmt-frontend
	cargo fmt --all

# Show help
help:
	@echo "Opnble Development Commands"
	@echo ""
	@echo "  make install      - Install all dependencies"
	@echo "  make dev-frontend - Start Vite dev server independently"
	@echo "  make dev          - Run Tauri only (start Vite separately first)"
	@echo "  make dev-all      - Start both Vite and Tauri together"
	@echo "  make restart      - Kill all, reset state, start fresh"
	@echo "  make kill-dev     - Kill all dev processes and clean VM state"
	@echo "  make vm-doctor    - Force-clear stuck vfkit/gvproxy (try with sudo)"
	@echo "  make reset-state  - Reset all projects to stopped"
	@echo "  make status       - Show VM, agent, and project status"
	@echo "  make build        - Build for production (runs prepare-resources first)"
	@echo "  make prepare-resources - Stage helper binaries into src/tauri/"
	@echo "  make check        - Run all checks (frontend + Rust quality gate)"
	@echo "  make lint         - Full lint (format + clippy + frontend)"
	@echo "  make lint-rust    - Rust quality gate (clippy --all-targets -D warnings)"
	@echo "  make fmt-check    - Check formatting without changing files"
	@echo "  make skills-check - Validate Cursor project skill metadata"
	@echo "  make test         - Run Rust tests (host + agent)"
	@echo "  make worker-check - Typecheck the Cloudflare tunnel worker"
	@echo "  make worker-deploy - Deploy the Cloudflare tunnel worker"
	@echo "  make lint-frontend - Lint frontend (Biome + ESLint)"
	@echo "  make fmt          - Format all code (frontend + Rust)"
	@echo ""
	@echo "  make setup-macos  - Install macOS dependencies (vfkit)"
	@echo "  make setup-linux  - Install Linux dependencies (Podman)"
	@echo "  make vm-image     - Build Alpine+Node VM image"
	@echo "  make wsl-image    - Build Alpine WSL distro image"
	@echo ""
	@echo "  make clean        - Clean build artifacts"
	@echo "  make clean-all    - Clean everything (including ~/.vibox)"
	@echo ""
	@echo "Quick Start:"
	@echo "  make install && make vm-image && make dev"
