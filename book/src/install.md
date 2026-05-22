# Install

Pick whichever package manager is already on your machine. Every method drops the same binary; the difference is who's curating the metadata.

## macOS / Linux — Homebrew

```bash
brew tap pawelb0/tap
brew install pawelb0/tap/mcpal
```

## Debian / Ubuntu — `.deb`

```bash
curl -fsSLO https://github.com/pawelb0/mcpal/releases/latest/download/mcpal_amd64.deb
sudo dpkg -i mcpal_amd64.deb
```

## Any platform — `cargo`

```bash
cargo install --git https://github.com/pawelb0/mcpal --path crates/mcpal
```

Needs a Rust toolchain (rustup recommended). Builds against the current `main`.

## Prebuilt binary — `curl | sh`

```bash
curl -fsSL https://raw.githubusercontent.com/pawelb0/mcpal/main/dist/install.sh | sh
```

Drops the binary into `$HOME/.local/bin`. Read the script first if you're cautious — it's short.

## Windows

```powershell
cargo install --git https://github.com/pawelb0/mcpal --path crates/mcpal
```

Tested on Windows 11 + Windows Terminal. Credentials persist via DPAPI (the OS-native secret store). A prebuilt MSI and `winget` / Scoop manifests are on the roadmap.

## Shell completions

```bash
mcpal completion bash > ~/.local/share/bash-completion/completions/mcpal
mcpal completion zsh  > ~/.zsh/completions/_mcpal       # ensure dir is on $fpath
mcpal completion fish > ~/.config/fish/completions/mcpal.fish
mcpal completion powershell >> $PROFILE
```

Re-source your shell after writing the completion file.

## Verify

```bash
mcpal --version
mcpal debug doctor
```

`debug doctor` runs a quick local sanity check — config path, keyring backend, presence of `npx` for stdio servers.

Curious what shipped in the version you got? See the [Changelog](./changelog.md).
