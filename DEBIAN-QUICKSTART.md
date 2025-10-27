# Debian Packaging Quick Start

Quick reference for building and using Debian packages for lektor.

## Install Build Tools

```bash
sudo apt-get install debhelper cargo rustc libgit2-dev pkg-config dpkg-dev
```

## Build Commands

| Command | Description |
|---------|-------------|
| `make deb` | Build binary `.deb` package |
| `make deb-binary` | Build binary package (same as `make deb`) |
| `make deb-source` | Build source package (`.dsc`, `.tar.xz`) |
| `make deb-clean` | Clean all Debian build artifacts |

## Install Package

```bash
# After building
sudo dpkg -i ../lektor_*.deb

# Or with automatic dependency resolution
sudo apt-get install ../lektor_*.deb
```

## Remove Package

```bash
sudo apt-get remove lektor       # Remove package
sudo apt-get purge lektor        # Remove package + config
```

## Package Contents

After installation:
- **Binary**: `/usr/bin/lektor`
- **Config**: `/usr/share/lektor/config.example.toml`
- **Docs**: `/usr/share/doc/lektor/`

## Configuration

```bash
mkdir -p ~/.config/lektor
cp /usr/share/lektor/config.example.toml ~/.config/lektor/config.toml
```

## Package Files

The `debian/` directory structure:

```
debian/
├── changelog         # Version history
├── compat           # Debhelper version (11)
├── control          # Package metadata and dependencies
├── copyright        # License information (BSD-2-Clause)
├── README.Debian    # Debian-specific user notes
├── rules            # Build instructions (executable)
└── source/
    └── format       # Source package format
```

## Troubleshooting

**Build fails with missing dependencies:**
```bash
sudo apt-get build-dep .
```

**Check package contents before installing:**
```bash
dpkg -c ../lektor_*.deb
```

**Verify package info:**
```bash
dpkg -I ../lektor_*.deb
```

**Test package installation without installing:**
```bash
lintian ../lektor_*.deb
```

For detailed information, see [PACKAGING.md](PACKAGING.md).
