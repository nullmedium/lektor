# Debian Package Building Guide

This document describes how to build Debian packages for lektor.

## Prerequisites

Before building Debian packages, you need to install the required build dependencies:

```bash
sudo apt-get install debhelper cargo rustc libgit2-dev pkg-config dpkg-dev
```

Or use the Debian build dependency resolver:

```bash
sudo apt-get build-dep .
```

## Building Binary Packages

### Using Make

The simplest way to build a binary package is using the provided Makefile:

```bash
make deb
```

Or explicitly:

```bash
make deb-binary
```

This will create a `.deb` package in the parent directory (`../`).

### Using dpkg-buildpackage Directly

Alternatively, you can use `dpkg-buildpackage` directly:

```bash
dpkg-buildpackage -us -uc -b
```

Flags explanation:
- `-us`: Do not sign the source package
- `-uc`: Do not sign the changes file
- `-b`: Build binary package only

## Building Source Packages

To build a source package (for uploading to repositories or building on other systems):

```bash
make deb-source
```

Or directly:

```bash
dpkg-buildpackage -us -uc -S
```

This will create:
- `lektor_*.dsc` - Package description file
- `lektor_*.tar.xz` - Source tarball
- `lektor_*.changes` - Changes file

## Installing the Package

Once the package is built, install it with:

```bash
sudo dpkg -i ../lektor_*.deb
```

Or to also install any missing dependencies:

```bash
sudo apt-get install ../lektor_*.deb
```

## Uninstalling the Package

To remove the package:

```bash
sudo apt-get remove lektor
```

Or to also remove configuration files:

```bash
sudo apt-get purge lektor
```

## Cleaning Build Artifacts

To clean up Debian package build artifacts:

```bash
make deb-clean
```

This removes:
- Temporary build directories
- Generated `.deb`, `.dsc`, `.changes` files
- Build logs and metadata

## Package Information

### Package Contents

The Debian package installs:

- **Binary**: `/usr/bin/lektor`
- **Configuration example**: `/usr/share/lektor/config.example.toml`
- **Documentation**: `/usr/share/doc/lektor/`
  - `README.md`
  - `LICENSE`
  - `FEATURES_AND_IMPROVEMENTS.md`
  - `SESSION_MANAGEMENT.md`

### Dependencies

The package automatically depends on:
- System libraries (detected by `dpkg-shlibdeps`)
- No additional runtime dependencies required

### Package Metadata

- **Package name**: lektor
- **Section**: editors
- **Priority**: optional
- **Architecture**: any (native compiled for your system)
- **License**: BSD-2-Clause

## Debian Package Files

The `debian/` directory contains:

- **control**: Package metadata and dependencies
- **changelog**: Version history and changes
- **rules**: Build instructions (uses debhelper)
- **copyright**: License information
- **compat**: Debhelper compatibility level (11)
- **source/format**: Source package format (3.0 native)
- **README.Debian**: Debian-specific notes for users

## Customizing the Package

### Updating Version

Edit `debian/changelog` to add a new version entry:

```bash
lektor (0.1.1-1) unstable; urgency=medium

  * New release with bug fixes
  * Add new features

 -- Your Name <your.email@example.com>  Mon, 28 Oct 2024 10:00:00 +0000
```

Or use `dch` (from devscripts package):

```bash
dch -i  # Interactive mode
```

### Modifying Build Process

Edit `debian/rules` to customize the build process. The current implementation:
- Uses cargo to build in release mode
- Installs binary and supporting files
- Skips tests during package build

### Adding Dependencies

Edit `debian/control` to add runtime or build dependencies:

```
Build-Depends: debhelper (>= 11),
               cargo,
               rustc (>= 1.70),
               your-new-dependency

Depends: ${shlibs:Depends}, ${misc:Depends},
         your-runtime-dependency
```

## Troubleshooting

### Missing Build Dependencies

If you get "Unmet build dependencies" error:

```bash
sudo apt-get install debhelper cargo rustc libgit2-dev pkg-config
```

### Build Fails During Cargo Build

Ensure you have a recent Rust toolchain:

```bash
rustc --version  # Should be 1.70 or higher
```

Update Rust if needed:

```bash
rustup update
```

### Package Won't Install

Check for dependency issues:

```bash
dpkg -i ../lektor_*.deb
sudo apt-get install -f  # Fix dependencies
```

## Repository Upload

For uploading to a Debian repository (like PPA or custom repo):

1. Sign the package:
   ```bash
   dpkg-buildpackage -S
   ```

2. Upload with dput (configure your repository first):
   ```bash
   dput your-repo ../lektor_*.changes
   ```

## Building for Multiple Architectures

To build for different architectures (requires cross-compilation setup):

```bash
dpkg-buildpackage -a armhf  # For ARM
dpkg-buildpackage -a arm64  # For ARM64
```

Note: Cross-compilation for Rust requires additional setup.

## Further Reading

- [Debian New Maintainers' Guide](https://www.debian.org/doc/manuals/maint-guide/)
- [Debian Policy Manual](https://www.debian.org/doc/debian-policy/)
- [debhelper Documentation](https://manpages.debian.org/debhelper)
- [Rust Debian Packaging](https://wiki.debian.org/Teams/RustPackaging)
