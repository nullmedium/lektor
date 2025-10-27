# Makefile for lektor text editor

# Installation prefix
PREFIX ?= /usr/local
BINDIR = $(PREFIX)/bin
SHAREDIR = $(PREFIX)/share
DOCDIR = $(SHAREDIR)/doc/lektor
CONFIGDIR = $(SHAREDIR)/lektor

# Binary name
BINARY = lektor
TARGET_DIR = target/release

# Build flags
CARGO = cargo
CARGO_BUILD_FLAGS = --release
INSTALL = install
INSTALL_PROGRAM = $(INSTALL) -m 0755
INSTALL_DATA = $(INSTALL) -m 0644

# Default target
.PHONY: all
all: build

# Build the project
.PHONY: build
build:
	@echo "Building $(BINARY) in release mode..."
	$(CARGO) build $(CARGO_BUILD_FLAGS)

# Install the binary and supporting files
.PHONY: install
install: build
	@echo "Installing $(BINARY) to $(BINDIR)..."
	$(INSTALL) -d $(BINDIR)
	$(INSTALL_PROGRAM) $(TARGET_DIR)/$(BINARY) $(BINDIR)/$(BINARY)
	@echo "Installing example configuration to $(CONFIGDIR)..."
	$(INSTALL) -d $(CONFIGDIR)
	$(INSTALL_DATA) config.example.toml $(CONFIGDIR)/config.example.toml
	@echo "Installing documentation to $(DOCDIR)..."
	$(INSTALL) -d $(DOCDIR)
	$(INSTALL_DATA) README.md $(DOCDIR)/README.md
	$(INSTALL_DATA) LICENSE $(DOCDIR)/LICENSE
	$(INSTALL_DATA) FEATURES_AND_IMPROVEMENTS.md $(DOCDIR)/FEATURES_AND_IMPROVEMENTS.md
	$(INSTALL_DATA) SESSION_MANAGEMENT.md $(DOCDIR)/SESSION_MANAGEMENT.md
	@echo ""
	@echo "Installation complete!"
	@echo "Binary installed to: $(BINDIR)/$(BINARY)"
	@echo "Config example: $(CONFIGDIR)/config.example.toml"
	@echo "Documentation: $(DOCDIR)"
	@echo ""
	@echo "To configure lektor, copy the example config to ~/.config/lektor/config.toml:"
	@echo "  mkdir -p ~/.config/lektor"
	@echo "  cp $(CONFIGDIR)/config.example.toml ~/.config/lektor/config.toml"

# Uninstall the binary and supporting files
.PHONY: uninstall
uninstall:
	@echo "Uninstalling $(BINARY)..."
	rm -f $(BINDIR)/$(BINARY)
	rm -rf $(CONFIGDIR)
	rm -rf $(DOCDIR)
	@echo "Uninstallation complete!"
	@echo "Note: User configuration in ~/.config/lektor/ was not removed"

# Clean build artifacts
.PHONY: clean
clean:
	@echo "Cleaning build artifacts..."
	$(CARGO) clean

# Run tests
.PHONY: test
test:
	@echo "Running tests..."
	$(CARGO) test

# Build and run
.PHONY: run
run:
	@echo "Building and running $(BINARY)..."
	$(CARGO) run

# Debian package targets
.PHONY: deb
deb: deb-binary

.PHONY: deb-binary
deb-binary:
	@echo "Building Debian binary package..."
	@if ! command -v dpkg-buildpackage >/dev/null 2>&1; then \
		echo "Error: dpkg-buildpackage not found. Install with: apt-get install dpkg-dev"; \
		exit 1; \
	fi
	dpkg-buildpackage -us -uc -b

.PHONY: deb-source
deb-source:
	@echo "Building Debian source package..."
	@if ! command -v dpkg-buildpackage >/dev/null 2>&1; then \
		echo "Error: dpkg-buildpackage not found. Install with: apt-get install dpkg-dev"; \
		exit 1; \
	fi
	dpkg-buildpackage -us -uc -S

.PHONY: deb-clean
deb-clean:
	@echo "Cleaning Debian package build artifacts..."
	rm -rf debian/lektor
	rm -rf debian/.debhelper
	rm -rf debian/cargo
	rm -f debian/debhelper-build-stamp
	rm -f debian/files
	rm -f debian/*.substvars
	rm -f debian/*.log
	rm -f ../lektor_*.deb
	rm -f ../lektor_*.changes
	rm -f ../lektor_*.buildinfo
	rm -f ../lektor_*.dsc
	rm -f ../lektor_*.tar.*

# Display help
.PHONY: help
help:
	@echo "Lektor Text Editor - Build and Installation"
	@echo ""
	@echo "Usage: make [target] [PREFIX=/custom/prefix]"
	@echo ""
	@echo "Targets:"
	@echo "  all         Build the project (default)"
	@echo "  build       Build the project in release mode"
	@echo "  install     Install binary and files to PREFIX (default: /usr/local)"
	@echo "  uninstall   Remove installed files"
	@echo "  clean       Remove build artifacts"
	@echo "  test        Run tests"
	@echo "  run         Build and run the editor"
	@echo "  deb         Build Debian binary package"
	@echo "  deb-binary  Build Debian binary package (.deb)"
	@echo "  deb-source  Build Debian source package (.dsc, .tar.xz)"
	@echo "  deb-clean   Clean Debian package build artifacts"
	@echo "  help        Display this help message"
	@echo ""
	@echo "Variables:"
	@echo "  PREFIX      Installation prefix (default: /usr/local)"
	@echo ""
	@echo "Examples:"
	@echo "  make                    # Build the project"
	@echo "  sudo make install       # Install to /usr/local"
	@echo "  make PREFIX=~/.local install  # Install to ~/.local"
	@echo "  sudo make uninstall     # Remove installed files"
	@echo "  make deb                # Build Debian binary package"
	@echo "  make deb-source         # Build Debian source package"
	@echo "  sudo dpkg -i ../lektor_*.deb  # Install the .deb package"
